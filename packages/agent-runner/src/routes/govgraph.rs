// Governance Graph routes — the compute core wired to governance.
//
// GET  /api/govgraph              — full graph (nodes + edges)
// GET  /api/govgraph/summary      — graph health, trust averages, traversal counts
// POST /api/govgraph/nodes        — register a new node
// GET  /api/govgraph/nodes/:id    — get a node
// POST /api/govgraph/edges        — add a governed edge
// GET  /api/govgraph/path/:from/:to — check if a governed path exists
// POST /api/govgraph/execute      — execute a path through the graph
// GET  /api/govgraph/executions   — execution history
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;
use crate::govgraph::{EdgeCondition, GovernanceNode, NodeKind, NodePolicy};

#[derive(Deserialize)]
struct AddNodeReq {
    id:           String,
    kind:         Option<String>,
    label:        String,
    description:  Option<String>,
    did:          Option<String>,
    capabilities: Option<Vec<String>>,
    requires:     Option<Vec<String>>,
}

#[derive(Deserialize)]
struct AddEdgeReq {
    from:       String,
    to:         String,
    capability: String,
    label:      Option<String>,
    milestone:  Option<String>,
    trust_min:  Option<f64>,
    weight:     Option<f64>,
}

#[derive(Deserialize)]
struct ExecuteReq {
    from:       String,
    to:         String,
    actor_did:  Option<String>,
    input:      Option<Value>,
    milestone:  Option<String>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/govgraph",              get(get_graph))
        .route("/govgraph/summary",      get(graph_summary))
        .route("/govgraph/nodes",        get(list_nodes).post(add_node))
        .route("/govgraph/nodes/:id",    get(get_node))
        .route("/govgraph/edges",        get(list_edges).post(add_edge))
        .route("/govgraph/path/:from/:to", get(check_path))
        .route("/govgraph/execute",      post(execute_path))
        .route("/govgraph/executions",   get(list_executions))
        .with_state(state)
}

async fn get_graph(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(state.govgraph.to_graph_json())
}

async fn graph_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(state.govgraph.summary())
}

async fn list_nodes(State(state): State<Arc<AppState>>) -> Json<Value> {
    let nodes = state.govgraph.list_nodes();
    Json(json!({ "nodes": nodes, "count": nodes.len() }))
}

async fn add_node(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddNodeReq>,
) -> Json<Value> {
    let kind = match req.kind.as_deref() {
        Some("tool")    => NodeKind::Tool,
        Some("storage") => NodeKind::Storage,
        Some("api")     => NodeKind::Api,
        Some("sensor")  => NodeKind::Sensor,
        Some("oracle")  => NodeKind::Oracle,
        Some("human")   => NodeKind::Human,
        Some("sink")    => NodeKind::Sink,
        Some("source")  => NodeKind::Source,
        _               => NodeKind::Agent,
    };
    let node = GovernanceNode {
        id:           req.id,
        kind,
        label:        req.label,
        description:  req.description.unwrap_or_default(),
        did:          req.did,
        capabilities: req.capabilities.unwrap_or_default(),
        requires:     req.requires.unwrap_or_default(),
        trust_score:  0.5,
        policy:       NodePolicy::default(),
        metadata:     json!({}),
        created_at:   chrono::Utc::now(),
        updated_at:   chrono::Utc::now(),
    };
    let n = state.govgraph.add_node(node);
    Json(json!({ "node": n, "status": "registered" }))
}

async fn get_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.govgraph.get_node(&id) {
        Some(n) => Json(json!({ "node": n })),
        None    => Json(json!({ "error": "node not found", "id": id })),
    }
}

async fn list_edges(State(state): State<Arc<AppState>>) -> Json<Value> {
    let edges = state.govgraph.list_edges();
    Json(json!({ "edges": edges, "count": edges.len() }))
}

async fn add_edge(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddEdgeReq>,
) -> Json<Value> {
    let has_milestone = req.milestone.is_some();
    let condition = EdgeCondition {
        milestone: req.milestone,
        trust_min: req.trust_min.unwrap_or(0.0),
        budget_ok: true,
        always:    !has_milestone,
    };
    match state.govgraph.add_edge(
        &req.from, &req.to, &req.capability,
        req.label.as_deref().unwrap_or(&format!("{} → {}", req.from, req.to)),
        condition,
        req.weight.unwrap_or(1.0),
    ) {
        Ok(e)  => Json(json!({ "edge": e, "status": "added" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn check_path(
    State(state): State<Arc<AppState>>,
    Path((from, to)): Path<(String, String)>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let actor     = q.get("actor_did").map(|s| s.as_str()).unwrap_or("did:autonomyx:platform");
    let milestone = q.get("milestone").map(|s| s.as_str());
    let trust     = q.get("trust").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
    let budget    = q.get("budget").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1000.0);

    let check = state.govgraph.check_path(&from, &to, actor, milestone, trust, budget);
    Json(json!({
        "from":       from,
        "to":         to,
        "reachable":  check.reachable,
        "path":       check.path,
        "steps":      check.steps,
        "blocked_at": check.blocked_at,
        "action":     if check.reachable {
            "Path is governed and reachable. POST /api/govgraph/execute to run it."
        } else {
            "No governed path. Add edges or check capability grants."
        },
    }))
}

async fn execute_path(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExecuteReq>,
) -> Json<Value> {
    let actor     = req.actor_did.as_deref().unwrap_or("did:autonomyx:platform");
    let input     = req.input.unwrap_or(json!({}));
    let milestone = req.milestone.as_deref();

    let exec = state.govgraph.execute_path(&req.from, &req.to, actor, input, milestone);

    // Update trust scores based on outcome
    if exec.status == crate::govgraph::ExecStatus::Completed {
        for step in &exec.steps {
            state.govgraph.update_trust(&step.from, true);
            state.govgraph.update_trust(&step.to, true);
        }
        // Emit fabric event for graph execution
        state.fabric.emit(crate::fabric::FabricEvent {
            id:         exec.id.clone(),
            artifact:   req.from.clone(),
            stage:      crate::lifecycle::Stage::Run,
            status:     crate::fabric::FabricStatus::Open,
            payload:    json!({
                "type":   "govgraph.execution",
                "from":   req.from,
                "to":     req.to,
                "status": exec.status,
                "path":   exec.path,
            }),
            emitted_at: chrono::Utc::now(),
        });
    }

    Json(json!({ "execution": exec }))
}

async fn list_executions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let graph   = state.govgraph.to_graph_json();
    let summary = state.govgraph.summary();
    Json(json!({
        "summary":    summary,
        "graph":      graph,
    }))
}
