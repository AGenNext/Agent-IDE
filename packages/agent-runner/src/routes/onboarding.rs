// Chat-based onboarding routes — Autonomyx platform.
// The chat IS the configuration interface. No forms. No YAML wizards.
// Describe what you want. The platform configures itself.
//
// POST /api/onboarding/start        — start a new onboarding session
// POST /api/onboarding/:id/chat     — send a message, get a response
// GET  /api/onboarding/:id          — get session state + config so far
// POST /api/onboarding/:id/confirm  — confirm config, create the application

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;
use crate::onboarding::{
    OnboardingStage, ChatMessage,
    onboarding_system_prompt, extract_config_from_response,
    generate_ayx, should_advance, next_stage, build_messages,
};
use crate::providers;

#[derive(Deserialize)]
struct StartRequest {
    owner_id: Option<String>,
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    model:   Option<String>,
    api_key: Option<String>,
}

async fn start_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartRequest>,
) -> Json<Value> {
    let owner_id = req.owner_id.as_deref().unwrap_or("user_anonymous");
    let session  = state.onboarding.create_session(owner_id);

    Json(json!({
        "session_id": session.id,
        "stage":      session.stage,
        "message":    "Welcome to Autonomyx — the platform where anyone can build their next.\n\nWhat do you want to build?",
        "hint":       "Describe your application in plain language. I'll configure the platform for you.",
    }))
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.onboarding.get_session(&id) {
        Some(s) => Json(json!({
            "session": s,
            "stage":   s.stage,
            "config":  s.config,
            "app_id":  s.app_id,
            "app_did": s.app_did,
        })),
        None => Json(json!({ "error": "session not found", "id": id })),
    }
}

async fn chat(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ChatRequest>,
) -> Json<Value> {
    let mut session = match state.onboarding.get_session(&id) {
        Some(s) => s,
        None    => return Json(json!({ "error": "session not found" })),
    };

    if session.stage == OnboardingStage::Complete {
        return Json(json!({
            "message": "Your application is live. Visit GET /api/apps to see it.",
            "app_id":  session.app_id,
            "app_did": session.app_did,
            "stage":   "complete",
        }));
    }

    // Add user message to history
    session.messages.push(ChatMessage {
        role:    "user".into(),
        content: req.message.clone(),
    });

    // Extract configuration signals from this message
    let extracted = extract_config_from_response(&session.stage, "", &req.message);
    if extracted.app_name.is_some()    { session.config.app_name    = extracted.app_name; }
    if extracted.provider.is_some()    { session.config.provider     = extracted.provider; }
    if extracted.model.is_some()       { session.config.model        = extracted.model; }
    if extracted.budget_usd.is_some()  { session.config.budget_usd  = extracted.budget_usd; }
    if extracted.cloud.is_some()       { session.config.cloud        = extracted.cloud; }

    // Advance stage if we have enough info
    if should_advance(&session.stage, &req.message, &session.config) {
        session.stage = next_stage(&session.stage);
    }

    // In Review stage: generate the .ayx source for the LLM to present
    if session.stage == OnboardingStage::Review && session.config.ayx_source.is_none() {
        session.config.ayx_source = Some(generate_ayx(&session.config));
    }

    // Build LLM call
    let model   = req.model.as_deref().unwrap_or("claude-opus-4-8");
    let api_key = providers::resolve_key(req.api_key.as_deref().unwrap_or(""));
    let client  = reqwest::Client::new();

    // Add the current stage + config as context injection
    let stage_context = stage_context_message(&session.stage, &session.config);
    let mut messages  = build_messages(&session);

    // Inject stage context as a system note (user-invisible, before last user turn)
    if !stage_context.is_empty() && messages.len() > 0 {
        let last = messages.pop().unwrap();
        messages.push(serde_json::json!({
            "role": "user",
            "content": format!("[Stage: {:?}]\n{}\n\n{}", session.stage, stage_context, req.message)
        }));
        // Replace with combined; drop the duplicate we just popped
        let _ = last;
    }

    let system   = onboarding_system_prompt();
    let llm_text = match providers::resolve_provider(model) {
        providers::ProviderKind::Anthropic => {
            providers::anthropic::complete(&client, model, &api_key, system, &messages, 1024).await
        }
        providers::ProviderKind::OpenAI => {
            providers::openai::complete(&client, model, &api_key, system, &messages, 1024).await
        }
    };

    let assistant_text = match llm_text {
        Ok(r)  => r.text,
        Err(e) => {
            // Fallback: scripted onboarding when no LLM key is configured
            scripted_response(&session.stage, &session.config, e.to_string())
        }
    };

    // Add assistant response to history
    session.messages.push(ChatMessage {
        role:    "assistant".into(),
        content: assistant_text.clone(),
    });

    // If stage is now Complete: create the application
    let mut app_id  = session.app_id.clone();
    let mut app_did = session.app_did.clone();

    if session.stage == OnboardingStage::Complete && app_id.is_none() {
        let ayx = session.config.ayx_source.clone()
                      .unwrap_or_else(|| generate_ayx(&session.config));
        let app = state.create_app(
            &session.owner_id,
            session.config.app_name.as_deref().unwrap_or("MyApp"),
            session.config.description.as_deref().unwrap_or("Built on Autonomyx"),
            session.config.version.as_deref().unwrap_or("0.1.0"),
            Some(&ayx),
        );
        let did = format!("did:autonomyx:{}", uuid::Uuid::new_v4().simple());
        state.activate_app(&app.id, &did);

        app_id  = Some(app.id.clone());
        app_did = Some(did.clone());
        session.app_id  = app_id.clone();
        session.app_did = app_did.clone();

        tracing::info!(app_id = %app.id, did = %did, "onboarding complete — application created");
    }

    session.updated_at = chrono::Utc::now();
    state.onboarding.update_session(session.clone());

    Json(json!({
        "session_id": id,
        "stage":      session.stage,
        "message":    assistant_text,
        "config":     session.config,
        "app_id":     app_id,
        "app_did":    app_did,
    }))
}

async fn confirm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut session = match state.onboarding.get_session(&id) {
        Some(s) => s,
        None    => return Json(json!({ "error": "session not found" })),
    };

    if session.app_id.is_some() {
        return Json(json!({
            "message": "Application already created.",
            "app_id":  session.app_id,
            "app_did": session.app_did,
        }));
    }

    let ayx = session.config.ayx_source.clone()
                  .unwrap_or_else(|| generate_ayx(&session.config));

    let app = state.create_app(
        &session.owner_id,
        session.config.app_name.as_deref().unwrap_or("MyApp"),
        session.config.description.as_deref().unwrap_or("Built on Autonomyx"),
        session.config.version.as_deref().unwrap_or("0.1.0"),
        Some(&ayx),
    );
    let did = format!("did:autonomyx:{}", uuid::Uuid::new_v4().simple());
    state.activate_app(&app.id, &did);

    session.app_id  = Some(app.id.clone());
    session.app_did = Some(did.clone());
    session.stage   = OnboardingStage::Complete;
    state.onboarding.update_session(session);

    Json(json!({
        "message":  "Application is live. The theory is now real.",
        "app_id":   app.id,
        "app_did":  did,
        "ayx":      ayx,
        "next":     format!("GET /api/apps/{} to see your application", app.id),
    }))
}

// ── Stage context injection ───────────────────────────────────────────────────

fn stage_context_message(stage: &OnboardingStage, config: &crate::onboarding::OnboardingConfig) -> String {
    match stage {
        OnboardingStage::Deployment | OnboardingStage::Review => {
            if let Some(ayx) = &config.ayx_source {
                format!("Present this .ayx configuration to the user for confirmation:\n```ayx\n{ayx}\n```\nAsk: 'Does this look right? Say yes to create your application.'")
            } else {
                String::new()
            }
        }
        OnboardingStage::Complete => {
            "The user confirmed. Tell them their application is being created. Say it warmly and concisely.".into()
        }
        _ => String::new(),
    }
}

// ── Scripted fallback — works without LLM key ────────────────────────────────

fn scripted_response(stage: &OnboardingStage, config: &crate::onboarding::OnboardingConfig, err: String) -> String {
    let has_key = !err.contains("empty") && !err.contains("401") && !err.contains("403");
    let prefix = if has_key {
        format!("(LLM unavailable: {err}. Scripted mode.)\n\n")
    } else {
        "No LLM key configured — running in scripted mode. Set LLM_API_KEY or ANTHROPIC_API_KEY for full chat.\n\n".into()
    };

    let scripted = match stage {
        OnboardingStage::Welcome => "Welcome to Autonomyx — the platform where anyone can build their next.\n\nWhat do you want to build? Tell me about your application idea.",
        OnboardingStage::Purpose => "Got it. Which LLM would you like to use?\n\n• **Claude (Anthropic)** — claude-opus-4-8, most capable\n• **GPT-4o (OpenAI)** — fast and capable\n• **Ollama (local)** — self-hosted, $0 token cost\n\nOr just say 'default' for Claude.",
        OnboardingStage::Intelligence => "What agents does your application need? For example: 'a researcher, a builder, and a coordinator' — or just say 'default' for a general assistant.",
        OnboardingStage::Agents => "What's your monthly budget in USD? And where should it run — AWS, GCP, Azure, Hetzner, k3s, or local?\n\n(Or say 'default' for $50/month on k3s.)",
        OnboardingStage::Governance => "Got it. Where should your application run?\n\n• **k3s** — your own server or laptop (default)\n• **AWS / GCP / Azure** — managed cloud\n• **Hetzner** — cost-effective European cloud\n• **local** — development mode, $0\n\nOr say 'default' for k3s.",
        OnboardingStage::Deployment => {
            let ayx = generate_ayx(config);
            format!("Here's your application declaration:\n\n```ayx\n{ayx}\n```\n\nDoes this look right? Say **yes** to create your application.").leak()
        }
        OnboardingStage::Review => "Creating your application now...",
        OnboardingStage::Complete => {
            let name = config.app_name.as_deref().unwrap_or("your application");
            format!("{name} is live. Your DID has been assigned. Visit GET /api/apps to see it.").leak()
        }
    };

    format!("{prefix}{scripted}")
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/onboarding/start",       post(start_session))
        .route("/onboarding/:id",         get(get_session))
        .route("/onboarding/:id/chat",    post(chat))
        .route("/onboarding/:id/confirm", post(confirm))
        .with_state(state)
}
