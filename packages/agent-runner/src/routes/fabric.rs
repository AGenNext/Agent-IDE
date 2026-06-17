// Fabric routes — the thread of the platform.
//
// GET /api/fabric/log                  — full event log (last 10k events)
// GET /api/fabric/recent?n=50          — latest N events
// GET /api/fabric/dead                 — dead-letter log
// GET /api/fabric/stats                — event counts by stage
// GET /api/fabric/:artifact/events     — all events for an artifact
// GET /api/fabric/thread/:entity_id    — pull the thread for any entity
//                                        matches artifact, entity tags, payload
// WS  /ws/fabric                       — live fabric stream (see routes/ws.rs)
// WS  /ws/stream                       — unified platform stream

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::store::AppState;
use crate::fabric::FabricEvent;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/fabric/log",               get(event_log))
        .route("/fabric/recent",            get(recent_events))
        .route("/fabric/dead",              get(dead_log))
        .route("/fabric/stats",             get(fabric_stats))
        .route("/fabric/thread/:entity_id", get(thread))
        .route("/fabric/:artifact/events",  get(artifact_events))
        .with_state(state)
}

async fn event_log(State(s): State<Arc<AppState>>) -> Json<Vec<FabricEvent>> {
    Json(s.fabric.full_log())
}

#[derive(Deserialize)]
struct RecentParams { n: Option<usize> }

async fn recent_events(State(s): State<Arc<AppState>>, Query(p): Query<RecentParams>) -> Json<Value> {
    let events = s.fabric.recent(p.n.unwrap_or(50));
    Json(json!({ "events": events, "count": events.len() }))
}

async fn dead_log(State(s): State<Arc<AppState>>) -> Json<Vec<FabricEvent>> {
    Json(s.fabric.dead_log())
}

async fn fabric_stats(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "fabric": s.fabric.stats(),
        "description": "fabric is the thread — every event, every surface, every stitch",
    }))
}

/// Pull the full thread for any entity_id.
/// Every fabric event that touches this entity — by artifact, tag, or payload.
/// This is the audit trail, the provenance, the history of any node in the megaverse.
async fn thread(
    State(s): State<Arc<AppState>>,
    Path(entity_id): Path<String>,
) -> Json<Value> {
    let events = s.fabric.thread(&entity_id);
    let count  = events.len();

    // Build thread summary: stages touched, first/last event, status spread
    let first = events.first().map(|e| e.emitted_at);
    let last  = events.last().map(|e| e.emitted_at);
    let stages: std::collections::HashSet<&str> = events.iter()
        .map(|e| e.stage.as_str()).collect();

    Json(json!({
        "entity_id": entity_id,
        "thread_length": count,
        "first_seen":    first,
        "last_seen":     last,
        "stages_touched": stages,
        "events": events,
        "description": "fabric thread — the complete history of this entity across all surfaces",
    }))
}

async fn artifact_events(
    State(s): State<Arc<AppState>>,
    Path(artifact): Path<String>,
) -> Json<Vec<FabricEvent>> {
    Json(s.fabric.log_for(&artifact))
}
