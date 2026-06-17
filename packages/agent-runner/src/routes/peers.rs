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
        .route("/peers",              get(list).post(add))
        .route("/peers/:id",          delete(remove))
        .route("/peers/announce",     post(announce))   // auto-clustering inbound
        .route("/transfer",           post(push_agent))
        .with_state(state)
}

/// POST /api/peers/announce — called by remote nodes in auto-cluster mode.
/// Registers or updates the calling node in the peer registry.
async fn announce(State(state): State<Arc<AppState>>, Json(b): Json<serde_json::Value>) -> Json<Value> {
    let url    = b.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let name   = b.get("name").and_then(|v| v.as_str()).unwrap_or("remote-node").to_string();
    let status = b.get("status").and_then(|v| v.as_str()).unwrap_or("online").to_string();
    if url.is_empty() { return Json(json!({ "ok": false, "error": "url required" })); }

    // Upsert peer — if URL already registered, just update status
    let existing = state.list_peers().into_iter().find(|p| p.url == url);
    if let Some(p) = existing {
        state.set_peer_status(&p.id, &status);
        Json(json!({ "ok": true, "peer_id": p.id, "action": "updated" }))
    } else {
        let peer = state.create_peer(&name, &url, None);
        state.set_peer_status(&peer.id, &status);
        tracing::info!(peer_id = %peer.id, url = %url, "cluster: new node announced — auto-joined");
        Json(json!({ "ok": true, "peer_id": peer.id, "action": "joined" }))
    }
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
