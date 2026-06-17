// Loop Coordinator — closes the loop at every gate.
//
// "One step at a time. Taking everyone together. Doing it in a loop."
// "Loop closes at every gate."
//
// The coordinator is a background task that subscribes to the fabric event bus.
// Events arrive as JSON strings (broadcast::Receiver<String>); we deserialize
// each one and react:
//   - Feedback gate open  → advance objectives for related goals
//   - Observe/Deploy/Build gate open → update governance graph trust
//   - Any gate closed (failure) → penalise trust on the responsible node
//   - Run gate open → record compute impact for active goals
//   - Goal fully achieved → activate next aligned goal for same agent
//
// This is NOT a polling loop. It is event-driven. No sleep, no polling, no waste.
// openautonomyx.com

use std::sync::Arc;
use crate::AppState;
use crate::fabric::{FabricEvent, FabricStatus};
use crate::lifecycle::Stage;
use crate::goals::{GoalStatus, ImpactDomain};

pub fn start(state: Arc<AppState>) {
    tokio::spawn(loop_task(state));
}

async fn loop_task(state: Arc<AppState>) {
    let mut rx = state.fabric.subscribe();
    tracing::info!("loop_coordinator: started — listening on fabric event bus");

    loop {
        let raw = match rx.recv().await {
            Ok(s)  => s,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "loop_coordinator: lagged — skipped fabric events");
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                tracing::warn!("loop_coordinator: fabric channel closed — stopping");
                break;
            }
        };

        let event: FabricEvent = match serde_json::from_str(&raw) {
            Ok(e)  => e,
            Err(e) => {
                tracing::warn!(error = %e, "loop_coordinator: failed to parse fabric event");
                continue;
            }
        };

        tracing::debug!(
            event_id = %event.id,
            artifact = %event.artifact,
            stage    = ?event.stage,
            status   = ?event.status,
            "loop_coordinator: event received"
        );

        // ── Gate: Feedback ────────────────────────────────────────────────────
        if matches!(event.stage, Stage::Feedback) && matches!(event.status, FabricStatus::Open) {
            if event.payload.get("action").and_then(|v| v.as_str()) == Some("fsm_state_output") {
                trigger_self_improvement(&state, &event.payload);
            } else {
                advance_goals_for(&state, &event.artifact).await;
            }
        }

        // ── Gate: Observe ─────────────────────────────────────────────────────
        if matches!(event.stage, Stage::Observe) && matches!(event.status, FabricStatus::Open) {
            state.govgraph.update_trust("agent:observe", true);
        }

        // ── Gate: Deploy ──────────────────────────────────────────────────────
        if matches!(event.stage, Stage::Deploy) && matches!(event.status, FabricStatus::Open) {
            state.govgraph.update_trust("agent:deploy", true);
        }

        // ── Gate: Build ───────────────────────────────────────────────────────
        if matches!(event.stage, Stage::Build) && matches!(event.status, FabricStatus::Open) {
            state.govgraph.update_trust("agent:build", true);
        }

        // ── Gate: any Closed (failure) → penalise responsible node ──────────
        if matches!(event.status, FabricStatus::Closed) {
            if let Some(node_id) = stage_to_node(&event.stage) {
                state.govgraph.update_trust(node_id, false);
                tracing::debug!(node = %node_id, "loop_coordinator: trust penalised");
            }
        }

        // ── Gate: Run complete → record compute impact ────────────────────────
        if matches!(event.stage, Stage::Run) && matches!(event.status, FabricStatus::Open) {
            record_compute_impact(&state, &event.artifact).await;
        }
    }

    tracing::warn!("loop_coordinator: event loop exited");
}

async fn advance_goals_for(state: &AppState, artifact: &str) {
    for goal in state.goals.list() {
        if goal.status != GoalStatus::Active { continue; }

        // Match goals related to this artifact or its agent
        let relevant = artifact.contains(&goal.agent_id)
            || goal.id.contains(artifact)
            || goal.intended_impact.to_lowercase().contains("feedback");

        if !relevant { continue; }

        // Advance the first pending objective
        let objs = state.goals.objectives_for(&goal.id);
        let pending = objs.iter().find(|o| o.status == crate::goals::ObjectiveStatus::Pending);
        if let Some(obj) = pending {
            let _ = state.goals.advance_objective(&goal.id, &obj.id);
            tracing::info!(
                goal_id  = %goal.id,
                obj_id   = %obj.id,
                artifact = %artifact,
                "loop_coordinator: objective advanced via feedback gate"
            );
        }

        // Check if all objectives completed — if so, complete the goal
        let all_done = state.goals.objectives_for(&goal.id).iter()
            .all(|o| o.status == crate::goals::ObjectiveStatus::Completed
                  || o.status == crate::goals::ObjectiveStatus::Skipped);
        if all_done && !state.goals.objectives_for(&goal.id).is_empty() {
            let _ = state.goals.complete(&goal.id);
            tracing::info!(goal_id = %goal.id, "loop_coordinator: goal completed — looping to next");
            activate_next_goal(state, &goal.agent_id).await;
        }
    }
}

async fn activate_next_goal(state: &AppState, agent_id: &str) {
    let candidate = state.goals.list().into_iter()
        .find(|g| g.agent_id == agent_id && g.status == GoalStatus::Aligned);

    if let Some(next) = candidate {
        match state.goals.activate(&next.id) {
            Ok(_) => {
                tracing::info!(goal_id = %next.id, agent_id = %agent_id,
                               "loop_coordinator: next goal activated");
                state.fabric.emit(crate::fabric::FabricEvent::open(
                    &next.id,
                    Stage::Run,
                    serde_json::json!({
                        "event":  "goal_loop_turn",
                        "agent":  agent_id,
                        "goal":   next.title,
                    }),
                ));
            }
            Err(e) => tracing::warn!(goal_id = %next.id, error = %e,
                                     "loop_coordinator: could not activate next goal"),
        }
    }
}

async fn record_compute_impact(state: &AppState, artifact: &str) {
    for goal in state.goals.list() {
        if goal.status != GoalStatus::Active { continue; }
        if !artifact.contains(&goal.agent_id) && !goal.id.contains(artifact) { continue; }
        let _ = state.goals.record_impact_by_domain(
            &goal.id,
            ImpactDomain::Technological,
            1,
            &format!("compute gate completed for {}", artifact),
        );
    }
}

fn trigger_self_improvement(state: &Arc<AppState>, payload: &serde_json::Value) {
    let agent_id      = payload["agent_id"].as_str().unwrap_or("").to_string();
    let system_prompt = payload["system_prompt"].as_str().unwrap_or("").to_string();
    let instruction   = payload["instruction"].as_str().unwrap_or("").to_string();
    let output        = payload["output"].as_str().unwrap_or("").to_string();
    let succeeded     = payload["succeeded"].as_bool().unwrap_or(false);
    let condition     = payload["condition"].as_str().unwrap_or("").to_string();

    if agent_id.is_empty() { return; }

    tracing::debug!(agent_id = %agent_id, "loop_coordinator: self-improvement task spawned");
    let state2 = state.clone();
    tokio::spawn(async move {
        crate::mega_agent::improve_prompt(
            &state2,
            &agent_id,
            &system_prompt,
            &instruction,
            &output,
            succeeded,
            &condition,
        ).await;
    });
}

fn stage_to_node(stage: &Stage) -> Option<&'static str> {
    match stage {
        Stage::Build    => Some("agent:build"),
        Stage::Deploy   => Some("agent:deploy"),
        Stage::Observe  => Some("agent:observe"),
        Stage::Feedback => Some("agent:observe"),
        Stage::Run      => Some("agent:observe"),
        _               => None,
    }
}
