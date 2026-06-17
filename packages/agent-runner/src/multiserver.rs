// Multi-server connection bridge — fabric mesh across platform instances.
//
// When multiple Autonomyx nodes run (k8s replicas, edge sites, regions):
//   - Each node emits events locally to its in-memory fabric channel.
//   - WebSocket clients on node B miss events that happened on node A.
//
// This bridge fixes that:
//   1. On startup, reads the peer registry for known sibling nodes.
//   2. For each peer, opens a WebSocket connection to their /ws/fabric endpoint.
//   3. Any event received from a peer is re-emitted into the local fabric channel.
//   4. New peers registered at runtime are auto-connected within 10 s.
//   5. Dead connections reconnect with exponential back-off (2s → 64s cap).
//
// Result: every fabric stream subscriber on any node sees events from all nodes.
// No central broker required. Peer-to-peer mesh, same DID/Bearer auth.
//
// openautonomyx.com

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::store::AppState;
use crate::fabric::FabricEvent;

const RECONNECT_BASE_MS:  u64 = 2_000;
const RECONNECT_CAP_MS:   u64 = 64_000;
const PEER_POLL_SECS:     u64 = 10;
const CLUSTER_ANNOUNCE_SECS: u64 = 30;  // auto-clustering: broadcast self to peers

/// Start the multi-server bridge + auto-clustering in the background.
/// Spawns one reconciler task that monitors the peer registry,
/// one per-peer bridge task for every online peer,
/// and one cluster-announce task that advertises this node to all peers.
pub fn start(state: Arc<AppState>) {
    let s1 = state.clone();
    tokio::spawn(async move { run_bridge(s1).await });

    // Auto-clustering: periodically announce this node's existence to all peers
    let s2 = state.clone();
    tokio::spawn(async move { run_cluster_announce(s2).await });
}

/// Auto-clustering — broadcasts this node's URL + port to all registered peers.
/// When peers receive the announce, they add this node to their peer registry
/// and initiate a fabric bridge. No manual cluster config needed.
async fn run_cluster_announce(state: Arc<AppState>) {
    let mut tick = tokio::time::interval(Duration::from_secs(CLUSTER_ANNOUNCE_SECS));
    let self_url = std::env::var("AUTONOMYX_PUBLIC_URL")
        .unwrap_or_else(|_| {
            let port = std::env::var("PORT").unwrap_or_else(|_| "3001".into());
            format!("http://localhost:{port}")
        });

    loop {
        tick.tick().await;
        let peers: Vec<(String, String)> = state.peers.read().unwrap()
            .values()
            .map(|p| (p.id.clone(), p.url.clone()))
            .collect();

        for (peer_id, peer_url) in peers {
            let announce_url = format!("{}/api/peers/announce", peer_url.trim_end_matches('/'));
            let body = serde_json::json!({
                "url":    self_url,
                "name":   std::env::var("AUTONOMYX_NODE_NAME").unwrap_or_else(|_| "autonomyx-node".into()),
                "status": "online",
            });

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build().unwrap_or_default();

            match client.post(&announce_url).json(&body).send().await {
                Ok(r) if r.status().is_success() =>
                    tracing::debug!(peer = %peer_id, "cluster: announce ok"),
                Ok(r) =>
                    tracing::debug!(peer = %peer_id, status = %r.status(), "cluster: announce non-2xx"),
                Err(e) =>
                    tracing::debug!(peer = %peer_id, error = %e, "cluster: announce failed (peer may be down)"),
            }
        }
    }
}

async fn run_bridge(state: Arc<AppState>) {
    // Track live bridge tasks by peer ID.
    // JoinHandle lets us detect finished (dead) tasks and respawn them.
    let mut bridges: std::collections::HashMap<String, tokio::task::JoinHandle<()>> =
        std::collections::HashMap::new();

    loop {
        // Prune finished handles — dead connections are respawnable
        bridges.retain(|_, handle| !handle.is_finished());

        let peers = state.peers.read().unwrap()
            .values()
            .filter(|p| p.status == "online" || p.status == "connected")
            .map(|p| (p.id.clone(), p.url.clone()))
            .collect::<Vec<_>>();

        for (id, url) in peers {
            if bridges.contains_key(&id) { continue; }

            let ws_url = peer_ws_url(&url);
            if ws_url.is_empty() { continue; }

            let state2  = state.clone();
            let peer_id = id.clone();
            let handle = tokio::spawn(async move {
                bridge_peer(peer_id, ws_url, state2).await;
            });
            bridges.insert(id, handle);
        }

        sleep(Duration::from_secs(PEER_POLL_SECS)).await;
    }
}

/// Maintain a WebSocket connection to a single peer's /ws/fabric endpoint.
/// Reconnects indefinitely with exponential back-off.
async fn bridge_peer(peer_id: String, ws_url: String, state: Arc<AppState>) {
    let mut backoff_ms = RECONNECT_BASE_MS;

    loop {
        tracing::info!(peer = %peer_id, url = %ws_url, "multiserver: connecting to peer fabric");

        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                backoff_ms = RECONNECT_BASE_MS; // reset on successful connect
                tracing::info!(peer = %peer_id, "multiserver: peer fabric connected");

                let (mut sink, mut stream) = ws_stream.split();

                loop {
                    tokio::select! {
                        msg = stream.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    // Skip welcome/meta frames (not FabricEvents)
                                    if let Ok(event) = serde_json::from_str::<FabricEvent>(&text) {
                                        // Re-emit into local fabric so all local subscribers receive it
                                        state.fabric.emit(event);
                                    }
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    let _ = sink.send(Message::Pong(data)).await;
                                }
                                Some(Ok(Message::Close(_))) | None => {
                                    tracing::warn!(peer = %peer_id, "multiserver: peer connection closed");
                                    break;
                                }
                                Some(Err(e)) => {
                                    tracing::warn!(peer = %peer_id, error = %e, "multiserver: peer ws error");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    peer    = %peer_id,
                    url     = %ws_url,
                    error   = %e,
                    backoff = backoff_ms,
                    "multiserver: peer connection failed — will retry"
                );
            }
        }

        sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(RECONNECT_CAP_MS);
    }
}

/// Convert a peer's base URL to a fabric WebSocket URL.
/// https://peer.example.com → wss://peer.example.com/ws/fabric
/// http://10.0.0.5:3001    → ws://10.0.0.5:3001/ws/fabric
fn peer_ws_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.starts_with("https://") {
        format!("wss://{}/ws/fabric", &trimmed["https://".len()..])
    } else if trimmed.starts_with("http://") {
        format!("ws://{}/ws/fabric", &trimmed["http://".len()..])
    } else if trimmed.starts_with("wss://") || trimmed.starts_with("ws://") {
        format!("{}/ws/fabric", trimmed)
    } else {
        String::new()
    }
}

/// Connection status for each peer bridge — exposed via /api/multiserver/status
#[derive(Debug, Clone, serde::Serialize)]
pub struct PeerBridgeStatus {
    pub peer_id:   String,
    pub ws_url:    String,
    pub connected: bool,
    pub note:      String,
}

/// Summary of multi-server bridge state (called from routes).
pub fn bridge_summary(state: &AppState) -> serde_json::Value {
    let peers = state.peers.read().unwrap();
    let online: Vec<_> = peers.values()
        .filter(|p| p.status == "online" || p.status == "connected")
        .map(|p| serde_json::json!({
            "peer_id":  p.id,
            "name":     p.name,
            "url":      p.url,
            "ws_url":   peer_ws_url(&p.url),
            "region":   p.region,
        }))
        .collect();

    serde_json::json!({
        "mode":         "peer-to-peer fabric mesh",
        "description":  "Each online peer gets a WS bridge; events from all peers flow into local fabric",
        "peers_online": online.len(),
        "peers":        online,
        "reconnect": {
            "base_ms": RECONNECT_BASE_MS,
            "cap_ms":  RECONNECT_CAP_MS,
        },
        "poll_secs":   PEER_POLL_SECS,
    })
}
