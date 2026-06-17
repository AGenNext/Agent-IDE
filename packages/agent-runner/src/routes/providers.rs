// Provider certification routes.
//
// GET  /api/providers/certified          — list all certifiable providers + current cert status
// POST /api/providers/certify            — certify a specific model string (sync, no ping)
// POST /api/providers/certify/ping       — certify + reachability ping (async)

use axum::{routing::{get, post}, extract::State, Json, Router};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::provider_cert;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/providers/certified",      get(certified_list))
        .route("/providers/certify",        post(certify_model))
        .route("/providers/certify/ping",   post(certify_model_ping))
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
