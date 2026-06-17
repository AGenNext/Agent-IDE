// Autonomyx Reconciler — auto rebase at every surface.
//
// Every surface in the platform can drift from its desired state:
//   - A plugin gets enabled but its govgraph nodes aren't wired yet
//   - A peer goes offline but the peer registry still shows it online
//   - A run hangs forever because a provider crashed mid-iteration
//   - An opt-in is approved but never reflected in the governance graph
//   - Credentials expire but aren't pruned from the auth-matic store
//   - A goal completes but the next one never activates
//
// The reconciler closes every one of these gaps on a continuous loop.
// It also reacts to fabric events for immediate rebase when drift is detected.
//
// Surfaces and their rebase cadence:
//   plugins   → 60 s   rebase govgraph nodes to match enabled plugins
//   providers → 120 s  degrade trust for providers that failed certification
//   runs      → 30 s   mark hung runs as failed after MAX_RUN_TTL_SECS
//   peers     → 90 s   mark peers offline if last_seen > PEER_OFFLINE_SECS
//   optin     → 60 s   activate approved opt-ins that aren't yet in govgraph
//   authmatic → 300 s  prune expired credentials
//   goals     → 60 s   detect completed goals and activate next in sequence
//
// All rebase actions emit a fabric event so the full system observes the correction.
//
// "The platform reconciles. It never drifts in silence." — openautonomyx.com

use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, interval};
use chrono::Utc;
use crate::store::AppState;
use crate::fabric::FabricEvent;
use crate::lifecycle::Stage;
use crate::govgraph::{GovernanceNode, NodeKind, NodePolicy};

const MAX_RUN_TTL_SECS:     i64 = 3600;   // 1 hour — hung run timeout
const PEER_OFFLINE_SECS:    i64 = 300;    // 5 min without a ping → offline
const PLUGIN_REBASE_SECS:   u64 = 60;
const PROVIDER_REBASE_SECS: u64 = 120;
const RUN_REBASE_SECS:      u64 = 30;
const PEER_REBASE_SECS:     u64 = 90;
const OPTIN_REBASE_SECS:    u64 = 60;
const AUTH_REBASE_SECS:     u64 = 300;
const GOAL_REBASE_SECS:     u64 = 60;

/// Start all reconciliation loops in background tasks.
pub fn start(state: Arc<AppState>) {
    tracing::info!("reconciler: auto rebase started — 7 surfaces");

    let s = state.clone(); tokio::spawn(async move { loop_plugins(s).await });
    let s = state.clone(); tokio::spawn(async move { loop_providers(s).await });
    let s = state.clone(); tokio::spawn(async move { loop_runs(s).await });
    let s = state.clone(); tokio::spawn(async move { loop_peers(s).await });
    let s = state.clone(); tokio::spawn(async move { loop_optin(s).await });
    let s = state.clone(); tokio::spawn(async move { loop_authmatic(s).await });
    let s = state.clone(); tokio::spawn(async move { loop_goals(s).await });
}

// ── Surface 1: Plugin ↔ Govgraph ─────────────────────────────────────────────
// Desired: every enabled plugin node is in the govgraph.
// Drift:   plugin enabled at runtime but govgraph hasn't been updated.
// Rebase:  add missing nodes, update trust on existing ones.

async fn loop_plugins(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(PLUGIN_REBASE_SECS));
    loop {
        tick.tick().await;
        rebase_plugins(&state);
    }
}

fn rebase_plugins(state: &Arc<AppState>) {
    let mut rebased = 0usize;
    for pn in state.plugins.all_nodes() {
        if state.govgraph.get_node(&pn.id).is_none() {
            let kind = match pn.kind.as_str() {
                "source" => NodeKind::Source,
                "sink"   => NodeKind::Sink,
                "api"    => NodeKind::Api,
                _        => NodeKind::Tool,
            };
            let node = GovernanceNode {
                id:           pn.id.clone(),
                kind,
                label:        pn.label.clone(),
                description:  format!("Plugin node: {}", pn.label),
                did:          None,
                capabilities: pn.capabilities.clone(),
                requires:     vec![pn.capability_required.clone()],
                trust_score:  0.8,
                policy:       NodePolicy::default(),
                metadata:     serde_json::json!({}),
                created_at:   Utc::now(),
                updated_at:   Utc::now(),
            };
            state.govgraph.add_node(node);
            rebased += 1;
        }
    }
    if rebased > 0 {
        tracing::info!(rebased, "reconciler: plugins → govgraph rebase");
        state.fabric.emit(FabricEvent::open(
            "reconciler:plugins",
            Stage::Observe,
            serde_json::json!({ "surface": "plugins", "rebased": rebased }),
        ));
    }
}

// ── Surface 2: Provider trust ─────────────────────────────────────────────────
// Desired: govgraph trust reflects real provider health.
// Drift:   provider fails but govgraph trust stays at 0.8.
// Rebase:  degrade trust for providers that fail certification checks.

async fn loop_providers(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(PROVIDER_REBASE_SECS));
    loop {
        tick.tick().await;
        rebase_providers(&state);
    }
}

fn rebase_providers(state: &Arc<AppState>) {
    let probe_models = [
        ("claude-opus-4-8", "compute:anthropic"),
        ("ollama:llama3",   "compute:ollama"),
        ("llama-rs:llama2", "compute:llamars"),
    ];
    for (model, node_id) in probe_models {
        let cert = crate::provider_cert::certify(model, state);
        if !cert.certified {
            // Degrade trust — provider failed cert
            state.govgraph.update_trust(node_id, false);
            tracing::warn!(
                model    = %model,
                node     = %node_id,
                "reconciler: provider cert failed → trust degraded"
            );
        } else {
            // Repair trust — provider is healthy
            state.govgraph.update_trust(node_id, true);
        }
    }
}

// ── Surface 3: Hung runs ──────────────────────────────────────────────────────
// Desired: all runs either complete or fail within MAX_RUN_TTL_SECS.
// Drift:   provider crashed mid-run; run stays in Running forever.
// Rebase:  mark any run running > TTL as Failed.

async fn loop_runs(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(RUN_REBASE_SECS));
    loop {
        tick.tick().await;
        rebase_runs(&state);
    }
}

fn rebase_runs(state: &Arc<AppState>) {
    use crate::store::RunStatus;
    let now = Utc::now();
    let hung: Vec<String> = state.runs.read().unwrap()
        .values()
        .filter(|r| r.status == RunStatus::Running
            && (now - r.started_at).num_seconds() > MAX_RUN_TTL_SECS)
        .map(|r| r.run_id.clone())
        .collect();

    for run_id in &hung {
        state.finish_run(run_id, RunStatus::Failed);
        state.add_run_step(run_id, "reconciler",
            &format!("auto-failed after {}s TTL — provider likely crashed", MAX_RUN_TTL_SECS));
        state.broadcast_to_run(run_id, &serde_json::json!({
            "type":    "reconciler",
            "content": "run timed out — auto-failed",
            "runId":   run_id,
        }).to_string());
        tracing::warn!(run_id = %run_id, "reconciler: hung run auto-failed");
    }

    if !hung.is_empty() {
        state.fabric.emit(FabricEvent::open(
            "reconciler:runs",
            Stage::Observe,
            serde_json::json!({ "surface": "runs", "hung_failed": hung.len() }),
        ));
    }
}

// ── Surface 4: Peer liveness ──────────────────────────────────────────────────
// Desired: peers are marked offline when they stop responding.
// Drift:   peer crashes; peer registry still shows status="online".
// Rebase:  mark peers offline if last_seen > PEER_OFFLINE_SECS.

async fn loop_peers(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(PEER_REBASE_SECS));
    loop {
        tick.tick().await;
        rebase_peers(&state);
    }
}

fn rebase_peers(state: &Arc<AppState>) {
    let now = Utc::now();
    let mut stale = vec![];
    {
        let peers = state.peers.read().unwrap();
        for p in peers.values() {
            if p.status == "online" || p.status == "connected" {
                if let Some(last) = p.last_seen {
                    if (now - last).num_seconds() > PEER_OFFLINE_SECS {
                        stale.push(p.id.clone());
                    }
                }
            }
        }
    }
    if !stale.is_empty() {
        let mut peers = state.peers.write().unwrap();
        for id in &stale {
            if let Some(p) = peers.get_mut(id) {
                p.status = "offline".into();
            }
        }
        drop(peers);
        tracing::warn!(count = stale.len(), "reconciler: peers marked offline (last_seen timeout)");
        state.fabric.emit(FabricEvent::open(
            "reconciler:peers",
            Stage::Observe,
            serde_json::json!({ "surface": "peers", "marked_offline": stale.len() }),
        ));
    }
}

// ── Surface 5: Opt-in → Govgraph ─────────────────────────────────────────────
// Desired: every approved opt-in extension is reflected in govgraph.
// Drift:   opt-in approved while govgraph was unavailable; node never added.
// Rebase:  ensure all active opt-in extensions have govgraph nodes.

async fn loop_optin(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(OPTIN_REBASE_SECS));
    loop {
        tick.tick().await;
        rebase_optin(&state);
    }
}

fn rebase_optin(state: &Arc<AppState>) {
    let extensions = state.optin.active_extensions();
    let mut rebased = 0usize;
    for ext in extensions {
        let node_id = format!("optin:{}", ext.id);
        if state.govgraph.get_node(&node_id).is_none() {
            let caps = ext.extension.as_ref()
                .map(|e| e.capabilities.clone())
                .unwrap_or_else(|| vec!["capability:custom".into()]);
            let cap0 = caps.first().cloned().unwrap_or_else(|| "capability:custom".into());
            let node_kind = ext.extension.as_ref()
                .map(|e| e.node_kind.as_str())
                .unwrap_or("api");
            let kind = match node_kind {
                "source" => NodeKind::Source,
                "sink"   => NodeKind::Sink,
                "tool"   => NodeKind::Tool,
                _        => NodeKind::Api,
            };
            let node = GovernanceNode {
                id:           node_id.clone(),
                kind,
                label:        ext.name.clone(),
                description:  format!("Opt-in extension: {}", ext.description),
                did:          Some(ext.actor_did.clone()),
                capabilities: caps,
                requires:     vec![cap0],
                trust_score:  0.75,
                policy:       NodePolicy::default(),
                metadata:     ext.metadata.clone(),
                created_at:   Utc::now(),
                updated_at:   Utc::now(),
            };
            state.govgraph.add_node(node);
            rebased += 1;
        }
    }
    if rebased > 0 {
        tracing::info!(rebased, "reconciler: optin → govgraph rebase");
    }
}

// ── Surface 6: Auth-matic credential expiry ───────────────────────────────────
// Desired: expired credentials are pruned from the auth-matic store.
// Drift:   credentials accumulate; memory grows; stale keys accepted briefly.
// Rebase:  call authmatic.prune() on cadence.

async fn loop_authmatic(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(AUTH_REBASE_SECS));
    loop {
        tick.tick().await;
        let pruned = state.authmatic.prune();
        if pruned > 0 {
            tracing::info!(pruned, "reconciler: authmatic credentials pruned");
        }
    }
}

// ── Surface 7: Goal progression ──────────────────────────────────────────────
// Desired: completed goals are retired and the next goal in sequence activates.
// Drift:   goal marked achieved but next goal stays pending forever.
// Rebase:  scan for achieved goals with pending successors and activate them.

async fn loop_goals(state: Arc<AppState>) {
    let mut tick = interval(Duration::from_secs(GOAL_REBASE_SECS));
    loop {
        tick.tick().await;
        rebase_goals(&state);
    }
}

fn rebase_goals(state: &Arc<AppState>) {
    use crate::goals::GoalStatus;
    let goals = state.goals.list();

    // Find agents whose most recently achieved goal has a next pending goal
    let mut activated = 0usize;
    for goal in &goals {
        if goal.status != GoalStatus::Achieved { continue; }
        // Look for an aligned (ready) goal on the same agent to activate next
        let next = goals.iter().find(|g| {
            g.agent_id == goal.agent_id
            && g.status == GoalStatus::Aligned
        });
        if let Some(next_goal) = next {
            if let Ok(_) = state.goals.activate(&next_goal.id) {
                activated += 1;
                tracing::info!(
                    agent_id  = %next_goal.agent_id,
                    goal_id   = %next_goal.id,
                    prev_goal = %goal.id,
                    "reconciler: next goal activated"
                );
                state.fabric.emit(FabricEvent::open(
                    &next_goal.agent_id,
                    Stage::Run,
                    serde_json::json!({
                        "surface":      "goals",
                        "activated":    next_goal.id,
                        "completed":    goal.id,
                    }),
                ));
            }
        }
    }

    if activated > 0 {
        tracing::info!(activated, "reconciler: goals rebased");
    }
}

// ── Rebase summary ────────────────────────────────────────────────────────────

/// Trigger all rebase surfaces immediately (e.g., called on startup or demand).
pub fn rebase_all(state: &Arc<AppState>) {
    rebase_plugins(state);
    rebase_providers(state);
    rebase_runs(state);
    rebase_peers(state);
    rebase_optin(state);
    rebase_goals(state);
    let pruned = state.authmatic.prune();
    tracing::info!(pruned, "reconciler: full rebase complete (all 7 surfaces)");
    state.fabric.emit(FabricEvent::open(
        "reconciler",
        Stage::Observe,
        serde_json::json!({ "action": "rebase_all", "surfaces": 7 }),
    ));
}
