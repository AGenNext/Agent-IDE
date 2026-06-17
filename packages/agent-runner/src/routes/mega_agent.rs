// MegaAgent routes — FSM-based automatic multi-agent orchestration.
// Aligned with MetaAgent (arXiv 2507.22606).

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;
use crate::mega_agent::{self, AgentType, OrchestrateRequest, MEGA_AGENT_ID};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/mega",                  get(identity))
        .route("/mega/orchestrate",      post(orchestrate))
        .route("/mega/design",           post(design))
        .route("/mega/agent-types",      get(agent_types))
        .with_state(state)
}

/// GET /api/mega — MegaAgent identity and capabilities
async fn identity(State(state): State<Arc<AppState>>) -> Json<Value> {
    let agent = state.get_agent(MEGA_AGENT_ID);
    Json(json!({
        "agent":    agent,
        "protocol": "FSM — MetaAgent (arXiv 2507.22606)",
        "features": {
            "fsm_auto_design":    "Given a goal, auto-designs a Finite State Machine",
            "fsm_optimize":       "LLM-merges redundant states before deployment",
            "null_transitions":   "Agent retries in current state when no condition met",
            "state_traceback":    "Can transition back to any prior state on error",
            "tool_use":           "Agents carry tool lists (shell, web_search, http_client)",
            "listener_memory":    "On transition, output inserted into listener agents' memory",
            "fabric_events":      "Every transition emits a FabricEvent",
            "accountability":     "Every FSM execution recorded in federation accountability",
        },
        "fsm_structures": {
            "linear":       "FSM with one transition per state — MetaGPT, ChatDev",
            "debate":       "FSM with limited traceback — LLM Debate",
            "orchestrator": "FSM with shared condition verifier — Magentic-One",
            "meta_agent":   "Full FSM: per-state verifier + null-transitions + traceback (this)",
        },
    }))
}

/// POST /api/mega/orchestrate — design + optimize + execute an FSM for a goal
async fn orchestrate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OrchestrateRequest>,
) -> Json<Value> {
    let resp = mega_agent::orchestrate(state, req).await;
    Json(serde_json::to_value(resp).unwrap_or_default())
}

/// GET /api/mega/agent-types — canonical agent type taxonomy (all 30 types, 6 tiers)
async fn agent_types() -> Json<Value> {
    let all = AgentType::all();
    let mut by_tier: std::collections::HashMap<&str, Vec<Value>> = std::collections::HashMap::new();
    for t in &all {
        by_tier.entry(t.tier_name()).or_default().push(json!({
            "type":  format!("{:?}", t),
            "tier":  t.tier(),
            "tools": t.tools(),
        }));
    }
    Json(json!({
        "total":    all.len(),
        "tiers": {
            "0_meta":       by_tier.get("Meta"),
            "1_execute":    by_tier.get("Execute"),
            "2_verify":     by_tier.get("Verify"),
            "3_synthesise": by_tier.get("Synthesise"),
            "4_govern":     by_tier.get("Govern"),
            "5_interface":  by_tier.get("Interface"),
        },
        "note": "Agent types are the canonical roles for FSM state assignment and team composition",
    }))
}

/// POST /api/mega/design — design + optimize an FSM without executing it
async fn design(
    State(_state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let goal   = body.get("goal").and_then(|v| v.as_str()).unwrap_or("general task");
    let opt    = body.get("optimize").and_then(|v| v.as_bool()).unwrap_or(true);

    let fsm = mega_agent::design_fsm(goal);
    let fsm = if opt { mega_agent::optimize_fsm(fsm) } else { fsm };

    Json(serde_json::to_value(fsm).unwrap_or_default())
}
