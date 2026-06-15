use axum::{Router, routing::{get, post}, extract::State, Json};
use std::sync::Arc;
use std::collections::HashMap;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::arithmetic::{eval_expr, platform_vars, compute_stats};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/arithmetic/eval",    post(eval_handler))
        .route("/arithmetic/vars",    get(vars_handler))
        .route("/arithmetic/stats",   get(stats_handler))
        .route("/arithmetic/formula", post(formula_handler))
        .with_state(state)
}

// POST /api/arithmetic/eval
// Body: { "expr": "runs.completed / runs.total * 100", "vars": { "x": 42 } }
#[derive(Deserialize)]
struct EvalReq {
    expr: String,
    vars: Option<HashMap<String, f64>>,
}

async fn eval_handler(
    State(s): State<Arc<AppState>>,
    Json(req): Json<EvalReq>,
) -> Json<Value> {
    let mut vars = platform_vars(&s);
    if let Some(extra) = req.vars {
        vars.extend(extra);
    }
    match eval_expr(&req.expr, &vars) {
        Ok(result) => Json(json!({
            "expr":   req.expr,
            "result": result,
            "ok":     true,
        })),
        Err(e) => Json(json!({ "expr": req.expr, "error": e, "ok": false })),
    }
}

// GET /api/arithmetic/vars — all platform variables available for expressions
async fn vars_handler(State(s): State<Arc<AppState>>) -> Json<Value> {
    let vars = platform_vars(&s);
    let mut sorted: Vec<(String, f64)> = vars.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    Json(json!({
        "vars":  sorted.into_iter().map(|(k, v)| json!({ "name": k, "value": v })).collect::<Vec<_>>(),
        "usage": "Use these variable names in POST /api/arithmetic/eval expressions",
    }))
}

// GET /api/arithmetic/stats — pre-computed platform statistics
async fn stats_handler(State(s): State<Arc<AppState>>) -> Json<Value> {
    let stats = compute_stats(&s);
    Json(json!(stats))
}

// POST /api/arithmetic/formula — run a named formula
// Body: { "formula": "success_rate" | "impact_efficiency" | "trust_health" | "cost_per_impact" | custom expr }
#[derive(Deserialize)]
struct FormulaReq { formula: String }

async fn formula_handler(
    State(s): State<Arc<AppState>>,
    Json(req): Json<FormulaReq>,
) -> Json<Value> {
    let vars = platform_vars(&s);

    // Named formulas
    let expr = match req.formula.as_str() {
        "success_rate"        => "runs.completed / (runs.completed + runs.failed + 0.001) * 100".to_string(),
        "failure_rate"        => "runs.failed / (runs.total + 0.001) * 100".to_string(),
        "alignment_rate"      => "(goals.active + goals.achieved) / (goals.total + 0.001) * 100".to_string(),
        "impact_efficiency"   => "goals.achieved / (runs.total + 0.001)".to_string(),
        "trust_health"        => "govgraph.avg_trust * 100".to_string(),
        "plugin_coverage"     => "plugins.enabled / (plugins.total + 0.001) * 100".to_string(),
        "fabric_health"       => "fabric.open / (fabric.events + 0.001) * 100".to_string(),
        "goal_pipeline"       => "goals.active / (goals.total + 0.001) * 100".to_string(),
        "peer_availability"   => "peers.online / (peers.total + 0.001) * 100".to_string(),
        other                 => other.to_string(),
    };

    match eval_expr(&expr, &vars) {
        Ok(result) => Json(json!({
            "formula": req.formula,
            "expr":    expr,
            "result":  result,
            "ok":      true,
        })),
        Err(e) => Json(json!({ "formula": req.formula, "error": e, "ok": false })),
    }
}
