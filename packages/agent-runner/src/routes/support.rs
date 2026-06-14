// Autonomyx Support — foundational knowledge + enterprise assistance routes.
//
// Foundational support is built into every runtime, every tier.
// No ticket required. Always available. First line of triage.
//
// GET /support         — support tier, channels, SLA commitments
// GET /support/health  — platform health summary (all gates, fabric, federation)
// GET /support/triage  — first-responder view: dead-letters + gate failures
// GET /support/runbook — links to operational runbooks

use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

// ── /support — tier + channels + resources ────────────────────────────────────

async fn support_info() -> Json<Value> {
    Json(json!({
        "platform":  "Autonomyx",
        "version":   env!("CARGO_PKG_VERSION"),
        "site":      "https://openautonomyx.com",

        "foundational": {
            "docs":      "https://docs.openautonomyx.com",
            "spec":      "AGENT_SPEC.md",
            "aip":       "AIP.md",
            "community": "https://github.com/AGenNextHub/Agent-IDE/issues",
            "status":    "https://status.openautonomyx.com",
            "always_available": true
        },

        "tiers": {
            "community": {
                "channels":          ["docs", "community_forum", "github_issues"],
                "response_critical": "72h",
                "self_service":      true
            },
            "startup": {
                "channels":          ["docs", "async_ticket", "onboarding_session"],
                "response_critical": "4h",
                "assistance":        ["onboarding", "architecture_review"]
            },
            "growth": {
                "channels":          ["docs", "priority_queue", "slack_shared", "csm"],
                "response_critical": "1h",
                "assistance":        [
                    "architecture_review", "deployment_assist",
                    "training_workshops", "cost_optimisation"
                ]
            },
            "enterprise": {
                "channels":          ["docs", "priority_queue", "slack_private", "tam", "on_call_24x7"],
                "response_critical": "15min",
                "assistance":        [
                    "architecture_review", "deployment_assist", "integration_build",
                    "incident_response", "runbook_authoring",
                    "training_workshops", "certification",
                    "gpa_policy_design", "audit_review", "compliance_alignment",
                    "cost_optimisation", "bom_review", "cosign_policy"
                ],
                "dedicated_cluster": true,
                "custom_sla":        true
            },
            "partner": {
                "channels":          ["docs", "slack_private", "on_call_24x7", "co_build"],
                "response_critical": "15min",
                "assistance":        "all",
                "joint_roadmap":     true,
                "white_glove":       true
            }
        },

        "enterprise_assistance": {
            "implementation":  ["architecture_review", "deployment_assist", "integration_build", "migration_support"],
            "operations":      ["incident_response", "runbook_authoring", "chaos_testing", "capacity_planning"],
            "adoption":        ["training_workshops", "certification", "enablement_materials", "onboarding"],
            "governance":      ["gpa_policy_design", "audit_review", "compliance_alignment"],
            "finops":          ["cost_optimisation", "provider_negotiation", "budget_modelling"],
            "supply_chain":    ["bom_review", "cosign_policy", "sbom_compliance"]
        },

        "contact": "support@openautonomyx.com"
    }))
}

// ── /support/health — platform health (first triage signal) ──────────────────

async fn support_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let agent_count = state.agents.read().unwrap().len();
    let run_count   = state.runs.read().unwrap().len();
    let peer_count  = state.peers.read().unwrap().len();

    let gate_log     = state.lifecycle.full_log();
    let open_gates   = gate_log.iter().filter(|r| matches!(r.status, crate::lifecycle::GateStatus::Open)).count();
    let closed_gates = gate_log.iter().filter(|r| matches!(r.status, crate::lifecycle::GateStatus::Closed)).count();

    let fabric_dead  = state.fabric.dead_log().len();
    let fabric_total = state.fabric.full_log().len();

    let fed_dids = state.federation.list_dids().len();

    let overall = if closed_gates == 0 && fabric_dead == 0 { "healthy" }
                  else if fabric_dead > 5 || closed_gates > 10 { "degraded" }
                  else { "warning" };

    Json(json!({
        "status":   overall,
        "platform": "Autonomyx",
        "components": {
            "agents":    { "registered": agent_count },
            "runs":      { "total": run_count },
            "peers":     { "registered": peer_count },
            "lifecycle": { "gate_opens": open_gates, "gate_closes": closed_gates },
            "fabric":    { "total_events": fabric_total, "dead_letter": fabric_dead },
            "federation":{ "known_dids": fed_dids }
        },
        "triage": {
            "dead_letter_events": fabric_dead,
            "gate_failures":      closed_gates,
            "action":
                if fabric_dead > 0 || closed_gates > 0 {
                    "check GET /support/triage for details"
                } else {
                    "no action required"
                }
        }
    }))
}

// ── /support/triage — first-responder: dead-letters + gate failures ───────────

async fn support_triage(State(state): State<Arc<AppState>>) -> Json<Value> {
    let dead_letters = state.fabric.dead_log();
    let gate_failures: Vec<_> = state.lifecycle.full_log().iter()
        .filter(|r| matches!(r.status, crate::lifecycle::GateStatus::Closed))
        .cloned()
        .collect();

    Json(json!({
        "triage_summary": {
            "dead_letter_count": dead_letters.len(),
            "gate_failure_count": gate_failures.len(),
            "severity": if dead_letters.is_empty() && gate_failures.is_empty() {
                "none"
            } else if dead_letters.len() > 5 || gate_failures.len() > 10 {
                "critical"
            } else {
                "warning"
            }
        },
        "dead_letters":   dead_letters,
        "gate_failures":  gate_failures,
        "runbook":        "https://docs.openautonomyx.com/runbooks/gate-failure",
        "escalation":     "If severity=critical: contact support@openautonomyx.com"
    }))
}

// ── /support/runbook — links to operational runbooks ─────────────────────────

async fn support_runbook() -> Json<Value> {
    Json(json!({
        "runbooks": {
            "gate_closed":         "https://docs.openautonomyx.com/runbooks/gate-failure",
            "dead_letter":         "https://docs.openautonomyx.com/runbooks/dead-letter",
            "sla_breach":          "https://docs.openautonomyx.com/runbooks/sla-breach",
            "supply_chain":        "https://docs.openautonomyx.com/runbooks/supply-chain",
            "identity_failure":    "https://docs.openautonomyx.com/runbooks/identity",
            "budget_exceeded":     "https://docs.openautonomyx.com/runbooks/finops",
            "configdb_disconnect": "https://docs.openautonomyx.com/runbooks/configdb",
            "peer_unreachable":    "https://docs.openautonomyx.com/runbooks/federation"
        },
        "knowledge_base": "https://docs.openautonomyx.com",
        "aip_spec":       "https://openautonomyx.com/AIP.md",
        "agent_spec":     "https://openautonomyx.com/AGENT_SPEC.md"
    }))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/support",          get(support_info))
        .route("/support/runbook",  get(support_runbook))
        .route("/support/health",   get(support_health).with_state(state.clone()))
        .route("/support/triage",   get(support_triage).with_state(state))
}
