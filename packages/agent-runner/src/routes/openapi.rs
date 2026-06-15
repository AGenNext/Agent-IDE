use axum::{Router, routing::get, Json};
use serde_json::{json, Value};

pub fn router() -> Router {
    Router::new()
        .route("/openapi.json", get(openapi_spec))
        .route("/api/routes",   get(route_list))
}

async fn route_list() -> Json<Value> {
    Json(json!({
        "platform": "Autonomyx",
        "version":  env!("CARGO_PKG_VERSION"),
        "base_url": "https://your-host",
        "auth":     "Bearer <API_KEY> — all routes except /health",
        "groups": [
            {
                "name": "Core",
                "routes": [
                    { "method": "GET",    "path": "/health",                      "description": "Platform health + version" },
                    { "method": "GET",    "path": "/api/agents",                  "description": "List agents" },
                    { "method": "POST",   "path": "/api/agents",                  "description": "Create agent" },
                    { "method": "GET",    "path": "/api/agents/:id",              "description": "Get agent" },
                    { "method": "GET",    "path": "/api/agents/:id/manifest",     "description": "Agent instruction manual + constraints" },
                    { "method": "GET",    "path": "/api/agents/:id/tools",        "description": "MCP tool schema for agent capabilities" },
                    { "method": "GET",    "path": "/api/runs",                    "description": "List runs" },
                    { "method": "POST",   "path": "/api/runs",                    "description": "Start a run" },
                    { "method": "GET",    "path": "/api/runs/:id",                "description": "Get run" },
                    { "method": "GET",    "path": "/api/apps",                    "description": "List applications" },
                    { "method": "POST",   "path": "/api/apps",                    "description": "Create application" },
                    { "method": "GET",    "path": "/api/apps/:id",                "description": "Get application" },
                ]
            },
            {
                "name": "Lifecycle + Fabric",
                "routes": [
                    { "method": "POST",   "path": "/api/lifecycle/:artifact/:stage",           "description": "Open a lifecycle gate" },
                    { "method": "GET",    "path": "/api/lifecycle/:artifact/:stage",           "description": "Gate status" },
                    { "method": "GET",    "path": "/api/lifecycle/:artifact/feasibility/:stage","description": "Pre-flight feasibility check" },
                    { "method": "GET",    "path": "/api/fabric/log",                           "description": "Full fabric event log" },
                    { "method": "GET",    "path": "/api/fabric/log/:artifact",                 "description": "Fabric events for one artifact" },
                ]
            },
            {
                "name": "Identity + Federation",
                "routes": [
                    { "method": "GET",    "path": "/api/peers",                   "description": "List federation peers" },
                    { "method": "POST",   "path": "/api/peers",                   "description": "Register peer" },
                    { "method": "GET",    "path": "/api/peers/:id",               "description": "Get peer" },
                    { "method": "DELETE", "path": "/api/peers/:id",               "description": "Remove peer" },
                    { "method": "POST",   "path": "/api/aip/message",             "description": "Agent-to-agent message (DID + Ed25519)" },
                    { "method": "POST",   "path": "/api/aip/verify",              "description": "Verify AIP message signature" },
                ]
            },
            {
                "name": "Blockchain",
                "routes": [
                    { "method": "POST",   "path": "/api/blockchain/did/anchor",   "description": "Anchor DID on-chain" },
                    { "method": "GET",    "path": "/api/blockchain/did/:did",      "description": "Resolve DID document" },
                    { "method": "POST",   "path": "/api/blockchain/accountability","description": "Emit accountability record on-chain" },
                    { "method": "POST",   "path": "/api/blockchain/usage/settle",  "description": "Settle usage on-chain" },
                    { "method": "GET",    "path": "/api/blockchain/governance/:did","description": "On-chain governance check for DID" },
                    { "method": "GET",    "path": "/api/blockchain/summary",       "description": "Chain context + bridge status" },
                ]
            },
            {
                "name": "Storage",
                "routes": [
                    { "method": "POST",   "path": "/api/storage/artifacts",        "description": "Store artifact (policy-gated, versioned)" },
                    { "method": "GET",    "path": "/api/storage/artifacts/:id",    "description": "Retrieve artifact" },
                    { "method": "GET",    "path": "/api/storage/projects",         "description": "List projects" },
                    { "method": "POST",   "path": "/api/storage/projects",         "description": "Create project" },
                    { "method": "GET",    "path": "/api/storage/projects/:id/milestones","description": "Project milestones" },
                ]
            },
            {
                "name": "Governance Graph",
                "routes": [
                    { "method": "GET",    "path": "/api/govgraph/nodes",           "description": "All governance nodes" },
                    { "method": "POST",   "path": "/api/govgraph/nodes",           "description": "Add governance node" },
                    { "method": "GET",    "path": "/api/govgraph/nodes/:id",       "description": "Get node" },
                    { "method": "GET",    "path": "/api/govgraph/paths?from=&to=", "description": "Find governance paths (BFS)" },
                    { "method": "POST",   "path": "/api/govgraph/check",           "description": "Check path traversal for actor" },
                    { "method": "POST",   "path": "/api/govgraph/execute",         "description": "Execute a governance path" },
                ]
            },
            {
                "name": "ComputeKube",
                "routes": [
                    { "method": "POST",   "path": "/api/computekube/jobs",         "description": "Spawn k8s compute job" },
                    { "method": "GET",    "path": "/api/computekube/jobs",         "description": "List jobs" },
                    { "method": "GET",    "path": "/api/computekube/jobs/:id",     "description": "Job status" },
                    { "method": "POST",   "path": "/api/computekube/jobs/:id/cancel","description": "Cancel job" },
                ]
            },
            {
                "name": "Goals",
                "routes": [
                    { "method": "POST",   "path": "/api/goals/missions",           "description": "Declare agent mission" },
                    { "method": "GET",    "path": "/api/goals",                    "description": "List goals" },
                    { "method": "POST",   "path": "/api/goals",                    "description": "Create goal" },
                    { "method": "POST",   "path": "/api/goals/:id/align",          "description": "Run 7-value alignment check" },
                    { "method": "POST",   "path": "/api/goals/:id/activate",       "description": "Activate aligned goal" },
                    { "method": "POST",   "path": "/api/goals/:id/objectives",     "description": "Add objective" },
                    { "method": "POST",   "path": "/api/goals/:id/impact",         "description": "Record impact metric" },
                    { "method": "GET",    "path": "/api/goals/summary",            "description": "Goals summary + impact progress" },
                ]
            },
            {
                "name": "Dashboards",
                "routes": [
                    { "method": "GET",    "path": "/api/dashboard",                "description": "List dashboards (4 built-in + custom)" },
                    { "method": "POST",   "path": "/api/dashboard",                "description": "Create custom dashboard" },
                    { "method": "GET",    "path": "/api/dashboard/:id",            "description": "Get dashboard" },
                    { "method": "GET",    "path": "/api/dashboard/:id/render",     "description": "Render dashboard with live data" },
                ]
            },
            {
                "name": "Plugins",
                "routes": [
                    { "method": "GET",    "path": "/api/plugins",                  "description": "List all plugins (12 built-in)" },
                    { "method": "POST",   "path": "/api/plugins",                  "description": "Register custom plugin" },
                    { "method": "GET",    "path": "/api/plugins/summary",          "description": "Plugin counts by kind" },
                    { "method": "GET",    "path": "/api/plugins/capabilities",     "description": "All available capabilities" },
                    { "method": "GET",    "path": "/api/plugins/:id",              "description": "Get plugin" },
                    { "method": "POST",   "path": "/api/plugins/:id/enable",       "description": "Enable plugin" },
                    { "method": "POST",   "path": "/api/plugins/:id/disable",      "description": "Disable plugin" },
                    { "method": "GET",    "path": "/api/plugins/:id/nodes",        "description": "Plugin governance nodes" },
                ]
            },
            {
                "name": "Search",
                "routes": [
                    { "method": "GET",    "path": "/api/search?q=&kinds=&limit=",  "description": "Universal open search across all platform data" },
                ]
            },
            {
                "name": "Opt-in",
                "routes": [
                    { "method": "POST",   "path": "/api/optin/extend",             "description": "Register a capability / governance node" },
                    { "method": "POST",   "path": "/api/optin/align",              "description": "7-value alignment check with verdict + guidance" },
                    { "method": "GET",    "path": "/api/optin",                    "description": "List all opt-ins" },
                    { "method": "GET",    "path": "/api/optin/summary",            "description": "Opt-in stats" },
                    { "method": "GET",    "path": "/api/optin/:id",                "description": "Get opt-in record" },
                    { "method": "POST",   "path": "/api/optin/:id/activate",       "description": "Activate approved opt-in" },
                    { "method": "POST",   "path": "/api/optin/:id/withdraw",       "description": "Withdraw opt-in" },
                ]
            },
            {
                "name": "Platform + Usage",
                "routes": [
                    { "method": "GET",    "path": "/api/platform",                 "description": "Platform identity + world model" },
                    { "method": "GET",    "path": "/api/usage",                    "description": "Usage metrics + billing summary" },
                    { "method": "GET",    "path": "/api/infra",                    "description": "Infrastructure status" },
                ]
            },
            {
                "name": "WebSocket",
                "routes": [
                    { "method": "WS",     "path": "/ws/:run_id",                   "description": "Run event stream" },
                    { "method": "WS",     "path": "/ws/fabric",                    "description": "Fabric event stream" },
                    { "method": "WS",     "path": "/ws/stream",                    "description": "Unified platform stream" },
                ]
            },
            {
                "name": "Protocols",
                "routes": [
                    { "method": "POST",   "path": "/mcp",                          "description": "MCP JSON-RPC 2.0 — platform as AI tool" },
                    { "method": "POST",   "path": "/transfer",                     "description": "Peer egress push (egress-only, no inbound)" },
                ]
            },
            {
                "name": "Discovery",
                "routes": [
                    { "method": "GET",    "path": "/openapi.json",                 "description": "OpenAPI 3.1 spec (this endpoint)" },
                    { "method": "GET",    "path": "/api/routes",                   "description": "All routes with descriptions" },
                ]
            }
        ]
    }))
}

async fn openapi_spec() -> Json<Value> {
    Json(json!({
        "openapi":  "3.1.0",
        "info": {
            "title":       "Autonomyx Platform API",
            "version":     env!("CARGO_PKG_VERSION"),
            "description": "Next-generation multi-ecosystem AI agent platform. Govern, deploy, observe, align. openautonomyx.com",
            "contact": {
                "name": "Autonomyx",
                "url":  "https://openautonomyx.com"
            }
        },
        "servers": [
            { "url": "https://your-host", "description": "Production" },
            { "url": "http://localhost:3001", "description": "Local dev" }
        ],
        "security": [
            { "bearerAuth": [] }
        ],
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type":   "http",
                    "scheme": "bearer",
                    "description": "Set AUTONOMYX_API_KEY env var on the server. Pass as Authorization: Bearer <key>"
                }
            },
            "schemas": {
                "Agent": {
                    "type": "object",
                    "properties": {
                        "id":           { "type": "string", "example": "agent_4f3a1b2c" },
                        "owner_id":     { "type": "string" },
                        "name":         { "type": "string" },
                        "description":  { "type": "string" },
                        "model":        { "type": "string", "example": "claude-opus-4-8" },
                        "status":       { "type": "string", "enum": ["idle", "running", "error"] },
                        "capabilities": { "type": "array", "items": { "type": "string" } },
                        "created_at":   { "type": "string", "format": "date-time" },
                        "updated_at":   { "type": "string", "format": "date-time" }
                    }
                },
                "Run": {
                    "type": "object",
                    "properties": {
                        "run_id":       { "type": "string", "example": "run_9e1d4f2a" },
                        "agent_id":     { "type": "string" },
                        "agent_name":   { "type": "string" },
                        "model":        { "type": "string" },
                        "task":         { "type": "string" },
                        "status":       { "type": "string", "enum": ["running", "completed", "failed", "cancelled"] },
                        "steps":        { "type": "array" },
                        "started_at":   { "type": "string", "format": "date-time" },
                        "completed_at": { "type": "string", "format": "date-time", "nullable": true }
                    }
                },
                "Goal": {
                    "type": "object",
                    "properties": {
                        "id":               { "type": "string" },
                        "agent_id":         { "type": "string" },
                        "title":            { "type": "string" },
                        "description":      { "type": "string" },
                        "intended_impact":  { "type": "string" },
                        "status":           { "type": "string", "enum": ["draft","aligned","active","achieved","abandoned","rejected"] },
                        "alignment":        { "type": "object", "nullable": true },
                        "impact_metrics":   { "type": "array" },
                        "tags":             { "type": "array", "items": { "type": "string" } }
                    }
                },
                "OptIn": {
                    "type": "object",
                    "properties": {
                        "id":          { "type": "string", "example": "optin_2a3b4c5d" },
                        "kind":        { "type": "string", "enum": ["extend", "align"] },
                        "actor_did":   { "type": "string" },
                        "name":        { "type": "string" },
                        "status":      { "type": "string", "enum": ["pending","approved","rejected","active","withdrawn"] },
                        "alignment":   { "type": "object", "nullable": true },
                        "extension":   { "type": "object", "nullable": true }
                    }
                },
                "SearchResult": {
                    "type": "object",
                    "properties": {
                        "kind":    { "type": "string" },
                        "id":      { "type": "string" },
                        "title":   { "type": "string" },
                        "excerpt": { "type": "string" },
                        "score":   { "type": "number" },
                        "source":  { "type": "string" },
                        "meta":    { "type": "object" }
                    }
                },
                "Plugin": {
                    "type": "object",
                    "properties": {
                        "id":           { "type": "string" },
                        "name":         { "type": "string" },
                        "version":      { "type": "string" },
                        "kind":         { "type": "string" },
                        "enabled":      { "type": "boolean" },
                        "capabilities": { "type": "array", "items": { "type": "string" } },
                        "homepage":     { "type": "string", "nullable": true }
                    }
                }
            }
        },
        "paths": {
            "/health":                     { "get":    { "summary": "Platform health", "security": [], "tags": ["Core"] } },
            "/openapi.json":               { "get":    { "summary": "OpenAPI spec", "security": [], "tags": ["Discovery"] } },
            "/api/routes":                 { "get":    { "summary": "All routes", "security": [], "tags": ["Discovery"] } },
            "/api/agents":                 { "get":    { "summary": "List agents", "tags": ["Core"] },
                                             "post":   { "summary": "Create agent", "tags": ["Core"] } },
            "/api/agents/{id}":            { "get":    { "summary": "Get agent", "tags": ["Core"] } },
            "/api/agents/{id}/manifest":   { "get":    { "summary": "Agent manifest", "tags": ["Core"] } },
            "/api/agents/{id}/tools":      { "get":    { "summary": "Agent tools", "tags": ["Core"] } },
            "/api/runs":                   { "get":    { "summary": "List runs", "tags": ["Core"] },
                                             "post":   { "summary": "Start run", "tags": ["Core"] } },
            "/api/runs/{id}":              { "get":    { "summary": "Get run", "tags": ["Core"] } },
            "/api/apps":                   { "get":    { "summary": "List apps", "tags": ["Core"] },
                                             "post":   { "summary": "Create app", "tags": ["Core"] } },
            "/api/plugins":                { "get":    { "summary": "List plugins", "tags": ["Plugins"] },
                                             "post":   { "summary": "Register plugin", "tags": ["Plugins"] } },
            "/api/plugins/summary":        { "get":    { "summary": "Plugin summary", "tags": ["Plugins"] } },
            "/api/plugins/capabilities":   { "get":    { "summary": "All capabilities", "tags": ["Plugins"] } },
            "/api/plugins/{id}/enable":    { "post":   { "summary": "Enable plugin", "tags": ["Plugins"] } },
            "/api/plugins/{id}/disable":   { "post":   { "summary": "Disable plugin", "tags": ["Plugins"] } },
            "/api/search":                 { "get":    { "summary": "Universal search", "tags": ["Search"],
                                                         "parameters": [
                                                             { "name": "q",     "in": "query", "schema": { "type": "string" } },
                                                             { "name": "kinds", "in": "query", "schema": { "type": "string" }, "description": "Comma-separated: agent,run,app,peer,goal,node,plugin,event,record,dashboard" },
                                                             { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 50 } }
                                                         ] } },
            "/api/optin/extend":           { "post":   { "summary": "Register capability / governance node", "tags": ["Opt-in"] } },
            "/api/optin/align":            { "post":   { "summary": "7-value alignment check", "tags": ["Opt-in"] } },
            "/api/optin":                  { "get":    { "summary": "List opt-ins", "tags": ["Opt-in"] } },
            "/api/optin/summary":          { "get":    { "summary": "Opt-in summary", "tags": ["Opt-in"] } },
            "/api/optin/{id}":             { "get":    { "summary": "Get opt-in", "tags": ["Opt-in"] } },
            "/api/optin/{id}/activate":    { "post":   { "summary": "Activate opt-in", "tags": ["Opt-in"] } },
            "/api/optin/{id}/withdraw":    { "post":   { "summary": "Withdraw opt-in", "tags": ["Opt-in"] } },
            "/api/goals":                  { "get":    { "summary": "List goals", "tags": ["Goals"] },
                                             "post":   { "summary": "Create goal", "tags": ["Goals"] } },
            "/api/goals/{id}/align":       { "post":   { "summary": "Alignment check", "tags": ["Goals"] } },
            "/api/goals/{id}/activate":    { "post":   { "summary": "Activate goal", "tags": ["Goals"] } },
            "/api/goals/{id}/impact":      { "post":   { "summary": "Record impact", "tags": ["Goals"] } },
            "/api/goals/summary":          { "get":    { "summary": "Goals summary", "tags": ["Goals"] } },
            "/api/dashboard":              { "get":    { "summary": "List dashboards", "tags": ["Dashboards"] },
                                             "post":   { "summary": "Create dashboard", "tags": ["Dashboards"] } },
            "/api/dashboard/{id}/render":  { "get":    { "summary": "Render dashboard live", "tags": ["Dashboards"] } },
            "/api/govgraph/nodes":         { "get":    { "summary": "Governance nodes", "tags": ["Governance"] },
                                             "post":   { "summary": "Add node", "tags": ["Governance"] } },
            "/api/govgraph/paths":         { "get":    { "summary": "Find paths (BFS)", "tags": ["Governance"] } },
            "/api/govgraph/check":         { "post":   { "summary": "Check path traversal", "tags": ["Governance"] } },
            "/api/govgraph/execute":       { "post":   { "summary": "Execute governance path", "tags": ["Governance"] } },
            "/api/blockchain/did/anchor":  { "post":   { "summary": "Anchor DID on-chain", "tags": ["Blockchain"] } },
            "/api/blockchain/summary":     { "get":    { "summary": "Chain status", "tags": ["Blockchain"] } },
            "/mcp":                        { "post":   { "summary": "MCP JSON-RPC 2.0", "tags": ["Protocols"] } }
        }
    }))
}
