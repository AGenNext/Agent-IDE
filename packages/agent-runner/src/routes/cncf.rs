// CNCF alignment routes — public read-only; no auth required for the report.
//
// GET /api/cncf          — full alignment report + gap analysis
// GET /api/cncf/map      — alignment map only (all 20+ mappings)
// GET /api/cncf/gaps     — identified gaps only (prioritized)

use axum::{routing::get, Router, Json};
use serde_json::Value;
use crate::cncf;

pub fn router() -> Router {
    Router::new()
        .route("/cncf",       get(report))
        .route("/cncf/map",   get(map))
        .route("/cncf/gaps",  get(gaps))
}

async fn report() -> Json<Value> {
    Json(cncf::report())
}

async fn map() -> Json<Value> {
    Json(serde_json::to_value(cncf::alignment_map()).unwrap_or_default())
}

async fn gaps() -> Json<Value> {
    let gaps = cncf::gaps();
    Json(serde_json::json!({
        "gaps":  gaps,
        "count": gaps.len(),
        "note":  "All gaps are on the Autonomyx roadmap. Priority: high = blocking enterprise adoption.",
    }))
}
