// Autonomyx MCP Server — Model Context Protocol
//
// Exposes the Autonomyx platform as an MCP server so any MCP-compatible AI
// (Claude, any OpenAI-compatible model) can use the platform as a tool.
//
// Tools exposed:
//   autonomyx_list_agents      — list registered agents
//   autonomyx_create_agent     — create a new agent
//   autonomyx_run_agent        — execute an agent on a task
//   autonomyx_list_apps        — list applications
//   autonomyx_create_app       — create an application from .ayx declaration
//   autonomyx_gate_transition  — trigger a lifecycle gate
//   autonomyx_usage_summary    — real-time usage and cost
//   autonomyx_platform_info    — platform identity and capabilities
//
// Wire format: JSON-RPC 2.0 over HTTP POST /mcp
// MCP spec: https://spec.modelcontextprotocol.io
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

// ── JSON-RPC 2.0 ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id:      Value,
    pub method:  String,
    pub params:  Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id:      Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result:  Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error:   Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code:    i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }
    pub fn err(id: Value, code: i32, message: String) -> Self {
        Self { jsonrpc: "2.0", id, result: None, error: Some(JsonRpcError { code, message }) }
    }
}

// ── MCP tool definitions ──────────────────────────────────────────────────────

pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "autonomyx_list_agents",
                "description": "List all registered agents on the Autonomyx platform.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "autonomyx_create_agent",
                "description": "Create a new agent on the Autonomyx platform with a name, model, and description.",
                "inputSchema": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name":        { "type": "string", "description": "Agent name" },
                        "description": { "type": "string" },
                        "model":       { "type": "string", "default": "claude-opus-4-8" },
                        "owner_id":    { "type": "string" }
                    }
                }
            },
            {
                "name": "autonomyx_run_agent",
                "description": "Execute an agent on a task. Returns the run ID and first steps.",
                "inputSchema": {
                    "type": "object",
                    "required": ["agent_id", "task"],
                    "properties": {
                        "agent_id": { "type": "string" },
                        "task":     { "type": "string", "description": "What the agent should do" },
                        "model":    { "type": "string" }
                    }
                }
            },
            {
                "name": "autonomyx_list_apps",
                "description": "List all applications on the platform. Applications are the product.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "autonomyx_create_app",
                "description": "Create a new application from a .ayx declaration. The theory becomes real.",
                "inputSchema": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name":        { "type": "string" },
                        "description": { "type": "string" },
                        "version":     { "type": "string", "default": "0.1.0" },
                        "ayx_source":  { "type": "string", "description": ".ayx declaration source" },
                        "owner_id":    { "type": "string" }
                    }
                }
            },
            {
                "name": "autonomyx_gate_transition",
                "description": "Trigger an idempotent lifecycle gate transition (build/sign/push/sync/deploy/run/observe/feedback).",
                "inputSchema": {
                    "type": "object",
                    "required": ["artifact", "stage"],
                    "properties": {
                        "artifact":  { "type": "string", "description": "Artifact or app ID" },
                        "stage":     {
                            "type": "string",
                            "enum": ["build","sign","push","sync","deploy","run","observe","feedback"]
                        },
                        "payload":   { "type": "object" },
                        "actor_did": { "type": "string" }
                    }
                }
            },
            {
                "name": "autonomyx_usage_summary",
                "description": "Get real-time platform usage summary — costs, budgets, provider breakdown.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "autonomyx_platform_info",
                "description": "Get Autonomyx platform identity, cloud context, capabilities, and world model.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

// ── Tool dispatcher ───────────────────────────────────────────────────────────

pub async fn dispatch(
    req: JsonRpcRequest,
    state: Arc<AppState>,
    http: Arc<reqwest::Client>,
) -> JsonRpcResponse {
    let id     = req.id.clone();
    let params = req.params.unwrap_or(json!({}));

    match req.method.as_str() {

        // MCP protocol methods
        "initialize" => JsonRpcResponse::ok(id, json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name":    "autonomyx",
                "version": env!("CARGO_PKG_VERSION"),
            }
        })),

        "tools/list" => JsonRpcResponse::ok(id, tool_list()),

        "tools/call" => {
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args      = params.get("arguments").cloned().unwrap_or(json!({}));

            let result = call_tool(tool_name, args, state, http).await;
            match result {
                Ok(v)  => JsonRpcResponse::ok(id, json!({
                    "content": [{ "type": "text", "text": v.to_string() }]
                })),
                Err(e) => JsonRpcResponse::err(id, -32603, e),
            }
        }

        "notifications/initialized" => JsonRpcResponse::ok(id, json!({})),
        "ping"                      => JsonRpcResponse::ok(id, json!({})),

        unknown => JsonRpcResponse::err(id, -32601, format!("method not found: {unknown}")),
    }
}

async fn call_tool(
    name:  &str,
    args:  Value,
    state: Arc<AppState>,
    _http: Arc<reqwest::Client>,
) -> Result<Value, String> {
    match name {
        "autonomyx_list_agents" => {
            Ok(json!(state.list_agents()))
        }

        "autonomyx_create_agent" => {
            let n = args["name"].as_str().ok_or("name required")?;
            let d = args["description"].as_str().unwrap_or("");
            let m = args["model"].as_str().unwrap_or("claude-opus-4-8");
            let o = args["owner_id"].as_str().unwrap_or("mcp_caller");
            Ok(json!(state.create_agent(o, n, d, m)))
        }

        "autonomyx_run_agent" => {
            let agent_id = args["agent_id"].as_str().ok_or("agent_id required")?;
            let task     = args["task"].as_str().ok_or("task required")?;
            let agent    = state.get_agent(agent_id).ok_or("agent not found")?;
            let model    = args["model"].as_str().unwrap_or(&agent.model).to_string();
            let run      = state.create_run(agent_id, &agent.name, &model, task);
            Ok(json!({ "run_id": run.run_id, "status": "running", "task": task }))
        }

        "autonomyx_list_apps" => {
            Ok(json!(state.list_apps()))
        }

        "autonomyx_create_app" => {
            let n = args["name"].as_str().ok_or("name required")?;
            let d = args["description"].as_str().unwrap_or("");
            let v = args["version"].as_str().unwrap_or("0.1.0");
            let s = args["ayx_source"].as_str();
            let o = args["owner_id"].as_str().unwrap_or("mcp_caller");
            let app = state.create_app(o, n, d, v, s);
            Ok(json!(app))
        }

        "autonomyx_gate_transition" => {
            use crate::lifecycle::{Gate, Stage};
            let artifact = args["artifact"].as_str().ok_or("artifact required")?;
            let stage_str = args["stage"].as_str().ok_or("stage required")?;
            let payload  = args.get("payload").cloned().unwrap_or(json!({}));
            let stage: Stage = serde_json::from_value(json!(stage_str))
                .map_err(|e| e.to_string())?;
            let gate = Gate::new(&state.lifecycle, artifact);
            let rec = match stage {
                Stage::Build    => gate.build(&payload),
                Stage::Sign     => gate.sign(&payload),
                Stage::Push     => gate.push(&payload),
                Stage::Sync     => gate.sync(&payload),
                Stage::Deploy   => gate.deploy(&payload),
                Stage::Run      => gate.run(&payload),
                Stage::Observe  => gate.observe(&payload),
                Stage::Feedback => gate.feedback(&payload),
            };
            state.fabric.emit_gate(&rec, payload);
            Ok(json!({
                "artifact": rec.artifact,
                "stage":    rec.stage.as_str(),
                "status":   rec.status,
                "oath":     rec.oath,
            }))
        }

        "autonomyx_usage_summary" => {
            Ok(state.usage.summary())
        }

        "autonomyx_platform_info" => {
            use crate::cloud::PlatformIdentity;
            let id = PlatformIdentity::new();
            Ok(json!({
                "name":       id.name,
                "version":    id.version,
                "protocol":   id.protocol,
                "philosophy": id.philosophy,
                "cloud":      id.cloud,
                "capabilities": id.capabilities,
                "ecosystems":   id.ecosystems,
            }))
        }

        unknown => Err(format!("unknown tool: {unknown}")),
    }
}
