// Application routes — the application is the product.
// The .ayx declaration is the theory. The platform makes it real.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

#[derive(Deserialize)]
struct CreateAppRequest {
    owner_id:    String,
    name:        String,
    description: Option<String>,
    version:     Option<String>,
    ayx_source:  Option<String>,   // .ayx theory declaration
}

#[derive(Deserialize)]
struct BindAgentRequest {
    agent_id: String,
}

async fn list_apps(State(state): State<Arc<AppState>>) -> Json<Value> {
    let apps = state.list_apps();
    let count = apps.len();
    Json(json!({
        "apps": apps,
        "count": count,
        "philosophy": "Application is the product. .ayx declares the theory. The platform makes it real.",
    }))
}

async fn create_app(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAppRequest>,
) -> Json<Value> {
    let app = state.create_app(
        &req.owner_id,
        &req.name,
        req.description.as_deref().unwrap_or(""),
        req.version.as_deref().unwrap_or("0.1.0"),
        req.ayx_source.as_deref(),
    );
    Json(json!({
        "app": app,
        "next": "POST /api/apps/{id}/activate to open the build gate and make it real",
    }))
}

async fn get_app(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.get_app(&id) {
        Some(app) => Json(json!({ "app": app })),
        None => Json(json!({ "error": "app not found", "id": id })),
    }
}

async fn activate_app(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let did = format!("did:autonomyx:{}", uuid::Uuid::new_v4().simple());
    state.activate_app(&id, &did);
    match state.get_app(&id) {
        Some(app) => Json(json!({
            "app": app,
            "message": "Application is live. The theory is now real.",
            "did": did,
        })),
        None => Json(json!({ "error": "app not found" })),
    }
}

async fn bind_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<BindAgentRequest>,
) -> Json<Value> {
    state.bind_agent_to_app(&id, &req.agent_id);
    Json(json!({
        "app_id":   id,
        "agent_id": req.agent_id,
        "status":   "bound",
    }))
}

async fn apps_philosophy() -> Json<Value> {
    Json(json!({
        "principle": "Application is the product",
        "declaration": "The .ayx file is the theory — readable, versioned, auditable",
        "reality": "The platform instantiates the theory — real DID, real gates, real fabric",
        "ownership": "The app owner controls the DID — the platform enforces governance, not ownership",
        "portability": "The DID is yours — move it to any Autonomyx node, any cloud",
        "lifecycle": ["draft", "building", "live", "paused", "retired"],
        "gates": "The same 8 gates apply to the app as to every agent — build gate = app comes alive",
        "without_disturbing": {
            "socioeconomic_fabric": "Usage-based, no extraction — you pay provider rates only",
            "ecosystem_balance": "Coral monitors provider share — no single LLM or cloud dominates",
            "user_freedom": "Self-hosted = $0 token cost — sovereignty over your own agents",
            "governance": "JIT access — no standing permissions — power stays with the DID owner",
        }
    }))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/apps",               get(list_apps).post(create_app))
        .route("/apps/philosophy",    get(apps_philosophy))
        .route("/apps/:id",           get(get_app))
        .route("/apps/:id/activate",  post(activate_app))
        .route("/apps/:id/agents",    post(bind_agent))
        .with_state(state)
}
