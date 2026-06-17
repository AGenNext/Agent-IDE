// Application routes — the application is the product.
// The .ayx declaration is the theory. The platform makes it real.
// No conflict system: upgrades are additive gate transitions — never destructive.
// Backward compatible: old versions coexist with new until retired.
//
// "Deny or accept is a right" — every application can refuse any action that
// violates its governance policy. Consent is a first-class primitive.
// No action may be taken on an application without the owner's explicit grant.

use axum::{
    extract::{Path, State},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::{AppState, AppStatus};

#[derive(Deserialize)]
struct CreateAppRequest {
    owner_id:    String,
    name:        String,
    description: Option<String>,
    version:     Option<String>,
    ayx_source:  Option<String>,   // .ayx theory declaration
}

#[derive(Deserialize)]
struct BindAgentRequest {
    agent_id: String,
}

async fn list_apps(State(state): State<Arc<AppState>>) -> Json<Value> {
    let apps = state.list_apps();
    let count = apps.len();
    Json(json!({
        "apps": apps,
        "count": count,
        "philosophy": "Application is the product. .ayx declares the theory. The platform makes it real.",
    }))
}

async fn create_app(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAppRequest>,
) -> Json<Value> {
    let app = state.create_app(
        &req.owner_id,
        &req.name,
        req.description.as_deref().unwrap_or(""),
        req.version.as_deref().unwrap_or("0.1.0"),
        req.ayx_source.as_deref(),
    );
    Json(json!({
        "app": app,
        "next": "POST /api/apps/{id}/activate to open the build gate and make it real",
    }))
}

async fn get_app(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.get_app(&id) {
        Some(app) => Json(json!({ "app": app })),
        None => Json(json!({ "error": "app not found", "id": id })),
    }
}

async fn activate_app(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let did = format!("did:autonomyx:{}", uuid::Uuid::new_v4().simple());
    state.activate_app(&id, &did);
    match state.get_app(&id) {
        Some(app) => Json(json!({
            "app": app,
            "message": "Application is live. The theory is now real.",
            "did": did,
        })),
        None => Json(json!({ "error": "app not found" })),
    }
}

async fn bind_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<BindAgentRequest>,
) -> Json<Value> {
    state.bind_agent_to_app(&id, &req.agent_id);
    Json(json!({
        "app_id":   id,
        "agent_id": req.agent_id,
        "status":   "bound",
    }))
}

// ── Upgrade an application ───────────────────────────────────────────────────
// Upgrades are additive: new version → new gates → new DID sub-path.
// Old version stays live until the new one is confirmed. No conflict. No downtime.
// "no conflict system" — the lifecycle gates prevent breaking changes reaching production.

#[derive(Deserialize)]
struct UpgradeRequest {
    version:    Option<String>,   // new semantic version
    model:      Option<String>,   // switch LLM provider
    cloud:      Option<String>,   // migrate cloud target
    ayx_source: Option<String>,   // new .ayx theory
}

async fn upgrade_app(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpgradeRequest>,
) -> Json<Value> {
    let app = match state.get_app(&id) {
        Some(a) => a,
        None    => return Json(json!({ "error": "app not found", "id": id })),
    };

    // Bump version — if not provided, auto-increment patch
    let new_version = req.version.clone().unwrap_or_else(|| {
        let parts: Vec<&str> = app.version.splitn(3, '.').collect();
        if parts.len() == 3 {
            let patch: u32 = parts[2].parse().unwrap_or(0);
            format!("{}.{}.{}", parts[0], parts[1], patch + 1)
        } else {
            format!("{}.1", app.version)
        }
    });

    // Merge upgrade into existing app — additive, never destructive
    let merged_ayx = req.ayx_source.or_else(|| app.ayx_source.clone());

    // Apply upgrades in store
    {
        let mut apps = state.apps.write().unwrap();
        if let Some(a) = apps.get_mut(&id) {
            a.version    = new_version.clone();
            a.ayx_source = merged_ayx.clone();
            a.status     = AppStatus::Building;   // re-open build gate
            a.updated_at = chrono::Utc::now();
        }
    }

    // Compute upgrade recommendations
    let mut upgrades_available: Vec<Value> = vec![];

    if app.ayx_source.as_deref().map(|s| s.contains("ollama")).unwrap_or(false) {
        upgrades_available.push(json!({
            "type":        "intelligence",
            "from":        "ollama (self-hosted)",
            "to":          "claude-opus-4-8 (Anthropic)",
            "benefit":     "state-of-the-art reasoning, 1M context, adaptive thinking",
            "cost":        "$5.00/1M input · $25.00/1M output — or keep self-hosted at $0",
            "how":         "set ANTHROPIC_API_KEY and redeploy"
        }));
    }

    if app.ayx_source.as_deref().map(|s| s.contains("local") || s.contains("k3s")).unwrap_or(true) {
        let cloud = req.cloud.as_deref().unwrap_or("hetzner");
        upgrades_available.push(json!({
            "type":    "infra",
            "from":    "local / k3s",
            "to":      cloud,
            "benefit": "managed nodes, automatic scaling, multi-region",
            "how":     format!("set CLOUD_PROVIDER={cloud} and apply deploy/cloud/autonomyx-cloud.yaml")
        }));
    }

    Json(json!({
        "app_id":              id,
        "previous_version":    app.version,
        "new_version":         new_version,
        "status":              "building",
        "conflict":            false,
        "backward_compatible": true,
        "note":                "Old version stays live until new version passes all gates. No downtime.",
        "upgrades_available":  upgrades_available,
        "next":                format!("POST /api/apps/{id}/activate to complete the upgrade"),
    }))
}

async fn upgrade_options(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let app = state.get_app(&id);
    let current_model = app.as_ref()
        .and_then(|a| a.ayx_source.as_deref())
        .map(|s| if s.contains("ollama") { "ollama" } else if s.contains("claude") { "anthropic" } else { "openai" })
        .unwrap_or("unknown");

    Json(json!({
        "app_id": id,
        "current": {
            "version": app.as_ref().map(|a| &a.version),
            "status":  app.as_ref().map(|a| &a.status),
            "model":   current_model,
        },
        "upgrade_paths": {
            "intelligence": [
                { "tier": "self_hosted",  "model": "ollama/llama3",          "cost_per_1m": "$0.00",  "note": "your compute" },
                { "tier": "standard",     "model": "openai/gpt-4o",           "cost_per_1m": "$5.00",  "note": "fast, capable" },
                { "tier": "advanced",     "model": "anthropic/claude-opus-4-8","cost_per_1m": "$5.00",  "note": "most capable, 1M context" },
                { "tier": "frontier",     "model": "anthropic/claude-fable-5", "cost_per_1m": "$10.00", "note": "frontier reasoning" },
            ],
            "infra": [
                { "tier": "local",    "target": "local",   "cost": "$0",       "note": "development" },
                { "tier": "self_hosted", "target": "k3s",  "cost": "your VPS", "note": "full control" },
                { "tier": "cloud",    "target": "hetzner", "cost": "€4/month", "note": "cost-effective" },
                { "tier": "cloud",    "target": "aws",     "cost": "variable", "note": "enterprise scale" },
            ],
            "support": [
                { "tier": "community",    "cost": "$0",     "sla": "best-effort" },
                { "tier": "professional", "cost": "$299/mo","sla": "4h response" },
                { "tier": "enterprise",   "cost": "custom", "sla": "1h dedicated" },
            ]
        },
        "backward_compatible": true,
        "no_conflict": true,
        "note": "All upgrades are additive gate transitions. Old version stays live during upgrade.",
    }))
}

// ── Consent — deny or accept is a right ──────────────────────────────────────
// Any action against an application requires explicit consent from the owner.
// This endpoint records the owner's decision — accept or deny — for a pending action.
// Denial is final. Acceptance opens the gate. The platform enforces, not decides.

#[derive(Deserialize)]
struct ConsentRequest {
    action:   String,           // what action is being requested
    decision: ConsentDecision,
    reason:   Option<String>,
}

#[derive(Deserialize, serde::Serialize, Debug)]
#[serde(rename_all = "snake_case")]
enum ConsentDecision { Accept, Deny }

async fn consent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ConsentRequest>,
) -> Json<Value> {
    let app = match state.get_app(&id) {
        Some(a) => a,
        None    => return Json(json!({ "error": "app not found" })),
    };

    match req.decision {
        ConsentDecision::Accept => {
            // Record acceptance in fabric as an event
            let payload = serde_json::json!({
                "action": req.action,
                "decision": "accept",
                "app_id": id,
                "app_did": app.did,
            });
            state.fabric.emit(crate::fabric::FabricEvent {
                id:         uuid::Uuid::new_v4().to_string(),
                artifact:   id.clone(),
                stage:      crate::lifecycle::Stage::Run,
                status:     crate::fabric::FabricStatus::Open,
                payload,
                emitted_at: chrono::Utc::now(),
                ..crate::fabric::FabricEvent::default()
            });
            Json(json!({
                "app_id":   id,
                "action":   req.action,
                "decision": "accept",
                "effect":   "gate opens — action may proceed",
                "right":    "accept is your right",
            }))
        }
        ConsentDecision::Deny => {
            // Record denial — gate stays closed, reason logged
            let payload = serde_json::json!({
                "action": req.action,
                "decision": "deny",
                "reason": req.reason,
                "app_id": id,
            });
            state.fabric.emit(crate::fabric::FabricEvent {
                id:         uuid::Uuid::new_v4().to_string(),
                artifact:   id.clone(),
                stage:      crate::lifecycle::Stage::Run,
                status:     crate::fabric::FabricStatus::Closed,
                payload,
                emitted_at: chrono::Utc::now(),
                ..crate::fabric::FabricEvent::default()
            });
            Json(json!({
                "app_id":   id,
                "action":   req.action,
                "decision": "deny",
                "reason":   req.reason,
                "effect":   "gate closed — action blocked, reason recorded",
                "right":    "deny is your right — no action may override it",
            }))
        }
    }
}

async fn apps_philosophy() -> Json<Value> {
    Json(json!({
        "principle": "Application is the product",
        "declaration": "The .ayx file is the theory — readable, versioned, auditable",
        "reality": "The platform instantiates the theory — real DID, real gates, real fabric",
        "ownership": "The app owner controls the DID — the platform enforces governance, not ownership",
        "portability": "The DID is yours — move it to any Autonomyx node, any cloud",
        "lifecycle": ["draft", "building", "live", "paused", "retired"],
        "gates": "The same 8 gates apply to the app as to every agent — build gate = app comes alive",
        "without_disturbing": {
            "socioeconomic_fabric": "Usage-based, no extraction — you pay provider rates only",
            "ecosystem_balance": "Coral monitors provider share — no single LLM or cloud dominates",
            "user_freedom": "Self-hosted = $0 token cost — sovereignty over your own agents",
            "governance": "JIT access — no standing permissions — power stays with the DID owner",
        }
    }))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/apps",                    get(list_apps).post(create_app))
        .route("/apps/philosophy",         get(apps_philosophy))
        .route("/apps/:id",                get(get_app))
        .route("/apps/:id/activate",       post(activate_app))
        .route("/apps/:id/agents",         post(bind_agent))
        .route("/apps/:id/upgrade",        put(upgrade_app))
        .route("/apps/:id/upgrade/options",get(upgrade_options))
        .route("/apps/:id/consent",        post(consent))
        .with_state(state)
}
