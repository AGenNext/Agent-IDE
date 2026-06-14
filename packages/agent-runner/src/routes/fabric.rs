use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use crate::store::AppState;
use crate::fabric::FabricEvent;

async fn event_log(State(state): State<Arc<AppState>>) -> Json<Vec<FabricEvent>> {
    Json(state.fabric.full_log())
}

async fn artifact_events(
    State(state): State<Arc<AppState>>,
    Path(artifact): Path<String>,
) -> Json<Vec<FabricEvent>> {
    Json(state.fabric.log_for(&artifact))
}

async fn dead_log(State(state): State<Arc<AppState>>) -> Json<Vec<FabricEvent>> {
    Json(state.fabric.dead_log())
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/fabric/log",                  get(event_log))
        .route("/fabric/dead",                 get(dead_log))
        .route("/fabric/:artifact/events",     get(artifact_events))
        .with_state(state)
}
