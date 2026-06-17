use axum::{routing::get, Json, Router};
use serde_json::Value;
use crate::cloud;

pub fn router() -> Router {
    Router::new()
        .route("/cloud",       get(context))
        .route("/cloud/stack", get(stack))
}

async fn context() -> Json<Value> {
    Json(serde_json::json!({
        "cloud":  cloud::CloudContext::detect(),
        "device": cloud::DeviceContext::detect(),
    }))
}

async fn stack() -> Json<Value> {
    Json(cloud::cloud_stack())
}
