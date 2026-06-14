use axum::{routing::{get, post}, Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router {
    Router::new()
        .route("/infra/instances",    get(list_instances))
        .route("/infra/runner-status", get(runner_status))
        .route("/deploy/status",      get(deploy_status))
        .route("/deploy/apply",       post(deploy_apply))
        .route("/ci/runs",            get(ci_runs))
        .route("/ci/trigger",         post(ci_trigger))
}

// GET /api/infra/instances — reads from INFRA_INSTANCES env var (JSON array)
async fn list_instances() -> Json<Value> {
    let raw = std::env::var("INFRA_INSTANCES").unwrap_or_default();
    if raw.is_empty() { return Json(json!([])); }
    match serde_json::from_str::<Value>(&raw) {
        Ok(v)  => Json(v),
        Err(e) => Json(json!({ "error": format!("INFRA_INSTANCES invalid JSON: {e}") })),
    }
}

// GET /api/infra/runner-status — reports Rust binary identity
async fn runner_status() -> Json<Value> {
    Json(json!({
        "phase":         2,
        "runtime":       "rust",
        "binary":        "agent-runner",
        "version":       "0.1.0",
        "tokio_threads": 4
    }))
}

// GET /api/deploy/status — stub KubeContainer status
async fn deploy_status() -> Json<Value> {
    Json(json!({
        "name":     "agent-runner",
        "image":    "agent-runner:rust-latest",
        "replicas": 1,
        "health":   "healthy",
        "phase":    "Running"
    }))
}

// POST /api/deploy/apply — stub apply
async fn deploy_apply() -> Json<Value> {
    Json(json!({ "ok": true }))
}

// GET /api/ci/runs — stub CI run list (empty for now)
async fn ci_runs() -> Json<Value> {
    Json(json!([]))
}

// POST /api/ci/trigger — stub CI trigger
async fn ci_trigger() -> Json<Value> {
    Json(json!({ "ok": true, "message": "CI triggered" }))
}
