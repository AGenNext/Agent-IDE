// MCP route — Autonomyx as an MCP server.
// POST /mcp  — JSON-RPC 2.0 dispatch; handles all MCP methods and tool calls.
// GET  /mcp  — server info + tool manifest (for discovery).

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;
use crate::mcp::{JsonRpcRequest, dispatch, tool_list};

async fn mcp_info() -> Json<Value> {
    Json(json!({
        "name":            "autonomyx",
        "version":         env!("CARGO_PKG_VERSION"),
        "protocol":        "MCP 2024-11-05",
        "transport":       "HTTP POST /mcp (JSON-RPC 2.0)",
        "homepage":        "https://openautonomyx.com",
        "description":     "Autonomyx platform as MCP server — agents, apps, gates, fabric, usage",
        "tools":           tool_list(),
        "usage": {
            "initialize":  "POST /mcp {jsonrpc:'2.0',id:1,method:'initialize',params:{protocolVersion:'2024-11-05',...}}",
            "list_tools":  "POST /mcp {jsonrpc:'2.0',id:2,method:'tools/list'}",
            "call_tool":   "POST /mcp {jsonrpc:'2.0',id:3,method:'tools/call',params:{name:'autonomyx_list_agents',arguments:{}}}",
        }
    }))
}

async fn mcp_dispatch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<Value> {
    let http = Arc::new(reqwest::Client::new());
    let resp = dispatch(req, state, http).await;
    Json(serde_json::to_value(resp).unwrap_or(json!({"error": "serialization failed"})))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/mcp", get(mcp_info).post(mcp_dispatch))
        .with_state(state)
}
