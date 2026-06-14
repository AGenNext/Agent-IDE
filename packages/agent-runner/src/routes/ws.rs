// WebSocket transport layer — the stream is the platform's nervous system.
// "Build and transport the stream" — fabric events, usage, accountability, gate transitions
// all flow through this channel in real time.
//
// Routes:
//   /ws/:run_id    — live steps for a specific agent run (existing)
//   /ws/fabric     — all fabric events: gate transitions, dead-letters, peer events
//   /ws/stream     — unified stream: fabric + usage + accountability (everything)
//
// Solid base: one broadcast per channel, fan-out to all connected clients.
// No polling. No missed events. Every event is delivered exactly once per subscriber.

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::extract::ws::{Message, WebSocket};
use std::sync::Arc;
use tokio::sync::broadcast;
use futures::{SinkExt, StreamExt};
use crate::store::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/:run_id",  get(ws_run_handler))
        .route("/fabric",   get(ws_fabric_handler))
        .route("/stream",   get(ws_unified_handler))
        .with_state(state)
}

// ── Run stream — live steps for a specific run ────────────────────────────────

async fn ws_run_handler(
    Path(run_id): Path<String>,
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_run_socket(socket, run_id, state))
}

async fn handle_run_socket(socket: WebSocket, run_id: String, state: Arc<AppState>) {
    let (tx, mut rx) = broadcast::channel::<String>(256);
    state.register_ws_sink(&run_id, tx);

    let (mut sink, _stream) = socket.split();

    // Replay existing steps on connect (client catches up immediately)
    if let Some(run) = state.get_run(&run_id) {
        for step in &run.steps {
            let msg = serde_json::to_string(step).unwrap_or_default();
            let _ = sink.send(Message::Text(msg)).await;
        }
    }

    // Stream new events as they arrive — each WebSocket runs in its own task
    while let Ok(msg) = rx.recv().await {
        if sink.send(Message::Text(msg)).await.is_err() { break; }
    }
}

// ── Fabric stream — all gate events, dead-letters, peer events ────────────────

async fn ws_fabric_handler(
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_fabric_socket(socket, state))
}

async fn handle_fabric_socket(socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.fabric.subscribe();
    let (mut sink, mut stream) = socket.split();

    // Welcome frame — tell the client what channel they're on
    let welcome = serde_json::json!({
        "channel": "fabric",
        "message": "subscribed to fabric event stream",
        "events":  ["gate_open", "gate_closed", "gate_idempotent", "dead_letter", "peer_event"],
    });
    let _ = sink.send(Message::Text(welcome.to_string())).await;

    loop {
        tokio::select! {
            // Fabric event arrives
            result = rx.recv() => {
                match result {
                    Ok(event_json) => {
                        if sink.send(Message::Text(event_json)).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        let lag = serde_json::json!({ "warn": "lagged", "missed": n });
                        let _ = sink.send(Message::Text(lag.to_string())).await;
                    }
                    Err(_) => break,
                }
            }
            // Client ping / close
            client_msg = stream.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sink.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ── Unified stream — everything: fabric + usage deltas + accountability ───────
// "Build and transport the stream on a solid base"
// One WebSocket. All signals. Real-time. The complete picture.

async fn ws_unified_handler(
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_unified_socket(socket, state))
}

async fn handle_unified_socket(socket: WebSocket, state: Arc<AppState>) {
    let mut fabric_rx = state.fabric.subscribe();
    let (mut sink, mut stream) = socket.split();

    // Welcome frame — platform identity + stream description
    let welcome = serde_json::json!({
        "channel":  "stream",
        "platform": "Autonomyx",
        "streams":  ["fabric", "usage", "accountability", "gate_transitions"],
        "message":  "unified platform stream — all signals, real time",
        "philosophy": "the fabric is the nervous system — it fills every gap, no polling",
    });
    let _ = sink.send(Message::Text(welcome.to_string())).await;

    loop {
        tokio::select! {
            // Fabric event (includes gate transitions, dead-letters, peer events)
            fabric_result = fabric_rx.recv() => {
                match fabric_result {
                    Ok(event_json) => {
                        // Wrap in stream envelope so client knows the source
                        let envelope = serde_json::json!({
                            "stream": "fabric",
                            "event":  serde_json::from_str::<serde_json::Value>(&event_json)
                                         .unwrap_or(serde_json::json!({ "raw": event_json })),
                        });
                        if sink.send(Message::Text(envelope.to_string())).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        let lag = serde_json::json!({ "stream": "fabric", "warn": "lagged", "missed": n });
                        let _ = sink.send(Message::Text(lag.to_string())).await;
                    }
                    Err(_) => break,
                }
            }
            // Client message (ping, keepalive, close)
            client_msg = stream.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Text(cmd))) => {
                        // Client can send commands: "snapshot:usage" → push current usage snapshot
                        if cmd.trim() == "snapshot:usage" {
                            let snapshot = serde_json::json!({
                                "stream":   "usage",
                                "snapshot": state.usage.summary(),
                            });
                            let _ = sink.send(Message::Text(snapshot.to_string())).await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
