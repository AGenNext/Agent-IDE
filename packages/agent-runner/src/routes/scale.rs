use axum::{extract::State, routing::get, Json, Router};
use serde_json::Value;
use std::sync::Arc;
use crate::store::AppState;
use crate::scale;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/scale",         get(report))
        .route("/scale/config",  get(config))
        .route("/scale/metrics", get(metrics))
        .route("/scale/hpa",     get(hpa))
        .route("/scale/keda",    get(keda))
        .with_state(state)
}

async fn report(State(state): State<Arc<AppState>>) -> Json<Value> {
    let active_runs = state.runs.read().unwrap()
        .values()
        .filter(|r| r.status == crate::store::RunStatus::Running)
        .count();
    Json(scale::report(active_runs))
}

async fn config() -> Json<Value> {
    Json(serde_json::to_value(scale::ScaleConfig::from_env()).unwrap_or_default())
}

async fn metrics(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cfg = scale::ScaleConfig::from_env();
    let active_runs = state.runs.read().unwrap()
        .values()
        .filter(|r| r.status == crate::store::RunStatus::Running)
        .count();
    let m = scale::ScaleMetrics::sample(active_runs, &cfg);
    Json(serde_json::to_value(m).unwrap_or_default())
}

async fn hpa() -> Json<Value> {
    let cfg = scale::ScaleConfig::from_env();
    let ns  = std::env::var("K8S_NAMESPACE").unwrap_or_else(|_| "autonomyx".into());
    Json(scale::hpa_manifest(&cfg, &ns))
}

async fn keda() -> Json<Value> {
    let cfg = scale::ScaleConfig::from_env();
    let ns  = std::env::var("K8S_NAMESPACE").unwrap_or_else(|_| "autonomyx".into());
    Json(scale::keda_manifest(&cfg, &ns))
}
