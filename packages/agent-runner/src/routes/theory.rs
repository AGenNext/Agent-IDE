// Theory routes — verified theoretical foundations, queryable at runtime.
//
// GET /api/theory         — full alignment report with runtime verification
// GET /api/theory/map     — all theory-to-code mappings
// GET /api/theory/:domain — mappings filtered by domain
//                           domains: systems | ethics | distributed | organization |
//                                    information | security | graph | economics | linguistics | philosophy

use axum::{
    extract::{Path, State},
    routing::get,
    Router, Json,
};
use std::sync::Arc;
use serde_json::{json, Value};
use crate::store::AppState;
use crate::theory;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/theory",         get(report))
        .route("/theory/map",     get(map))
        .route("/theory/:domain", get(by_domain))
        .with_state(state)
}

async fn report(State(s): State<Arc<AppState>>) -> Json<Value> {
    let agents       = s.agents.read().unwrap().len();
    let runs         = s.runs.read().unwrap().len();
    let peers        = s.peers.read().unwrap().len();
    let fabric_count = s.fabric.full_log().len();
    Json(theory::report(agents, runs, peers, fabric_count))
}

async fn map(_: State<Arc<AppState>>) -> Json<Value> {
    let theories = theory::theory_map();
    Json(json!({
        "count":   theories.len(),
        "theories": theories,
    }))
}

async fn by_domain(
    _: State<Arc<AppState>>,
    Path(domain): Path<String>,
) -> Json<Value> {
    let d = domain.to_lowercase();
    let filtered: Vec<_> = theory::theory_map()
        .into_iter()
        .filter(|t| format!("{:?}", t.domain).to_lowercase() == d)
        .collect();

    Json(json!({
        "domain": domain,
        "count":  filtered.len(),
        "theories": filtered,
    }))
}
