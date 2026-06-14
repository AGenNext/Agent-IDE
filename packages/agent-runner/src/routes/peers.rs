use axum::{extract::{Path, State}, routing::{delete, get, post}, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

#[derive(Deserialize)]
struct PeerBody { name: String, url: String, region: Option<String> }

#[derive(Deserialize)]
struct TransferBody { agent_id: String, peer_id: String }

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/peers",     get(list).post(add))
        .route("/peers/:id", delete(remove))
        .route("/transfer",  post(push_agent))
        .with_state(state)
}

async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.list_peers()))
}

async fn add(State(state): State<Arc<AppState>>, Json(b): Json<PeerBody>) -> Json<Value> {
    let peer = state.create_peer(&b.name, &b.url, b.region.as_deref());
    let peer_id = peer.id.clone();
    let url = peer.url.clone();
    // Parallel health ping — doesn't block the response
    let state2 = state.clone();
    tokio::spawn(async move {
        let ok = reqwest::Client::new()
            .get(format!("{url}/health"))
            .timeout(std::time::Duration::from_secs(5))
            .send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        state2.set_peer_status(&peer_id, if ok { "online" } else { "offline" });
    });
    Json(json!(peer))
}

async fn remove(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    Json(json!({ "deleted": state.remove_peer(&id) }))
}

async fn push_agent(State(state): State<Arc<AppState>>, Json(b): Json<TransferBody>) -> Json<Value> {
    let peer = match state.get_peer(&b.peer_id) {
        Some(p) => p,
        None    => return Json(json!({ "ok": false, "message": "peer not found" })),
    };
    let agent = match state.get_agent(&b.agent_id) {
        Some(a) => a,
        None    => return Json(json!({ "ok": false, "message": "agent not found" })),
    };

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("content-type", "application/json".parse().unwrap());
    if let Ok(key) = std::env::var("TRANSFER_API_KEY") {
        headers.insert("authorization", format!("Bearer {key}").parse().unwrap());
    }

    let public_url = std::env::var("PUBLIC_URL").unwrap_or_default();
    let body = json!({ "agent": agent, "source_url": public_url });

    match reqwest::Client::new()
        .post(format!("{}/transfer/receive", peer.url))
        .headers(headers)
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send().await
    {
        Ok(r) if r.status().is_success() => {
            state.set_peer_status(&peer.id, "online");
            Json(json!({ "ok": true, "message": format!("Agent '{}' transferred to '{}'", agent.name, peer.name) }))
        }
        Ok(r) => {
            let msg = r.text().await.unwrap_or_default();
            Json(json!({ "ok": false, "message": msg }))
        }
        Err(e) => {
            state.set_peer_status(&peer.id, "offline");
            Json(json!({ "ok": false, "message": e.to_string() }))
        }
    }
}
