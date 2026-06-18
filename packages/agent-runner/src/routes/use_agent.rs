// USEagent routes — Unified Software Engineering Agent (ICSE '26).
// Implements the USEagent framework: 6 task types, 7 SE actions, task state S=(Lc,Lt,Rexec,DS).

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;
use crate::use_agent::{
    execute_use_agent, UseAgentRequest, SeTaskType, SeAction,
};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/use-agent/run",        post(run_use_agent))
        .route("/use-agent/task-types", get(list_task_types))
        .route("/use-agent/actions",    get(list_actions))
        .with_state(state)
}

/// POST /api/use-agent/run — execute the USEagent on a software engineering task
async fn run_use_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UseAgentRequest>,
) -> Json<Value> {
    let response = execute_use_agent(state, req).await;
    Json(serde_json::to_value(response).unwrap_or_default())
}

/// GET /api/use-agent/task-types — list supported SE task types (USEbench)
async fn list_task_types() -> Json<Value> {
    let task_types = vec![
        json!({"variant": "program_repair",     "description": SeTaskType::ProgramRepair.description()}),
        json!({"variant": "regression_testing", "description": SeTaskType::RegressionTesting.description()}),
        json!({"variant": "code_generation",    "description": SeTaskType::CodeGeneration.description()}),
        json!({"variant": "test_generation",    "description": SeTaskType::TestGeneration.description()}),
        json!({"variant": "partial_fix",        "description": SeTaskType::PartialFix.description()}),
        json!({"variant": "feature_development","description": SeTaskType::FeatureDevelopment.description()}),
    ];
    Json(json!({"task_types": task_types}))
}

/// GET /api/use-agent/actions — list the 7 SE actions (Table 2)
async fn list_actions() -> Json<Value> {
    let actions: Vec<Value> = SeAction::all_variants()
        .into_iter()
        .map(|a| json!({"name": a.name(), "description": a.description()}))
        .collect();
    Json(json!({"actions": actions}))
}
