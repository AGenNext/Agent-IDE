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

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/lifecycle/transition",       post(transition))
        .route("/lifecycle/log",              get(full_log))
        .route("/lifecycle/:artifact/log",    get(artifact_log))
        .route("/lifecycle/:artifact/stage",  get(artifact_stage))
        .with_state(state)
}
