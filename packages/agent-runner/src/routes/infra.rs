use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router {
    Router::new().route("/infra/instances", get(list))
}

async fn list() -> Json<Value> {
    let raw = std::env::var("INFRA_INSTANCES").unwrap_or_default();
    if raw.is_empty() { return Json(json!([])); }
    match serde_json::from_str::<Value>(&raw) {
        Ok(v)  => Json(v),
        Err(e) => Json(json!({ "error": format!("INFRA_INSTANCES invalid JSON: {e}") })),
    }
}
