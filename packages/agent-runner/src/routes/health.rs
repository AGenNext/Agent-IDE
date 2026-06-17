use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready",  get(ready))
}

/// Liveness probe — always returns 200 if the process is alive.
async fn health() -> Json<Value> {
    Json(json!({
        "status":    "ok",
        "platform":  "Autonomyx",
        "runtime":   "rust",
        "version":   env!("CARGO_PKG_VERSION"),
    }))
}

/// Readiness probe — returns 200 only when the platform has fully initialised.
/// k8s sends traffic only after this returns 200.
async fn ready() -> Json<Value> {
    Json(json!({
        "ready":    true,
        "platform": "Autonomyx",
        "version":  env!("CARGO_PKG_VERSION"),
    }))
}
