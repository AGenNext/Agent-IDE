// WebSocket layer — one broadcast channel per run_id.
// Clients connect to /ws/:run_id and receive live step events.

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
        .route("/:run_id", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    Path(run_id): Path<String>,
    State(state): State<Arc<AppState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_socket(socket, run_id, state))
}

async fn handle_socket(socket: WebSocket, run_id: String, state: Arc<AppState>) {
    let (tx, mut rx) = broadcast::channel::<String>(256);
    state.register_ws_sink(&run_id, tx);

    let (mut sink, _stream) = socket.split();

    // Replay existing steps
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
