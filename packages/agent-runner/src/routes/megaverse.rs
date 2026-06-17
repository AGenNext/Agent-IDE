// Megaverse routes — the unified world model API.
//
// GET  /api/megaverse              — summary: node counts, trust, federation
// GET  /api/megaverse/graph        — full graph: all nodes + edges
// GET  /api/megaverse/query        — ?q=&kind=&limit=  search the megaverse
// GET  /api/megaverse/node/:id     — single node + its neighbors
// GET  /api/megaverse/path         — ?from=&to=  shortest path between nodes
// POST /api/megaverse/index        — trigger full re-index now
// WS   /ws/megaverse               — live node updates via fabric stream

use axum::{routing::{get, post}, extract::{State, Query, Path}, Json, Router};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::megaverse;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/megaverse",            get(summary))
        .route("/megaverse/graph",      get(graph))
        .route("/megaverse/query",      get(query))
        .route("/megaverse/node/:id",   get(node_detail))
        .route("/megaverse/path",       get(path_query))
        .route("/megaverse/index",      post(reindex))
        .with_state(state)
}

async fn summary(State(s): State<Arc<AppState>>) -> Json<Value> {
    megaverse::index(&s.megaverse, &s);
    Json(json!({
        "megaverse": s.megaverse.summary(),
        "description": "Unified world model — every entity, every relationship, every surface",
        "live":  true,
        "ws":    "/ws/megaverse",
    }))
}

async fn graph(State(s): State<Arc<AppState>>) -> Json<Value> {
    megaverse::index(&s.megaverse, &s);
    Json(json!({
        "nodes": s.megaverse.all_nodes(),
        "edges": s.megaverse.all_edges(),
    }))
}

#[derive(Deserialize)]
struct QueryParams {
    q:     Option<String>,
    kind:  Option<String>,
    limit: Option<usize>,
}

async fn query(State(s): State<Arc<AppState>>, Query(p): Query<QueryParams>) -> Json<Value> {
    megaverse::index(&s.megaverse, &s);
    let results = s.megaverse.query(
        p.q.as_deref().unwrap_or(""),
        p.kind.as_deref(),
        p.limit.unwrap_or(50),
    );
    Json(json!({ "results": results, "count": results.len() }))
}

async fn node_detail(State(s): State<Arc<AppState>>, Path(raw_id): Path<String>) -> Json<Value> {
    megaverse::index(&s.megaverse, &s);
    let id = raw_id.replace('_', ":");  // allow /node/agent_demo as alias for agent:demo
    match s.megaverse.get(&id) {
        Some(node) => {
            let neighbors = s.megaverse.neighbors(&id);
            Json(json!({
                "node":      node,
                "neighbors": neighbors.iter().map(|(kind, n)| json!({
                    "edge_kind": kind,
                    "node": n,
                })).collect::<Vec<_>>(),
            }))
        }
        None => Json(json!({ "error": "node not found", "id": id })),
    }
}

#[derive(Deserialize)]
struct PathParams { from: String, to: String }

async fn path_query(State(s): State<Arc<AppState>>, Query(p): Query<PathParams>) -> Json<Value> {
    megaverse::index(&s.megaverse, &s);
    match s.megaverse.path(&p.from, &p.to) {
        Some(path) => {
            let nodes: Vec<_> = path.iter()
                .filter_map(|id| s.megaverse.get(id))
                .collect();
            Json(json!({ "found": true, "hops": path.len() - 1, "path": nodes }))
        }
        None => Json(json!({ "found": false, "from": p.from, "to": p.to })),
    }
}

async fn reindex(State(s): State<Arc<AppState>>) -> Json<Value> {
    megaverse::index(&s.megaverse, &s);
    Json(json!({ "ok": true, "summary": s.megaverse.summary() }))
}
