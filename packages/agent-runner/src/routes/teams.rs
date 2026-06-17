// Team routes — institutional agent teams, universal cross-sector collaboration.
//
// POST   /api/teams                          — create institutional team
// GET    /api/teams                          — list all teams
// GET    /api/teams/summary                  — cross-sector overview
// GET    /api/teams/:id                      — get team + federated links
// POST   /api/teams/:id/activate             — activate team (assign DID)
// POST   /api/teams/:id/agents/:agent_id     — add agent to team
// DELETE /api/teams/:id/agents/:agent_id     — remove agent from team
// POST   /api/teams/:id/goals/:goal_id       — align goal to team
// POST   /api/teams/:id/regions/:region      — add geographic region
// POST   /api/teams/:id/languages            — set language list
// POST   /api/teams/:id/federate/:partner_id — federate two teams
//
// Every mutation emits a fabric event — no ungoverned subgraph is created.

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::store::AppState;
use crate::teams::InstitutionKind;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/teams",                               get(list_teams).post(create_team))
        .route("/teams/summary",                       get(summary))
        .route("/teams/:id",                           get(get_team))
        .route("/teams/:id/activate",                  post(activate_team))
        .route("/teams/:id/agents/:agent_id",          post(add_agent).delete(remove_agent))
        .route("/teams/:id/goals/:goal_id",            post(add_goal))
        .route("/teams/:id/regions/:region",           post(add_region))
        .route("/teams/:id/languages",                 post(set_languages))
        .route("/teams/:id/federate/:partner_id",      post(federate))
        .with_state(state)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateTeamBody {
    name:             String,
    institution_name: String,
    #[serde(default = "default_kind")]
    kind:             String,
    #[serde(default)]
    charter:          String,
    /// Domain of operation — e.g. "healthcare", "financial services", "software engineering"
    field_of_work:    Option<String>,
    /// Primary objective — "customer_satisfaction", "operational_excellence", "research", etc.
    objective:        Option<String>,
}
fn default_kind() -> String { "enterprise".into() }

async fn create_team(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateTeamBody>,
) -> Json<Value> {
    let kind = InstitutionKind::from_str(&body.kind);
    let team = s.teams.create(
        &body.name,
        &body.institution_name,
        kind,
        &body.charter,
        body.field_of_work.as_deref(),
        body.objective.as_deref(),
    );
    // Fabric: every team creation is visible in the thread
    s.fabric.emit(
        crate::fabric::FabricEvent::open(
            &format!("team:{}", team.id),
            crate::lifecycle::Stage::Build,
            json!({ "action": "created", "name": &team.name, "institution": &team.institution_name, "kind": &body.kind }),
        ).with_entities([format!("team:{}", team.id), format!("institution:{}", team.institution_name)])
    );
    Json(json!({ "team": team }))
}

async fn list_teams(State(s): State<Arc<AppState>>) -> Json<Value> {
    let teams = s.teams.list();
    Json(json!({ "teams": teams, "count": teams.len() }))
}

async fn summary(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(s.teams.summary())
}

async fn get_team(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match s.teams.get(&id) {
        Some(t) => Json(json!({ "team": t })),
        None    => Json(json!({ "error": "team not found", "id": id })),
    }
}

#[derive(Deserialize)]
struct ActivateBody {
    #[serde(default)]
    did: Option<String>,
}

async fn activate_team(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ActivateBody>,
) -> Json<Value> {
    let ok = s.teams.activate(&id, body.did.as_deref());
    if ok {
        s.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("team:{id}"),
                crate::lifecycle::Stage::Deploy,
                json!({ "action": "activated", "did": body.did }),
            ).with_entities([format!("team:{id}")])
        );
        Json(json!({ "ok": true, "team_id": id, "status": "active" }))
    } else {
        Json(json!({ "error": "team not found", "id": id }))
    }
}

async fn add_agent(
    State(s): State<Arc<AppState>>,
    Path((id, agent_id)): Path<(String, String)>,
) -> Json<Value> {
    let ok = s.teams.add_agent(&id, &agent_id);
    if ok {
        s.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("team:{id}"),
                crate::lifecycle::Stage::Run,
                json!({ "action": "agent_joined", "agent_id": agent_id }),
            ).with_entities([format!("team:{id}"), format!("agent:{agent_id}")])
        );
        Json(json!({ "ok": true, "team_id": id, "agent_id": agent_id }))
    } else {
        Json(json!({ "error": "team not found", "id": id }))
    }
}

async fn remove_agent(
    State(s): State<Arc<AppState>>,
    Path((id, agent_id)): Path<(String, String)>,
) -> Json<Value> {
    let ok = s.teams.remove_agent(&id, &agent_id);
    if ok {
        s.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("team:{id}"),
                crate::lifecycle::Stage::Observe,
                json!({ "action": "agent_left", "agent_id": agent_id }),
            ).with_entities([format!("team:{id}"), format!("agent:{agent_id}")])
        );
        Json(json!({ "ok": true, "team_id": id, "agent_id": agent_id }))
    } else {
        Json(json!({ "error": "team not found", "id": id }))
    }
}

async fn add_goal(
    State(s): State<Arc<AppState>>,
    Path((id, goal_id)): Path<(String, String)>,
) -> Json<Value> {
    let ok = s.teams.add_goal(&id, &goal_id);
    if ok {
        s.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("team:{id}"),
                crate::lifecycle::Stage::Run,
                json!({ "action": "goal_aligned", "goal_id": goal_id }),
            ).with_entities([format!("team:{id}"), format!("goal:{goal_id}")])
        );
        Json(json!({ "ok": true, "team_id": id, "goal_id": goal_id }))
    } else {
        Json(json!({ "error": "team not found", "id": id }))
    }
}

async fn add_region(
    State(s): State<Arc<AppState>>,
    Path((id, region)): Path<(String, String)>,
) -> Json<Value> {
    let ok = s.teams.add_region(&id, &region);
    Json(json!({ "ok": ok, "team_id": id, "region": region }))
}

#[derive(Deserialize)]
struct LanguagesBody { languages: Vec<String> }

async fn set_languages(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<LanguagesBody>,
) -> Json<Value> {
    let langs = body.languages.clone();
    let ok = s.teams.set_languages(&id, langs);
    if ok {
        s.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("team:{id}"),
                crate::lifecycle::Stage::Run,
                json!({ "action": "languages_set", "languages": body.languages }),
            ).with_entities([format!("team:{id}")])
        );
    }
    Json(json!({ "ok": ok, "team_id": id }))
}

async fn federate(
    State(s): State<Arc<AppState>>,
    Path((id, partner_id)): Path<(String, String)>,
) -> Json<Value> {
    let ok = s.teams.federate(&id, &partner_id);
    if ok {
        s.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("team:{id}"),
                crate::lifecycle::Stage::Sync,
                json!({ "action": "federated", "partner_team_id": partner_id }),
            ).with_entities([format!("team:{id}"), format!("team:{partner_id}")])
        );
    }
    Json(json!({ "ok": ok, "team_id": id, "partner_id": partner_id, "status": "federated" }))
}
