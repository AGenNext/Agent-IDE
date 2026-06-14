// Feedback gate — the loop that makes the platform a living system.
//
// "Everything is possible" because the platform learns from every run.
// Every agent execution produces signal. Signal flows back through this gate.
// The gate fires a SurrealDB live query. Subscribed agents update their behaviour.
// Updated behaviour flows to the next build. Loop repeats.
//
// "Or money to buy" — usage is metered, cost is transparent, budget is enforced.
// The feedback gate records the cost of every run and settles it instantly.
// No hidden fees. No platform tax. Freedom, not free.
//
// POST /api/feedback          — submit run outcome, trigger feedback gate
// GET  /api/feedback/:run_id  — get feedback for a run
// GET  /api/feedback/loop     — show the full feedback loop state
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use crate::store::AppState;
use crate::lifecycle::{Gate, Stage};

#[derive(Debug, Deserialize, Serialize)]
pub struct FeedbackRequest {
    pub run_id:      String,
    pub agent_id:    String,
    pub actor_did:   Option<String>,
    pub outcome:     FeedbackOutcome,
    pub signal:      Value,          // structured signal from the run
    pub cost_usd:    Option<f64>,    // actual cost if known
    pub tokens_in:   Option<u64>,
    pub tokens_out:  Option<u64>,
    pub compute_ms:  Option<u64>,
    pub improvement: Option<String>, // what should change for the next run?
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackOutcome {
    Success,
    PartialSuccess,
    Failure,
    Cancelled,
}

async fn submit_feedback(
    State(state): State<Arc<AppState>>,
    Json(req): Json<FeedbackRequest>,
) -> Json<Value> {
    let actor_did = req.actor_did.as_deref().unwrap_or("did:autonomyx:platform");

    // 1. Close the run's lifecycle with the feedback gate
    let gate = Gate::new(&state.lifecycle, &req.run_id);
    let payload = json!({
        "run_id":      req.run_id,
        "agent_id":    req.agent_id,
        "outcome":     req.outcome,
        "signal":      req.signal,
        "actor_did":   actor_did,
        "tokens_in":   req.tokens_in.unwrap_or(0),
        "tokens_out":  req.tokens_out.unwrap_or(0),
        "compute_ms":  req.compute_ms.unwrap_or(0),
        "cost_usd":    req.cost_usd.unwrap_or(0.0),
    });
    let rec = gate.feedback(&payload);

    // 2. Emit fabric event — SurrealDB live query picks this up,
    //    pushes to all subscribed agents, they update their behaviour.
    //    This is the living system: signal → agents → next build → loop.
    state.fabric.emit_gate(&rec, payload.clone());

    // 3. Settle usage — real-time liquid cost recorded at the feedback gate.
    //    Every run's cost is settled here. No batch billing. No surprise invoices.
    if let Some(cost) = req.cost_usd {
        let tokens_in  = req.tokens_in.unwrap_or(0);
        let tokens_out = req.tokens_out.unwrap_or(0);
        let cost_mc    = (cost * 1_000_000.0) as i64;  // USD → micro-cents
        let usage_rec  = crate::usage::UsageRecord {
            id:                    Uuid::new_v4().to_string(),
            did:                   actor_did.to_string(),
            artifact:              req.run_id.clone(),
            stage:                 Stage::Feedback,
            provider:              "settled".into(),
            model:                 "feedback_gate".into(),
            run_id:                Some(req.run_id.clone()),
            grant_id:              None,
            tokens_in,
            tokens_out,
            compute_ms:            req.compute_ms.unwrap_or(0),
            storage_bytes:         0,
            egress_bytes:          0,
            token_cost_usd_mc:     cost_mc,
            compute_cost_usd_mc:   0,
            storage_cost_usd_mc:   0,
            egress_cost_usd_mc:    0,
            total_cost_usd_mc:     cost_mc,
            recorded_at:           Utc::now(),
        };
        state.usage.record(usage_rec);
    }

    // 4. Record accountability — agent is accountable for this run's outcome.
    //    Non-repudiable. The feedback is signed and stored.
    {
        use crate::identity::AgentIdentity;
        use crate::federation::ActionOutcome;
        let outcome_mapped = match req.outcome {
            FeedbackOutcome::Success        => ActionOutcome::Success,
            FeedbackOutcome::PartialSuccess => ActionOutcome::Partial,
            FeedbackOutcome::Failure        => ActionOutcome::Failed,
            FeedbackOutcome::Cancelled      => ActionOutcome::Denied,
        };
        let identity = AgentIdentity::from_did(actor_did);
        state.federation.record(
            &identity,
            "run:feedback",
            &req.run_id,
            None,
            outcome_mapped,
            payload.clone(),
        );
    }

    // 5. If there's an improvement signal, broadcast it to all agents
    //    subscribed to this agent's feedback channel via WebSocket.
    if let Some(improvement) = &req.improvement {
        let broadcast_msg = json!({
            "type":        "feedback.improvement",
            "agent_id":    req.agent_id,
            "run_id":      req.run_id,
            "improvement": improvement,
            "signal":      req.signal,
            "at":          Utc::now(),
        });
        state.broadcast_to_run(&req.run_id, &broadcast_msg.to_string());
    }

    Json(json!({
        "run_id":     req.run_id,
        "gate":       "feedback",
        "status":     rec.status,
        "oath":       rec.oath,
        "settled":    req.cost_usd.unwrap_or(0.0),
        "loop":       "signal received → fabric → live query → agents update → next build",
        "message":    "The loop is closed. The platform learned. Everything is possible.",
    }))
}

async fn get_feedback(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> Json<Value> {
    let log     = state.lifecycle.log_for(&run_id);
    let usage   = state.usage.records_for(&run_id);
    let cost: f64 = usage.iter().map(|r| r.total_usd()).sum();

    let feedback_gate = log.iter()
        .find(|r| r.stage == Stage::Feedback)
        .cloned();

    Json(json!({
        "run_id":       run_id,
        "lifecycle":    log.iter().map(|r| json!({
            "stage": r.stage.as_str(), "status": r.status, "oath": r.oath
        })).collect::<Vec<_>>(),
        "feedback_gate": feedback_gate.map(|r| json!({
            "status": r.status, "detail": r.detail, "at": r.transitioned_at
        })),
        "cost_usd":     cost,
        "usage_records": usage.len(),
    }))
}

async fn feedback_loop_state(State(state): State<Arc<AppState>>) -> Json<Value> {
    let summary = state.usage.summary();
    let runs    = state.list_runs();
    let apps    = state.list_apps();

    let completed = runs.iter().filter(|r| r.status == crate::store::RunStatus::Completed).count();
    let failed    = runs.iter().filter(|r| r.status == crate::store::RunStatus::Failed).count();
    let live_apps = apps.iter().filter(|a| a.status == crate::store::AppStatus::Live).count();

    Json(json!({
        "loop": {
            "description": "Every run produces signal. Signal flows back. Agents improve. Loop repeats.",
            "steps": [
                "agent run produces signal",
                "signal flows to feedback gate",
                "gate fires SurrealDB live query",
                "live query reaches subscribed agents",
                "agents update behaviour",
                "updated behaviour flows to next build",
                "build produces new BOM-carrying artifact",
                "artifact signed, pushed, synced, deployed",
                "new agents run with updated intelligence",
                "loop repeats"
            ]
        },
        "state": {
            "total_runs":      runs.len(),
            "completed_runs":  completed,
            "failed_runs":     failed,
            "live_apps":       live_apps,
            "total_apps":      apps.len(),
        },
        "usage": summary,
        "philosophy": {
            "freedom_not_free": "You pay for what you use. Nothing hidden. Nothing marked up.",
            "or_money_to_buy":  "The platform earns trust through transparency, not extraction.",
            "self_hosted":      "$0 token cost — your compute, your agents, your data",
        },
        "everything_is_possible": true,
    }))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/feedback",          post(submit_feedback))
        .route("/feedback/loop",     get(feedback_loop_state))
        .route("/feedback/:run_id",  get(get_feedback))
        .with_state(state)
}
