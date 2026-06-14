// Transfer layer — agent teleportation.
// Egress: POST /api/transfer → push agent to peer (runs in its own Tokio task).
// Ingress: POST /transfer/receive → gated by TRANSFER_API_KEY.

use axum::{extract::State, http::{HeaderMap, StatusCode}, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::store::{AppState, AgentIdentity};

#[derive(Deserialize)]
struct ReceivePayload {
    agent:      AgentIdentity,
    source_url: Option<String>,
}

#[derive(Serialize)]
struct ReceiveResponse {
    ok:          bool,
    imported_id: String,
    name:        String,
}

async fn receive(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ReceivePayload>,
) -> impl IntoResponse {
    // Gate: require Bearer TRANSFER_API_KEY if set
    if let Ok(key) = std::env::var("TRANSFER_API_KEY") {
        let auth = headers.get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if auth != format!("Bearer {key}") {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "Unauthorized" }))).into_response();
        }
    }

    let src = payload.source_url.as_deref().unwrap_or("unknown");
    let imported = state.create_agent(
        "user_demo",
        &payload.agent.name,
        &format!("Teleported from {src}"),
        &payload.agent.model,
    );
    tracing::info!("transfer: received agent '{}' ({}) from {}", imported.name, imported.id, src);
    Json(ReceiveResponse { ok: true, imported_id: imported.id.clone(), name: imported.name.clone() }).into_response()
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/receive", post(receive))
        .with_state(state)
}
