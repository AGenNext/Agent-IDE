// Marketplace routes — profile discovery, health, and edge routing.
// Every API consumer can query which profiles are available and healthy,
// then request a specific profile via X-Autonomyx-Profile header.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;
use crate::marketplace::MarketplaceRegistry;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/marketplace/profiles",         get(list_profiles))
        .route("/marketplace/profiles/:key",    get(get_profile))
        .route("/marketplace/health",           get(profile_health))
        .route("/marketplace/default",          get(get_default))
        .with_state(state)
}

async fn list_profiles(_: State<Arc<AppState>>) -> Json<Value> {
    let reg = MarketplaceRegistry::new();
    let profiles: Vec<Value> = reg.list().iter().map(|p| json!({
        "key":        p.key(),
        "provider":   p.provider,
        "operator":   p.operator,
        "profile":    p.name,
        "model":      p.model,
        "context":    p.context,
        "max_tokens": p.max_tokens,
        "tags":       p.tags,
        "default":    p.is_default,
        "key_configured": p.api_key_env.as_ref()
            .map(|e| std::env::var(e).map(|v| !v.is_empty()).unwrap_or(false))
            .unwrap_or(true),
    })).collect();

    Json(json!({ "profiles": profiles, "count": profiles.len() }))
}

async fn get_profile(Path(key): Path<String>, _: State<Arc<AppState>>) -> Json<Value> {
    let reg = MarketplaceRegistry::new();
    let decoded = key.replace(':', "/");
    match reg.get(&decoded) {
        Some(p) => Json(json!({
            "key":      p.key(),
            "provider": p.provider,
            "operator": p.operator,
            "profile":  p.name,
            "model":    p.model,
        })),
        None => Json(json!({ "error": "profile not found", "key": decoded })),
    }
}

async fn get_default(_: State<Arc<AppState>>) -> Json<Value> {
    let reg = MarketplaceRegistry::new();
    match reg.default_profile() {
        Some(p) => Json(json!({
            "key":      p.key(),
            "model":    p.model,
            "provider": p.provider,
        })),
        None => Json(json!({ "error": "no default profile configured" })),
    }
}

/// Probe each configured profile and return latency + availability.
/// This is the data the edge router uses for branching decisions.
async fn profile_health(_: State<Arc<AppState>>) -> Json<Value> {
    let reg   = MarketplaceRegistry::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let mut results = Vec::new();

    for profile in reg.list() {
        let base = match profile.resolve_base_url() {
            Some(u) => u,
            None    => {
                // Ollama: read from env
                std::env::var("OLLAMA_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".into())
            }
        };

        // Use /models endpoint as a lightweight health probe (OpenAI-compatible)
        let probe_url = format!("{}/v1/models", base.trim_end_matches('/'));
        let start     = std::time::Instant::now();

        let status = client.get(&probe_url)
            .header("authorization", format!("Bearer {}", profile.resolve_key()))
            .send()
            .await
            .map(|r| if r.status().is_success() { "online" } else { "degraded" })
            .unwrap_or("offline");

        let latency_ms = start.elapsed().as_millis();

        results.push(json!({
            "key":        profile.key(),
            "model":      profile.model,
            "status":     status,
            "latency_ms": latency_ms,
        }));
    }

    Json(json!({ "profiles": results, "probed_at": chrono::Utc::now() }))
}
