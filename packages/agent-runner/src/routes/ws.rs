use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::extract::ws::{Message, WebSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;
use futures::{SinkExt, StreamExt};
use crate::store::AppState;

// Hard cap on concurrent WebSocket connections — prevents resource exhaustion
const MAX_WS_CONNECTIONS: usize = 500;
static WS_CONN_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/:run_id",  get(ws_run_handler))
        .route("/fabric",   get(ws_fabric_handler))
        .route("/stream",   get(ws_unified_handler))
        .with_state(state)
}

fn acquire_connection() -> bool {
    let prev = WS_CONN_COUNT.fetch_add(1, Ordering::Relaxed);
    if prev >= MAX_WS_CONNECTIONS {
        WS_CONN_COUNT.fetch_sub(1, Ordering::Relaxed);
        false
    } else {
        true
    }
}

fn release_connection() {
    WS_CONN_COUNT.fetch_sub(1, Ordering::Relaxed);
}

// ── Run stream ────────────────────────────────────────────────────────────────

async fn ws_run_handler(
    Path(run_id): Path<String>,
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_run_socket(socket, run_id, state))
}

async fn handle_run_socket(socket: WebSocket, run_id: String, state: Arc<AppState>) {
    if !acquire_connection() {
        tracing::warn!("ws: connection limit reached — rejecting run socket");
        return;
    }
    let (tx, mut rx) = broadcast::channel::<String>(512);
    state.register_ws_sink(&run_id, tx);

    let (mut sink, _stream) = socket.split();

    if let Some(run) = state.get_run(&run_id) {
        for step in &run.steps {
            let msg = serde_json::to_string(step).unwrap_or_default();
            if sink.send(Message::Text(msg)).await.is_err() {
                release_connection();
                return;
            }
        }
    }

    while let Ok(msg) = rx.recv().await {
        if sink.send(Message::Text(msg)).await.is_err() { break; }
    }
    release_connection();
}


// ── Fabric stream ─────────────────────────────────────────────────────────────

async fn ws_fabric_handler(
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_fabric_socket(socket, state))
}

async fn handle_fabric_socket(socket: WebSocket, state: Arc<AppState>) {
    if !acquire_connection() {
        tracing::warn!("ws: connection limit reached — rejecting fabric socket");
        return;
    }

    let mut rx = state.fabric.subscribe();
    let (mut sink, mut stream) = socket.split();

    let welcome = serde_json::json!({
        "channel": "fabric",
        "message": "subscribed to fabric event stream",
        "events":  ["gate_open", "gate_closed", "gate_idempotent", "dead_letter"],
    });
    let _ = sink.send(Message::Text(welcome.to_string())).await;

    loop {
        tokio::select! {
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
            client_msg = stream.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => { let _ = sink.send(Message::Pong(data)).await; }
                    _ => {}
                }
            }
        }
    }
    release_connection();
}

// ── Unified stream ────────────────────────────────────────────────────────────

async fn ws_unified_handler(
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_unified_socket(socket, state))
}

async fn handle_unified_socket(socket: WebSocket, state: Arc<AppState>) {
    if !acquire_connection() {
        tracing::warn!("ws: connection limit reached — rejecting unified socket");
        return;
    }

    let mut fabric_rx = state.fabric.subscribe();
    let (mut sink, mut stream) = socket.split();

    let welcome = serde_json::json!({
        "channel":  "stream",
        "platform": "Autonomyx",
        "streams":  ["fabric", "usage", "accountability", "gate_transitions"],
        "message":  "unified platform stream — all signals, real time",
        "connections": {
            "current": WS_CONN_COUNT.load(Ordering::Relaxed),
            "max":     MAX_WS_CONNECTIONS,
        },
    });
    let _ = sink.send(Message::Text(welcome.to_string())).await;

    loop {
        tokio::select! {
            fabric_result = fabric_rx.recv() => {
                match fabric_result {
                    Ok(event_json) => {
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
            client_msg = stream.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => { let _ = sink.send(Message::Pong(data)).await; }
                    Some(Ok(Message::Text(cmd))) => {
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
    release_connection();
}
