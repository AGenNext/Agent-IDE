// Provider certification + runtime config routes.
//
// GET  /api/providers/certified          — list all certifiable providers + current cert status
// POST /api/providers/certify            — certify a specific model string (sync, no ping)
// POST /api/providers/certify/ping       — certify + reachability ping (async)
// GET  /api/providers/config             — read runtime LLM config
// PUT  /api/providers/config             — update runtime LLM config (no restart required)
// PUT  /api/providers/config/agent/:id   — set per-agent model override

use axum::{extract::{Path, State}, routing::{get, post, put}, Json, Router};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::provider_cert;
use crate::store::LlmConfig;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/providers/certified",          get(certified_list))
        .route("/providers/certify",            post(certify_model))
        .route("/providers/certify/ping",       post(certify_model_ping))
        .route("/providers/config",             get(get_config).put(put_config))
        .route("/providers/config/agent/:id",   put(put_agent_model))
        .with_state(state)
}

/// GET /api/providers/certified — certify every enabled plugin that is a compute provider.
async fn certified_list(State(s): State<Arc<AppState>>) -> Json<Value> {
    let probe_models = [
        ("claude-opus-4-8",  "plugin_anthropic"),
        ("gpt-4o",           "plugin_openai_compat"),
        ("ollama:llama3",    "plugin_ollama"),
        ("llama-rs:llama2",  "plugin_llamars"),
    ];

    let results: Vec<Value> = probe_models.iter().map(|(model, plugin_id)| {
        // Only certify if the plugin exists and is enabled
        let plugin = s.plugins.get(plugin_id);
        let relevant = plugin.as_ref().map(|p| p.enabled).unwrap_or(false)
            || *plugin_id == "plugin_openai_compat";

        if !relevant {
            return json!({
                "model":      model,
                "plugin_id":  plugin_id,
                "certified":  false,
                "skipped":    true,
                "reason":     "plugin not enabled",
            });
        }

        let cert = provider_cert::certify(model, &s);
        json!({
            "model":         cert.model,
            "plugin_id":     cert.provider_id,
            "cert_id":       cert.cert_id,
            "certified":     cert.certified,
            "trust_score":   cert.trust_score,
            "checks":        serde_json::to_value(&cert.checks).unwrap_or_default(),
            "reject_reason": cert.reject_reason,
            "certified_at":  cert.certified_at,
        })
    }).collect();

    let total     = results.len();
    let certified = results.iter().filter(|r| r["certified"] == true).count();

    Json(json!({
        "certified": certified,
        "total":     total,
        "providers": results,
        "policy":    "certified providers only — uncertified providers are rejected at run time",
    }))
}

#[derive(Deserialize)]
struct CertifyReq { model: String }

/// POST /api/providers/certify — certify a specific model string (no network ping).
async fn certify_model(
    State(s): State<Arc<AppState>>,
    Json(req): Json<CertifyReq>,
) -> Json<Value> {
    let cert = provider_cert::certify(&req.model, &s);
    Json(serde_json::to_value(&cert).unwrap_or_default())
}

/// POST /api/providers/certify/ping — certify + async reachability ping.
async fn certify_model_ping(
    State(s): State<Arc<AppState>>,
    Json(req): Json<CertifyReq>,
) -> Json<Value> {
    let cert = provider_cert::certify_with_ping(&req.model, &s).await;
    Json(serde_json::to_value(&cert).unwrap_or_default())
}

/// GET /api/providers/config — read current runtime LLM config.
async fn get_config(State(s): State<Arc<AppState>>) -> Json<Value> {
    let cfg = s.llm_config.read().unwrap().clone();
    Json(json!({
        "default_model":      cfg.default_model,
        "default_max_tokens": cfg.default_max_tokens,
        "agent_models":       cfg.agent_models,
        "resolution_order":   ["agent_model_field", "default_model", "LLM_MODEL env", "auto-detect from key"],
        "supported_providers": {
            "anthropic":  "claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5",
            "openai":     "gpt-4o, gpt-4o-mini, gpt-4-turbo (set OPENAI_BASE_URL for OpenAI-compatible)",
            "ollama":     "ollama:llama3, ollama:mistral — set OLLAMA_BASE_URL (default http://localhost:11434)",
            "vllm":       "any huggingface model — set OPENAI_BASE_URL to vLLM endpoint",
        },
    }))
}

#[derive(Deserialize)]
struct ConfigUpdate {
    default_model:      Option<String>,
    default_max_tokens: Option<u32>,
}

/// PUT /api/providers/config — update runtime LLM config without restart.
async fn put_config(
    State(s): State<Arc<AppState>>,
    Json(req): Json<ConfigUpdate>,
) -> Json<Value> {
    let mut cfg = s.llm_config.write().unwrap();
    if let Some(m) = req.default_model      { cfg.default_model      = m; }
    if let Some(t) = req.default_max_tokens { cfg.default_max_tokens = t; }
    Json(json!({
        "ok": true,
        "default_model":      cfg.default_model,
        "default_max_tokens": cfg.default_max_tokens,
    }))
}

#[derive(Deserialize)]
struct AgentModelUpdate { model: String }

/// PUT /api/providers/config/agent/:id — set per-agent model override.
async fn put_agent_model(
    State(s): State<Arc<AppState>>,
    Path(agent_id): Path<String>,
    Json(req): Json<AgentModelUpdate>,
) -> Json<Value> {
    let mut cfg = s.llm_config.write().unwrap();
    cfg.agent_models.insert(agent_id.clone(), req.model.clone());
    Json(json!({ "ok": true, "agent_id": agent_id, "model": req.model }))
}
