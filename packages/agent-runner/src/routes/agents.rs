use axum::{extract::{Path, State}, routing::{get, post, delete}, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

#[derive(Deserialize)]
struct AgentBody { name: String, description: Option<String>, model: Option<String> }

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/agents",     get(list).post(create))
        .route("/agents/:id", get(get_one).delete(remove))
        .with_state(state)
}

async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.list_agents()))
}

async fn create(State(state): State<Arc<AppState>>, Json(b): Json<AgentBody>) -> Json<Value> {
    let a = state.create_agent("user_demo", &b.name, b.description.as_deref().unwrap_or(""), b.model.as_deref().unwrap_or("claude-sonnet-4-6"));
    Json(json!(a))
}

async fn get_one(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    match state.get_agent(&id) { Some(a) => Json(json!(a)), None => Json(json!({ "error": "not found" })) }
}

async fn remove(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    state.agents.write().unwrap().remove(&id);
    Json(json!({ "deleted": true }))
}
