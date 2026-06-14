use axum::{extract::{Path, State}, routing::{delete, get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::{agent, store::{AppState, RunStatus}};

#[derive(Deserialize)]
struct RunBody {
    task:       String,
    model:      Option<String>,
    agent_id:   Option<String>,
    agent_name: Option<String>,
    api_key:    Option<String>,
    max_iterations: Option<usize>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/runs",     get(list).post(create))
        .route("/runs/:id", get(get_one).delete(cancel))
        .with_state(state)
}

async fn create(State(state): State<Arc<AppState>>, Json(body): Json<RunBody>) -> Json<Value> {
    let run = state.create_run(
        body.agent_id.as_deref().unwrap_or("agent_demo"),
        body.agent_name.as_deref().unwrap_or("Agent"),
        body.model.as_deref().unwrap_or("claude-sonnet-4-6"),
        &body.task,
    );
    let req = agent::RunRequest {
        run_id:     run.run_id.clone(),
        agent_id:   run.agent_id.clone(),
        agent_name: run.agent_name.clone(),
        model:      run.model.clone(),
        task:       run.task.clone(),
        api_key:    body.api_key.unwrap_or_else(|| std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()),
        max_iter:   body.max_iterations.unwrap_or(10),
    };
    // Each run is an independent parallel Tokio task
    agent::spawn_run(state, req);
    Json(json!({ "runId": run.run_id, "status": "running", "wsUrl": format!("/ws/{}", run.run_id) }))
}

async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.list_runs()))
}

async fn get_one(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    match state.get_run(&id) {
        Some(r) => Json(json!(r)),
        None    => Json(json!({ "error": "not found" })),
    }
}

async fn cancel(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    state.finish_run(&id, RunStatus::Cancelled);
    Json(json!({ "cancelled": true }))
}
