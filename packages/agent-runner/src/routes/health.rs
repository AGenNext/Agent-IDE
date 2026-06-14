use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<Value> {
    Json(json!({
        "status":    "ok",
        "platform":  "Autonomyx",
        "runtime":   "rust",
        "phase":     2,
        "version":   env!("CARGO_PKG_VERSION"),
        "providers": ["anthropic", "openai", "ollama", "openai-compatible"],
    }))
}
