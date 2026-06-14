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
