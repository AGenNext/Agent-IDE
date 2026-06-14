// Agent routes — every agent is a tool, every tool ships with its instruction manual.
//
// "Agent is a tool" — a governed, accountable, versioned tool that can be invoked,
// inspected, trusted, and held accountable for its outcomes.
//
// "Agent must come with its instruction manual" — every agent exposes a manifest:
// what it does, what it can and cannot do, what it costs, what governance applies,
// and what facts it can never change (past is immutable, accountability is permanent).
//
// "Agent can be anything as long as it does not change the facts" —
// agents may adapt, learn, evolve — but they cannot alter the accountability log,
// cannot rewrite lifecycle records, cannot undo what has been recorded.
// The past is fact. The real is fact. The surface is fact.
//
// GET  /api/agents              — list all agents
// POST /api/agents              — register a new agent
// GET  /api/agents/:id          — get agent
// GET  /api/agents/:id/manifest — agent instruction manual (capabilities, constraints, cost)
// GET  /api/agents/:id/tools    — tools this agent exposes (MCP tool format)
// DELETE /api/agents/:id        — remove agent
//
// openautonomyx.com

use axum::{extract::{Path, State}, routing::{delete, get, post}, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use chrono::Utc;
use crate::store::AppState;

#[derive(Deserialize)]
struct AgentBody {
    name:         String,
    description:  Option<String>,
    model:        Option<String>,
    capabilities: Option<Vec<String>>,
    tools:        Option<Vec<String>>,
    budget_usd:   Option<f64>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/agents",              get(list).post(create))
        .route("/agents/:id",          get(get_one).delete(remove))
        .route("/agents/:id/manifest", get(get_manifest))
        .route("/agents/:id/tools",    get(get_tools))
        .with_state(state)
}

async fn list(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.list_agents()))
}

async fn create(State(state): State<Arc<AppState>>, Json(b): Json<AgentBody>) -> Json<Value> {
    let a = state.create_agent(
        "user_demo",
        &b.name,
        b.description.as_deref().unwrap_or(""),
        b.model.as_deref().unwrap_or("claude-opus-4-8"),
    );
    Json(json!(a))
}

async fn get_one(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    match state.get_agent(&id) {
        Some(a) => Json(json!(a)),
        None    => Json(json!({ "error": "not found" })),
    }
}

async fn remove(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    state.agents.write().unwrap().remove(&id);
    Json(json!({ "deleted": true }))
}

/// GET /api/agents/:id/manifest — the agent's instruction manual.
///
/// Every agent ships with a manifest. The manifest tells you:
///   - What the agent does (purpose, description)
///   - What it CAN do (capabilities, tools)
///   - What it CANNOT do (constraints — facts it can never change)
///   - What it costs (budget, model pricing)
///   - What governance applies (accountability, audit trail)
///   - How to invoke it (AIP endpoint, MCP format)
///
/// This is not documentation. This is the contract.
/// Agents are tools. Tools need manuals. The manual IS the contract.
async fn get_manifest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let Some(agent) = state.get_agent(&id) else {
        return Json(json!({ "error": "agent not found", "id": id }));
    };

    // Lifecycle history for this agent
    let lifecycle = state.lifecycle.log_for(&id);
    let current_stage = state.lifecycle.stage_of(&id);
    let usage = state.usage.records_for(&id);
    let total_cost: f64 = usage.iter().map(|r| r.total_usd()).sum();

    Json(json!({
        // ── Identity ─────────────────────────────────────────────────────────
        "agent": {
            "id":          agent.id,
            "name":        agent.name,
            "description": agent.description,
            "model":       agent.model,
            "status":      agent.status,
            "created_at":  agent.created_at,
        },

        // ── Capabilities — what this agent CAN do ────────────────────────────
        "capabilities": agent.capabilities,
        "tools": [
            {
                "name":        "run",
                "description": "Execute this agent on a task",
                "endpoint":    format!("POST /api/runs/{}", id),
                "input":       { "task": "string", "context": "object?" },
            },
            {
                "name":        "observe",
                "description": "Stream this agent's run output in real time",
                "endpoint":    format!("WS /ws/{}", id),
            },
        ],

        // ── Constraints — what this agent CANNOT do ──────────────────────────
        // "Agent can be anything as long as it does not change the facts"
        // These are immutable invariants — not configurable, not bypassable.
        "constraints": {
            "immutable_facts": [
                "accountability_log",    // append-only; no record can be altered or deleted
                "lifecycle_transitions", // gate transitions are final; past stages cannot be reopened
                "usage_records",         // every token cost recorded at the gate; no retroactive adjustment
                "run_outcomes",          // completed/failed runs are facts; they cannot be re-labelled
            ],
            "axioms": {
                "past_is_fact":    "All recorded events are immutable. The past cannot be changed.",
                "real_is_fact":    "The accountability log reflects reality. No drift, no correction.",
                "solid_is_fact":   "Gate records are write-once. No update after transition.",
                "surface_is_fact": "Every boundary is hardened. The attack surface is what it is.",
            },
            "governance": {
                "all_actions_audited":     true,
                "jit_access_only":         true,
                "no_standing_permissions": true,
                "accountability_required": true,
            },
        },

        // ── Lifecycle — where the agent is now ───────────────────────────────
        "lifecycle": {
            "current_stage": current_stage.as_ref().map(|s| s.as_str()),
            "gates_passed":  lifecycle.len(),
            "history": lifecycle.iter().map(|r| json!({
                "stage":  r.stage.as_str(),
                "status": r.status,
                "oath":   r.oath,
                "at":     r.transitioned_at,
            })).collect::<Vec<_>>(),
        },

        // ── Cost — what this agent costs to run ──────────────────────────────
        "cost": {
            "total_usd":     total_cost,
            "usage_records": usage.len(),
            "model":         agent.model,
            "pricing_note":  "Token costs metered at the gate — real-time, micro-cent precision",
            "transparency":  "Every cost visible. Nothing hidden. Freedom, not free.",
        },

        // ── Invocation — how to use this agent ───────────────────────────────
        "invocation": {
            "http": {
                "run":     format!("POST /api/runs"),
                "stream":  format!("WS /ws/{}", id),
                "status":  format!("GET /api/runs/{{run_id}}"),
                "feedback": "POST /api/feedback",
            },
            "aip": {
                "handshake":  "POST /api/aip/handshake",
                "message":    "POST /api/aip/message",
                "capability": "POST /api/aip/capability",
                "type":       "aip.capability.request",
                "payload": {
                    "capability": format!("agent:run:{}", id),
                    "args": { "task": "string" }
                },
            },
            "mcp": {
                "endpoint":   "POST /mcp",
                "protocol":   "JSON-RPC 2.0",
                "tool":       "run_agent",
                "params": { "agent_id": id, "task": "string" },
            },
        },

        // ── The contract ─────────────────────────────────────────────────────
        "contract": {
            "oath":       "I act within my declared capabilities. I do not alter facts. I am accountable for every action.",
            "governed_by": "Autonomyx governance policy — JIT grants, capability-scoped, time-limited",
            "audited_by":  "Autonomyx accountability log — non-repudiable, append-only, Ed25519-signed",
            "billed_by":   "Autonomyx usage meter — micro-cent precision, real-time, no hidden costs",
        },

        // ── Platform ─────────────────────────────────────────────────────────
        "platform": {
            "version":   env!("CARGO_PKG_VERSION"),
            "generated": Utc::now(),
            "note":      "This manifest is the contract. Read it before invoking the agent.",
        },
    }))
}

/// GET /api/agents/:id/tools — tools this agent exposes in MCP tool format.
/// Agents are tools. This endpoint returns the agent's tool definitions
/// in the standard MCP JSON schema format so any MCP client can invoke them.
async fn get_tools(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let Some(agent) = state.get_agent(&id) else {
        return Json(json!({ "error": "agent not found", "id": id }));
    };

    // Core tool: run this agent
    let run_tool = json!({
        "name":        format!("agent_{}_run", id.replace('-', "_")),
        "description": format!("Run agent '{}': {}", agent.name, agent.description),
        "inputSchema": {
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task to execute",
                },
                "context": {
                    "type": "object",
                    "description": "Optional structured context for the run",
                },
            },
            "required": ["task"],
        },
    });

    // Tool for each declared capability
    let capability_tools: Vec<Value> = agent.capabilities.iter().map(|cap| json!({
        "name":        format!("agent_{}_{}", id.replace('-', "_"), cap.replace(':', "_")),
        "description": format!("Invoke '{}' capability on agent '{}'", cap, agent.name),
        "inputSchema": {
            "type": "object",
            "properties": {
                "input": { "type": "object", "description": "Capability-specific input" }
            },
        },
    })).collect();

    let mut tools = vec![run_tool];
    tools.extend(capability_tools);

    Json(json!({
        "agent_id": id,
        "tools":    tools,
        "protocol": "MCP 2024-11-05 (JSON Schema)",
        "note":     "Pass these tool definitions to any MCP client to invoke this agent",
    }))
}
