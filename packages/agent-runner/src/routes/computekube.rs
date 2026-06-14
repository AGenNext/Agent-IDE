// ComputeKube routes — Kubernetes execution backend for the governance graph.
//
// GET  /api/kube/summary         — cluster state, job counts, cost
// POST /api/kube/jobs            — spawn a governed k8s job
// GET  /api/kube/jobs            — list all jobs (with phase filter)
// GET  /api/kube/jobs/:id        — get job detail + status
// POST /api/kube/jobs/:id/sync   — sync status from k8s API
// GET  /api/kube/jobs/:id/logs   — fetch job logs
// DELETE /api/kube/jobs/:id      — cancel and delete job
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

#[derive(Deserialize)]
struct SpawnReq {
    agent_id:   String,
    run_id:     Option<String>,
    node_id:    Option<String>,
    capability: Option<String>,
    task:       String,
    did:        Option<String>,
    cpu_milli:  Option<u32>,
    mem_mb:     Option<u32>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/kube/summary",         get(kube_summary))
        .route("/kube/jobs",            get(list_jobs).post(spawn_job))
        .route("/kube/jobs/:id",        get(get_job).delete(cancel_job))
        .route("/kube/jobs/:id/sync",   post(sync_job))
        .route("/kube/jobs/:id/logs",   get(job_logs))
        .with_state(state)
}

async fn kube_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    let summary = state.computekube.summary();
    let govgraph_summary = state.govgraph.summary();
    Json(json!({
        "computekube": summary,
        "govgraph":    govgraph_summary,
        "integrated":  true,
        "flow": "govgraph.execute_path() → computekube.spawn() → k8s Job → governed workload",
    }))
}

async fn spawn_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SpawnReq>,
) -> Json<Value> {
    let run_id = req.run_id.unwrap_or_else(|| format!("run_{}", uuid::Uuid::new_v4().simple()));
    let node_id = req.node_id.as_deref().unwrap_or("agent:compute");
    let capability = req.capability.as_deref().unwrap_or("agent:run");

    let job = state.computekube.spawn(
        &req.agent_id,
        &run_id,
        node_id,
        capability,
        &req.task,
        req.did.as_deref(),
        req.cpu_milli,
        req.mem_mb,
    ).await;

    // Record accountability for spawning
    {
        use crate::identity::AgentIdentity;
        use crate::federation::ActionOutcome;
        let identity = AgentIdentity::from_did(
            req.did.as_deref().unwrap_or("did:autonomyx:computekube")
        );
        let outcome = match job.phase {
            crate::computekube::JobPhase::Failed => ActionOutcome::Failed,
            _ => ActionOutcome::Success,
        };
        state.federation.record(
            &identity,
            "computekube:spawn",
            &req.agent_id,
            None,
            outcome,
            json!({
                "job_id":   job.id,
                "job_name": job.job_name,
                "node_id":  node_id,
                "run_id":   run_id,
            }),
        );
    }

    // Emit fabric event — compute dispatched
    state.fabric.emit(crate::fabric::FabricEvent {
        id:         job.id.clone(),
        artifact:   req.agent_id.clone(),
        stage:      crate::lifecycle::Stage::Run,
        status:     crate::fabric::FabricStatus::Open,
        payload:    json!({
            "type":       "computekube.job_spawned",
            "job_id":     job.id,
            "job_name":   job.job_name,
            "node_id":    node_id,
            "capability": capability,
            "phase":      job.phase,
        }),
        emitted_at: chrono::Utc::now(),
    });

    Json(json!({
        "job":    job,
        "status": "spawned",
        "mode":   if state.computekube.is_available() {
            "kubernetes"
        } else {
            "in-process (set KUBERNETES_SERVICE_HOST to enable k8s)"
        },
    }))
}

async fn list_jobs(State(state): State<Arc<AppState>>) -> Json<Value> {
    let jobs = state.computekube.list_jobs();
    Json(json!({
        "jobs":  jobs,
        "count": jobs.len(),
    }))
}

async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.computekube.get_job(&id) {
        Some(job) => Json(json!({ "job": job })),
        None      => Json(json!({ "error": "job not found", "id": id })),
    }
}

async fn sync_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.computekube.sync_status(&id).await {
        Some(job) => Json(json!({ "job": job, "synced": true })),
        None      => Json(json!({ "error": "job not found", "id": id })),
    }
}

async fn job_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.computekube.fetch_logs(&id).await {
        Some(logs) => Json(json!({ "job_id": id, "logs": logs })),
        None       => Json(json!({ "error": "logs not available", "id": id })),
    }
}

async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.computekube.cancel(&id).await {
        Ok(())  => Json(json!({ "cancelled": true, "id": id })),
        Err(e)  => Json(json!({ "error": e })),
    }
}
