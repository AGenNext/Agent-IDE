use axum::{Router, routing::get, extract::{State, Query}, Json};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::search::{SearchQuery, search};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/search", get(search_handler))
        .with_state(state)
}

#[derive(Deserialize)]
struct SearchParams {
    q:     Option<String>,
    limit: Option<usize>,
    kinds: Option<String>,   // comma-separated: agent,goal,run,plugin,event,record,node,dashboard
}

async fn search_handler(
    State(s): State<Arc<AppState>>,
    Query(p): Query<SearchParams>,
) -> Json<Value> {
    let q = match p.q {
        Some(q) if !q.trim().is_empty() => q.trim().to_string(),
        _ => return Json(json!({ "results": [], "total": 0, "query": "" })),
    };
    let limit = p.limit.unwrap_or(50).min(200);
    let kinds = p.kinds.map(|k| k.split(',').map(|s| s.trim().to_string()).collect());

    let sq = SearchQuery { q: q.clone(), limit, kinds };
    let results = search(sq, s).await;
    let total = results.len();

    Json(json!({
        "query":   q,
        "total":   total,
        "limit":   limit,
        "results": results,
    }))
}
