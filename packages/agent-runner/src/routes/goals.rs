// Goal routes — purpose-driven agents that make the world better.
//
// POST /api/goals/missions          — declare an agent's mission
// GET  /api/goals/missions/:agent   — get an agent's missions
// POST /api/goals                   — create a goal
// POST /api/goals/:id/align         — run alignment check (7 values)
// POST /api/goals/:id/activate      — activate an aligned goal
// POST /api/goals/:id/objectives    — add an objective
// POST /api/goals/:id/impact        — record real-world impact measurement
// GET  /api/goals/:id               — get goal detail + objectives + impact
// GET  /api/goals                   — list all goals
// GET  /api/goals/summary           — platform-wide goal health + impact progress
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;
use crate::goals::{ImpactDomain, ImpactMetric, WorldModelDelta};

#[derive(Deserialize)]
struct MissionReq {
    agent_id:      String,
    statement:     String,
    values:        Option<Vec<String>>,
    beneficiaries: Option<Vec<String>>,
    domain:        Option<String>,
}

#[derive(Deserialize)]
struct CreateGoalReq {
    agent_id:        String,
    title:           String,
    description:     String,
    intended_impact: String,
    mission_id:      Option<String>,
    world_before:    Option<String>,
    world_after:     Option<String>,
    world_scope:     Option<String>,
    timeline_days:   Option<u32>,
    impact_metrics:  Option<Vec<MetricReq>>,
}

#[derive(Deserialize)]
struct MetricReq {
    name:        String,
    description: Option<String>,
    unit:        String,
    baseline:    f64,
    target:      f64,
    domain:      Option<String>,
}

#[derive(Deserialize)]
struct AddObjectiveReq {
    title:       String,
    description: Option<String>,
    assigned_to: Option<String>,
    from_node:   Option<String>,
    to_node:     Option<String>,
}

#[derive(Deserialize)]
struct RecordImpactReq {
    metric_name: String,
    actual:      f64,
    notes:       Option<String>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/goals/summary",              get(goals_summary))
        .route("/goals/missions",             post(declare_mission))
        .route("/goals/missions/:agent_id",   get(agent_missions))
        .route("/goals",                      get(list_goals).post(create_goal))
        .route("/goals/:id",                  get(get_goal))
        .route("/goals/:id/align",            post(align_goal))
        .route("/goals/:id/activate",         post(activate_goal))
        .route("/goals/:id/objectives",       post(add_objective))
        .route("/goals/:id/objectives/:oid/complete", post(complete_objective))
        .route("/goals/:id/impact",           post(record_impact))
        .with_state(state)
}

async fn goals_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(state.goals.summary())
}

async fn declare_mission(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MissionReq>,
) -> Json<Value> {
    let domain = parse_domain(req.domain.as_deref());
    let values = req.values.unwrap_or_else(|| vec![
        "transparency".into(), "equity".into(),
        "sustainability".into(), "mutual_respect".into(),
    ]);
    let beneficiaries = req.beneficiaries.unwrap_or_else(|| vec!["humanity".into()]);

    let mission = state.goals.declare_mission(
        &req.agent_id, &req.statement, values, beneficiaries, domain,
    );

    // Record in federation — mission is an accountable commitment
    let identity = crate::identity::AgentIdentity::from_did(
        &format!("did:autonomyx:{}", req.agent_id)
    );
    state.federation.record(
        &identity, "goal:mission_declared", &req.agent_id, None,
        crate::federation::ActionOutcome::Success,
        json!({ "mission_id": mission.id, "statement": mission.statement }),
    );

    Json(json!({ "mission": mission, "status": "declared" }))
}

async fn agent_missions(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<String>,
) -> Json<Value> {
    let missions = state.goals.missions_for(&agent_id);
    Json(json!({ "agent_id": agent_id, "missions": missions }))
}

async fn create_goal(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateGoalReq>,
) -> Json<Value> {
    let world_model = WorldModelDelta {
        before:        req.world_before.unwrap_or_else(|| "Current state unspecified".into()),
        after:         req.world_after.unwrap_or_else(|| req.intended_impact.clone()),
        scope:         req.world_scope.unwrap_or_else(|| "local".into()),
        timeline_days: req.timeline_days,
    };

    let metrics: Vec<ImpactMetric> = req.impact_metrics.unwrap_or_default()
        .into_iter().map(|m| ImpactMetric {
            id:          uuid::Uuid::new_v4().to_string(),
            name:        m.name,
            description: m.description.unwrap_or_default(),
            unit:        m.unit,
            baseline:    m.baseline,
            target:      m.target,
            actual:      None,
            measured_at: None,
            domain:      parse_domain(m.domain.as_deref()),
        }).collect();

    let goal = state.goals.create_goal(
        &req.agent_id, &req.title, &req.description,
        &req.intended_impact, req.mission_id, world_model, metrics,
    );

    Json(json!({
        "goal":   goal,
        "status": "draft",
        "next":   "POST /api/goals/{id}/align to run the 7-value alignment check",
    }))
}

async fn align_goal(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.goals.align(&id) {
        Ok(check) => {
            // Emit fabric event — aligned goal ready to pursue
            state.fabric.emit(crate::fabric::FabricEvent {
                id:         uuid::Uuid::new_v4().to_string(),
                artifact:   id.clone(),
                stage:      crate::lifecycle::Stage::Build,
                status:     crate::fabric::FabricStatus::Open,
                payload:    json!({
                    "type":  "goal.aligned",
                    "goal":  id,
                    "score": check.score(),
                }),
                emitted_at: chrono::Utc::now(),
            });
            Json(json!({
                "goal_id":   id,
                "aligned":   true,
                "score":     check.score(),
                "check":     check,
                "next":      "POST /api/goals/{id}/activate to begin pursuit",
                "message":   "Goal aligned. The path to making the world better is open.",
            }))
        }
        Err(e) => Json(json!({
            "goal_id":  id,
            "aligned":  false,
            "rejected": true,
            "reason":   e,
            "message":  "Goal rejected — alignment is the filter between ambition and action.",
        })),
    }
}

async fn activate_goal(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.goals.activate(&id) {
        Ok(goal) => {
            // Register in governance graph as a goal node
            let node = crate::govgraph::GovernanceNode {
                id:           format!("goal:{}", id),
                kind:         crate::govgraph::NodeKind::Agent,
                label:        goal.title.clone(),
                description:  goal.intended_impact.clone(),
                did:          Some(format!("did:autonomyx:goal:{}", id)),
                capabilities: vec!["goal:pursue".into(), "impact:record".into()],
                requires:     vec!["goal:aligned".into()],
                trust_score:  goal.alignment.as_ref().map(|a| a.score()).unwrap_or(0.5),
                policy:       crate::govgraph::NodePolicy::default(),
                metadata:     json!({ "goal_id": id }),
                created_at:   chrono::Utc::now(),
                updated_at:   chrono::Utc::now(),
            };
            state.govgraph.add_node(node);

            Json(json!({
                "goal":    goal,
                "status":  "active",
                "message": "Goal activated. Agents are now pursuing it. One step at a time, taking everyone together.",
            }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn add_objective(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<AddObjectiveReq>,
) -> Json<Value> {
    let node_path = req.from_node.zip(req.to_node);
    match state.goals.add_objective(
        &id, &req.title,
        req.description.as_deref().unwrap_or(""),
        req.assigned_to,
        node_path,
    ) {
        Ok(obj) => Json(json!({ "objective": obj })),
        Err(e)  => Json(json!({ "error": e })),
    }
}

async fn complete_objective(
    State(state): State<Arc<AppState>>,
    Path((_, oid)): Path<(String, String)>,
) -> Json<Value> {
    match state.goals.complete_objective(&oid) {
        Ok(obj) => Json(json!({ "objective": obj, "status": "completed" })),
        Err(e)  => Json(json!({ "error": e })),
    }
}

async fn record_impact(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<RecordImpactReq>,
) -> Json<Value> {
    match state.goals.record_impact(&id, &req.metric_name, req.actual) {
        Ok(goal) => {
            let achieved = goal.status == crate::goals::GoalStatus::Achieved;

            // If goal achieved — close the loop via feedback gate
            if achieved {
                state.fabric.emit(crate::fabric::FabricEvent {
                    id:         uuid::Uuid::new_v4().to_string(),
                    artifact:   id.clone(),
                    stage:      crate::lifecycle::Stage::Feedback,
                    status:     crate::fabric::FabricStatus::Open,
                    payload:    json!({
                        "type":    "goal.achieved",
                        "goal_id": id,
                        "title":   goal.title,
                        "impact":  req.metric_name,
                        "actual":  req.actual,
                        "message": "Goal achieved. The world is better. The loop closes and begins again.",
                    }),
                    emitted_at: chrono::Utc::now(),
                });
            }

            Json(json!({
                "goal_id":  id,
                "metric":   req.metric_name,
                "actual":   req.actual,
                "notes":    req.notes,
                "achieved": achieved,
                "status":   goal.status,
                "metrics":  goal.impact_metrics.iter().map(|m| json!({
                    "name":     m.name,
                    "progress": m.progress(),
                    "achieved": m.achieved(),
                    "actual":   m.actual,
                    "target":   m.target,
                    "unit":     m.unit,
                })).collect::<Vec<_>>(),
                "message": if achieved {
                    "Goal achieved. The world is better. The loop closes and begins again."
                } else {
                    "Impact recorded. One step at a time, taking everyone together."
                },
            }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn get_goal(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.goals.get_goal(&id) {
        Some(goal) => {
            let objectives = state.goals.objectives_for(&id);
            let mission = goal.mission_id.as_ref()
                .and_then(|mid| state.goals.get_mission(mid));
            Json(json!({
                "goal":       goal,
                "mission":    mission,
                "objectives": objectives,
            }))
        }
        None => Json(json!({ "error": "goal not found", "id": id })),
    }
}

async fn list_goals(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let agent_id = q.get("agent_id").map(|s| s.as_str());
    let goals = state.goals.list_goals(agent_id);
    Json(json!({ "goals": goals, "count": goals.len() }))
}

fn parse_domain(s: Option<&str>) -> ImpactDomain {
    match s {
        Some("social")        => ImpactDomain::Social,
        Some("economic")      => ImpactDomain::Economic,
        Some("environmental") => ImpactDomain::Environmental,
        Some("educational")   => ImpactDomain::Educational,
        Some("health")        => ImpactDomain::Health,
        Some("civic")         => ImpactDomain::Civic,
        Some("cultural")      => ImpactDomain::Cultural,
        _                     => ImpactDomain::Technological,
    }
}
