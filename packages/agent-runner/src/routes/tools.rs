use axum::{extract::{Path, State}, routing::{get, post}, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/tools",           get(list))
        .route("/tools/:id/invoke", post(invoke))
        .with_state(state)
}

async fn list(_: axum::extract::State<Arc<AppState>>) -> Json<Value> {
    Json(json!([
        { "id": "http_client", "name": "HTTP Client",   "category": "api" },
        { "id": "web_search",  "name": "Web Search",    "category": "web" },
        { "id": "shell",       "name": "Shell",          "category": "code" },
    ]))
}

async fn invoke(Path(id): Path<String>, Json(input): Json<Value>) -> Json<Value> {
    let result = crate::tools::invoke(&id, &input).await;
    Json(json!({ "result": result }))
}
