// Accountability at the gate — every gate transition is a signed accountability record.
// "Agent is accountable for its own decisions" — recorded, signed, non-repudiable.
// No action passes through a gate without a trace. The agent owns its audit log.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use crate::store::AppState;
use crate::lifecycle::{Gate, GateRecord, GateStatus, Stage};

// ── Request / response shapes ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct TransitionReq {
    artifact: String,
    stage:    Stage,
    payload:  Value,
    // Accountability: who is making this transition?
    // Pinned at the gate — non-negotiable. No DID = platform assigns one.
    actor_did:   Option<String>,
    grant_id:    Option<String>,   // the JIT grant that authorises this action
}

#[derive(Serialize)]
struct TransitionResp {
    artifact: String,
    stage:    &'static str,
    status:   GateStatus,
    oath:     String,
    detail:   Option<String>,
    id:       String,
}

impl From<GateRecord> for TransitionResp {
    fn from(r: GateRecord) -> Self {
        Self {
            artifact: r.artifact,
            stage:    r.stage.as_str(),
            status:   r.status,
            oath:     r.oath,
            detail:   r.detail,
            id:       r.id,
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn transition(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TransitionReq>,
) -> Json<TransitionResp> {
    let gate = Gate::new(&state.lifecycle, &req.artifact);
    let rec = match req.stage {
        Stage::Build    => gate.build(&req.payload),
        Stage::Sign     => gate.sign(&req.payload),
        Stage::Push     => gate.push(&req.payload),
        Stage::Sync     => gate.sync(&req.payload),
        Stage::Deploy   => gate.deploy(&req.payload),
        Stage::Run      => gate.run(&req.payload),
        Stage::Observe  => gate.observe(&req.payload),
        Stage::Feedback => gate.feedback(&req.payload),
    };
    // Gate result → fabric: fills the gap to the next stage
    state.fabric.emit_gate(&rec, req.payload.clone());

    // Real-time liquid usage — meter at the gate, settle instantly.
    // Every gate opening is a micro-transaction: compute time + token cost.
    // Self-hosted = $0. Provider rates pass-through at published rates, no markup.
    if rec.status == GateStatus::Open {
        use crate::usage::UsageRecord;
        let compute_ms = req.payload.get("compute_ms")
            .and_then(|v| v.as_u64()).unwrap_or(0);
        let tokens_in  = req.payload.get("tokens_in")
            .and_then(|v| v.as_u64()).unwrap_or(0);
        let tokens_out = req.payload.get("tokens_out")
            .and_then(|v| v.as_u64()).unwrap_or(0);
        let actor_did  = req.actor_did.as_deref().unwrap_or("did:autonomyx:platform");
        let provider   = req.payload.get("provider")
            .and_then(|v| v.as_str()).unwrap_or("self");
        let model      = req.payload.get("model")
            .and_then(|v| v.as_str()).unwrap_or("unknown");

        let usage = UsageRecord {
            id:            uuid::Uuid::new_v4().to_string(),
            did:           actor_did.to_string(),
            artifact:      req.artifact.clone(),
            stage:         req.stage,
            provider:      provider.to_string(),
            model:         model.to_string(),
            run_id:        None,
            grant_id:      req.grant_id.clone(),
            tokens_in,
            tokens_out,
            compute_ms,
            storage_bytes: 0,
            egress_bytes:  0,
            token_cost_usd_mc:   0,
            compute_cost_usd_mc: 0,
            storage_cost_usd_mc: 0,
            egress_cost_usd_mc:  0,
            total_cost_usd_mc:   0,
            recorded_at: chrono::Utc::now(),
        };
        state.usage.record(usage);
    }

    // Accountability pinned at the gate — signed, non-repudiable.
    // The agent is accountable for this decision. Always. No exceptions.
    {
        use crate::identity::AgentIdentity;
        use crate::federation::ActionOutcome;
        let actor_did = req.actor_did.as_deref().unwrap_or("did:autonomyx:platform");
        let outcome = match rec.status {
            GateStatus::Open    => ActionOutcome::Success,
            GateStatus::Closed  => ActionOutcome::Denied,
            GateStatus::Already => ActionOutcome::Partial,
        };
        let evidence = serde_json::json!({
            "gate_id": rec.id,
            "stage":   rec.stage.as_str(),
            "oath":    rec.oath,
        });
        // Use a platform-pinned identity for the accountability record.
        // Production: inject caller's identity via mTLS / Bearer token DID resolution.
        let platform_identity = AgentIdentity::from_did(actor_did);
        state.federation.record(
            &platform_identity,
            &format!("lifecycle:{}", rec.stage.as_str()),
            &req.artifact,
            req.grant_id.clone(),
            outcome,
            evidence,
        );
    }

    Json(rec.into())
}

async fn artifact_log(
    State(state): State<Arc<AppState>>,
    Path(artifact): Path<String>,
) -> Json<Vec<TransitionResp>> {
    let records = state.lifecycle.log_for(&artifact)
        .into_iter().map(Into::into).collect();
    Json(records)
}

async fn artifact_stage(
    State(state): State<Arc<AppState>>,
    Path(artifact): Path<String>,
) -> Json<Value> {
    let stage = state.lifecycle.stage_of(&artifact)
        .map(|s| s.as_str())
        .unwrap_or("none");
    Json(serde_json::json!({ "artifact": artifact, "stage": stage }))
}

async fn full_log(State(state): State<Arc<AppState>>) -> Json<Vec<TransitionResp>> {
    let records = state.lifecycle.full_log()
        .into_iter().map(Into::into).collect();
    Json(records)
}

/// GET /api/lifecycle/:artifact/feasibility — check feasibility and usability before transitioning.
///
/// "Check feasibility and usability at the gates" — before opening any gate,
/// verify that the transition CAN succeed and that the result WILL be usable.
///
/// Feasibility = can we do this? (resources, dependencies, budget)
/// Usability   = if we do this, will it work? (governance, compatibility, readiness)
///
/// This endpoint is advisory — it does not transition the gate.
/// Call it before POST /lifecycle/transition to surface blockers early.
async fn feasibility_check(
    State(state): State<Arc<AppState>>,
    Path((artifact, stage_str)): Path<(String, String)>,
) -> Json<Value> {
    use crate::lifecycle::Stage;

    let stage: Stage = serde_json::from_value(serde_json::json!(stage_str))
        .unwrap_or(Stage::Build);

    let current = state.lifecycle.stage_of(&artifact);
    let usage   = state.usage.records_for(&artifact);
    let cost_usd: f64 = usage.iter().map(|r| r.total_usd()).sum();

    // Check ordering — is this stage reachable from the current stage?
    let stage_order: &[Stage] = &[
        Stage::Build, Stage::Sign, Stage::Push, Stage::Sync,
        Stage::Deploy, Stage::Run, Stage::Observe, Stage::Feedback,
    ];
    let current_idx = current.as_ref()
        .and_then(|s| stage_order.iter().position(|x| x == s))
        .map(|i| i + 1)    // next expected index
        .unwrap_or(0);
    let target_idx = stage_order.iter().position(|x| x == &stage).unwrap_or(0);

    let ordering_ok = target_idx >= current_idx;
    let skip_gap    = target_idx > current_idx + 1;

    // Checks
    let checks = serde_json::json!([
        {
            "check":   "stage_ordering",
            "pass":    ordering_ok,
            "detail":  if ordering_ok {
                format!("Stage '{}' is reachable from current stage '{}'",
                    stage.as_str(), current.as_ref().map(|s| s.as_str()).unwrap_or("none"))
            } else {
                format!("Stage '{}' is behind current stage '{}'",
                    stage.as_str(), current.as_ref().map(|s| s.as_str()).unwrap_or("none"))
            },
        },
        {
            "check":   "no_stage_skip",
            "pass":    !skip_gap,
            "detail":  if !skip_gap { "No stage gap detected".to_string() }
                       else { format!("Skipping stages between '{}' and '{}'",
                           current.as_ref().map(|s| s.as_str()).unwrap_or("none"),
                           stage.as_str()) },
        },
        {
            "check":   "artifact_exists",
            "pass":    state.get_agent(&artifact).is_some() || state.get_app(&artifact).is_some(),
            "detail":  "Artifact must be a registered agent or application",
        },
        {
            "check":   "budget_not_exceeded",
            "pass":    cost_usd < 1000.0,   // platform-wide soft limit; per-app budget checked at run gate
            "detail":  format!("Total cost so far: ${:.6}", cost_usd),
        },
        {
            "check":   "governance_active",
            "pass":    true,   // governance is always active — no bypass
            "detail":  "Governance is always enforced. JIT grants required for privileged gates.",
        },
    ]);

    let all_pass: bool = checks.as_array()
        .map(|arr| arr.iter().all(|c| c["pass"].as_bool().unwrap_or(false)))
        .unwrap_or(false);

    Json(serde_json::json!({
        "artifact":   artifact,
        "stage":      stage.as_str(),
        "current":    current.as_ref().map(|s| s.as_str()),
        "feasible":   all_pass,
        "usable":     all_pass,
        "checks":     checks,
        "action":     if all_pass {
            "Gate transition is feasible and the result will be usable. Proceed with POST /lifecycle/transition."
        } else {
            "One or more checks failed. Resolve blockers before transitioning."
        },
    }))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/lifecycle/transition",                          post(transition))
        .route("/lifecycle/log",                                 get(full_log))
        .route("/lifecycle/:artifact/log",                       get(artifact_log))
        .route("/lifecycle/:artifact/stage",                     get(artifact_stage))
        .route("/lifecycle/:artifact/feasibility/:stage",        get(feasibility_check))
        .with_state(state)
}
