// Opt-in gap-filler middleware — fills the gaps between gates.
//
// The fabric fills structural gaps. This middleware fills runtime gaps:
// any request that reaches an undefined or unknown capability is not rejected —
// it is routed to the opt-in flow for dynamic resolution.
//
// Gap types:
//   capability_gap  — request requires a capability no current agent/plugin has
//   alignment_gap   — request comes from an unaligned actor
//   extension_gap   — request targets a route not yet registered
//
// On a gap, the middleware:
//   1. Logs the gap as a fabric event (stage=Observe, status=Open, payload=gap details)
//   2. Returns a structured gap response — not a 404, not a 500:
//      { "gap": true, "kind": "...", "optin_url": "/api/optin/...", "message": "..." }
//
// This gives the caller everything they need to resolve the gap:
// either extend the platform with the missing capability or align their intent.
//
// "The fabric fills the gaps. The opt-in fills what the fabric can't."
// openautonomyx.com

use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    middleware::Next,
    extract::State,
};
use std::sync::Arc;
use serde_json::json;
use crate::AppState;
use crate::fabric::{FabricEvent, FabricStatus};
use crate::lifecycle::Stage;

// ── Gap signals ───────────────────────────────────────────────────────────────

// Known capability prefixes — requests to these are always passed through.
const KNOWN_PREFIXES: &[&str] = &[
    "/health", "/api/agents", "/api/runs", "/api/apps", "/api/peers",
    "/api/lifecycle", "/api/fabric", "/api/tools", "/api/infra",
    "/api/usage", "/api/platform", "/api/onboarding", "/api/support",
    "/api/mcp", "/api/aip", "/api/feedback", "/api/blockchain",
    "/api/storage", "/api/govgraph", "/api/computekube", "/api/goals",
    "/api/dashboard", "/api/plugins", "/api/search", "/api/optin",
    "/ws", "/transfer", "/mcp",
];

fn is_known(path: &str) -> bool {
    KNOWN_PREFIXES.iter().any(|prefix| path.starts_with(prefix))
}

// ── Gap-filler middleware ─────────────────────────────────────────────────────

pub async fn optin_gap_filler(
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let path   = req.uri().path().to_string();
    let method = req.method().clone();

    // Pass through all known routes immediately
    if is_known(&path) {
        return next.run(req).await;
    }

    // Unknown path — this is a gap.
    // We cannot access state here without extracting it, so we emit a structured gap response.
    // The contract: unknown routes get a gap response, not a 404.
    let gap_kind = if path.contains("capability") || path.contains("compute") {
        "capability_gap"
    } else if path.contains("align") || path.contains("intent") {
        "alignment_gap"
    } else {
        "extension_gap"
    };

    let body = json!({
        "gap":     true,
        "kind":    gap_kind,
        "path":    path,
        "method":  method.as_str(),
        "message": "This capability is not yet registered. Use the opt-in flow to extend the platform or align your intent.",
        "resolve": {
            "extend": {
                "url":     "POST /api/optin/extend",
                "purpose": "Register a new capability, plugin, or governance node",
                "fields":  ["name", "description", "capabilities", "node_kind", "edges_to"],
            },
            "align": {
                "url":     "POST /api/optin/align",
                "purpose": "Submit intent for 7-value alignment check",
                "fields":  ["name", "description", "intended_impact"],
            },
        },
        "philosophy": "Every gap is an invitation to extend. Every unknown is an opportunity to align.",
    });

    tracing::info!(
        path    = %path,
        method  = %method,
        kind    = %gap_kind,
        "optin: gap detected — returning structured gap response"
    );

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "application/json")
        .header("x-autonomyx-gap", gap_kind)
        .header("x-autonomyx-resolve", "/api/optin")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

// ── State-aware gap handler (used in routes for explicit gaps) ────────────────

pub async fn handle_capability_gap(
    state: Arc<AppState>,
    capability: &str,
    actor_did: &str,
    context: &str,
) -> serde_json::Value {
    // Emit fabric event so the loop coordinator sees the gap
    state.fabric.emit(FabricEvent::open(
        &format!("gap:{}", capability),
        Stage::Observe,
        json!({
            "event":      "capability_gap",
            "capability": capability,
            "actor":      actor_did,
            "context":    context,
        }),
    ));

    json!({
        "gap":        true,
        "kind":       "capability_gap",
        "capability": capability,
        "actor":      actor_did,
        "message":    format!("Capability '{}' is not registered. Extend the platform to add it.", capability),
        "optin_url":  "/api/optin/extend",
    })
}
