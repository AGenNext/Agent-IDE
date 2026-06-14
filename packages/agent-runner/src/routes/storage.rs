// Storage routes — distributed, policy-driven, milestone-bound.
//
// POST /api/storage/artifacts         — write an artifact
// GET  /api/storage/artifacts         — list artifacts (policy-filtered)
// GET  /api/storage/artifacts/:key    — read an artifact
// DELETE /api/storage/artifacts/:key  — delete (if policy allows)
// POST /api/storage/projects          — create a project
// GET  /api/storage/projects          — list projects
// GET  /api/storage/projects/:id      — get project + milestones
// POST /api/storage/projects/:id/milestones      — create milestone
// POST /api/storage/projects/:id/milestones/:mid/complete — complete milestone
// GET  /api/storage/summary           — storage state summary
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use crate::store::AppState;
use crate::storage::StoragePolicy;

#[derive(Deserialize)]
struct PutReq {
    key:          String,
    content:      String,          // base64 or plain text
    content_type: Option<String>,
    actor_did:    Option<String>,
    public:       Option<bool>,
    immutable:    Option<bool>,
    milestone:    Option<String>,
    tags:         Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct GetReq {
    actor_did: Option<String>,
}

#[derive(Deserialize)]
struct CreateProjectReq {
    name:        String,
    description: Option<String>,
    owner_did:   Option<String>,
    agents:      Option<Vec<String>>,
}

#[derive(Deserialize)]
struct CreateMilestoneReq {
    name:        String,
    description: Option<String>,
    stage:       Option<String>,
    assigned_to: Option<String>,
}

#[derive(Deserialize)]
struct CompleteMilestoneReq {
    artifact_keys: Option<Vec<String>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/storage/summary",                                    get(storage_summary))
        .route("/storage/artifacts",                                  get(list_artifacts).post(put_artifact))
        .route("/storage/artifacts/*key",                             get(get_artifact).delete(delete_artifact))
        .route("/storage/projects",                                   get(list_projects).post(create_project))
        .route("/storage/projects/:id",                               get(get_project))
        .route("/storage/projects/:id/milestones",                    post(create_milestone))
        .route("/storage/projects/:id/milestones/:mid/complete",      post(complete_milestone))
        .with_state(state)
}

async fn storage_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(state.storage.summary())
}

async fn put_artifact(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PutReq>,
) -> Json<Value> {
    let actor  = req.actor_did.as_deref().unwrap_or("did:autonomyx:platform");
    let public = req.public.unwrap_or(false);
    let mut policy = if public {
        StoragePolicy::public_read(actor)
    } else {
        StoragePolicy::owner_only(actor)
    };
    if let Some(imm) = req.immutable { policy.immutable = imm; }

    let content = req.content.into_bytes();
    let content_type = req.content_type.as_deref().unwrap_or("text/plain");
    let tags = req.tags.unwrap_or_default();

    match state.storage.put(
        &req.key, content, content_type, actor, policy, tags, req.milestone,
    ) {
        Ok(artifact) => Json(json!({
            "status":       "stored",
            "id":           artifact.id,
            "key":          artifact.key,
            "version":      artifact.version,
            "content_hash": artifact.content_hash,
            "size_bytes":   artifact.size_bytes,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn list_artifacts(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let actor  = q.get("actor_did").map(|s| s.as_str()).unwrap_or("*");
    let prefix = q.get("prefix").map(|s| s.as_str());
    let artifacts = state.storage.list(actor, prefix);
    Json(json!({ "artifacts": artifacts, "count": artifacts.len() }))
}

async fn get_artifact(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let actor = q.get("actor_did").map(|s| s.as_str()).unwrap_or("*");
    let stage = q.get("stage").map(|s| s.as_str());
    match state.storage.get(&key, actor, stage) {
        Ok(artifact) => Json(json!({
            "id":           artifact.id,
            "key":          artifact.key,
            "owner_did":    artifact.owner_did,
            "content_type": artifact.content_type,
            "content":      String::from_utf8_lossy(&artifact.content),
            "content_hash": artifact.content_hash,
            "size_bytes":   artifact.size_bytes,
            "version":      artifact.version,
            "milestone":    artifact.milestone,
            "tags":         artifact.tags,
            "created_at":   artifact.created_at,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn delete_artifact(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let actor = q.get("actor_did").map(|s| s.as_str()).unwrap_or("unknown");
    match state.storage.delete(&key, actor) {
        Ok(()) => Json(json!({ "deleted": true, "key": key })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn create_project(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateProjectReq>,
) -> Json<Value> {
    let owner = req.owner_did.as_deref().unwrap_or("did:autonomyx:platform");
    let agents = req.agents.unwrap_or_default();
    let p = state.storage.create_project(
        &req.name,
        req.description.as_deref().unwrap_or(""),
        owner,
        agents,
    );
    Json(json!(p))
}

async fn list_projects(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let owner = q.get("owner_did").map(|s| s.as_str());
    let projects = state.storage.list_projects(owner);
    Json(json!({ "projects": projects, "count": projects.len() }))
}

async fn get_project(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.storage.get_project(&id) {
        Some(p) => {
            let milestones = state.storage.list_milestones(&id);
            Json(json!({ "project": p, "milestones": milestones }))
        }
        None => Json(json!({ "error": "project not found", "id": id })),
    }
}

async fn create_milestone(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<CreateMilestoneReq>,
) -> Json<Value> {
    let m = state.storage.create_milestone(
        &id,
        &req.name,
        req.description.as_deref().unwrap_or(""),
        req.stage.as_deref().unwrap_or("build"),
        req.assigned_to,
    );
    Json(json!(m))
}

async fn complete_milestone(
    State(state): State<Arc<AppState>>,
    Path((_, mid)): Path<(String, String)>,
    Json(req): Json<CompleteMilestoneReq>,
) -> Json<Value> {
    let keys = req.artifact_keys.unwrap_or_default();
    match state.storage.complete_milestone(&mid, keys) {
        Ok(m)  => Json(json!({ "milestone": m, "status": "completed" })),
        Err(e) => Json(json!({ "error": e })),
    }
}
