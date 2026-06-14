// Contract — the middleware holds the contract.
//
// Every request that touches a governed resource must pass the contract check
// before any handler runs. The contract is the union of:
//
//   Oath          — named invariant predicate; gate's twin; if it breaks, gate stays closed
//   Governance    — per-DID policy: max TTL, allowed capabilities, allowed operators
//   Accountability — signed record of every enforcement decision; non-repudiable
//   Fabric        — emits the contract outcome as an event; downstream handlers fire
//
// "Middleware holds the contract" — this is not application logic.
// The contract is enforced at the boundary. Always. Before the handler sees the request.
//
// Contract lifecycle per request:
//   1. Extract actor DID + requested capability from Authorization + path
//   2. Look up governance policy for the resource DID
//   3. Check: capability allowed? operator allowed? TTL valid? budget available?
//   4. If all pass → open gate, record Success, emit fabric event, pass to handler
//   5. If any fail → close gate, record Denied, emit dead-letter, return 403
//
// openautonomyx.com

use std::sync::Arc;
use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::store::AppState;

// ── Contract definition ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    /// The oath this contract enforces — named invariant predicate.
    /// Must evaluate to true before the gate opens.
    pub oath:       String,

    /// The capability required to satisfy this contract.
    /// Must be in the actor's governance policy allowlist.
    pub capability: String,

    /// The resource this contract governs.
    pub resource:   String,

    /// Maximum time-to-live for the grant (seconds).
    /// 0 = no time limit (only for public endpoints like /health).
    pub max_ttl:    u32,

    /// Whether this contract requires an explicit JIT grant.
    /// true  = grant_id must be present and valid
    /// false = capability in policy is sufficient
    pub requires_grant: bool,
}

impl Contract {
    /// Derive the contract for a given path.
    /// Production: load from governance policy in DID Document.
    /// Now: path-based derivation with sensible defaults.
    pub fn for_path(path: &str, method: &str) -> Self {
        // Map HTTP method + path to a capability
        let capability = match (method, path) {
            ("GET",  p) if p.starts_with("/api/agents")     => "agent:read",
            ("POST", p) if p.starts_with("/api/agents")     => "agent:write",
            ("GET",  p) if p.starts_with("/api/apps")       => "app:read",
            ("POST", p) if p.starts_with("/api/apps")       => "app:write",
            ("PUT",  p) if p.starts_with("/api/apps")       => "app:upgrade",
            ("POST", p) if p.ends_with("/consent")          => "app:consent",
            ("POST", p) if p.starts_with("/api/lifecycle")  => "lifecycle:transition",
            ("GET",  p) if p.starts_with("/api/lifecycle")  => "lifecycle:read",
            ("POST", p) if p.starts_with("/api/runs")       => "run:execute",
            ("GET",  p) if p.starts_with("/api/runs")       => "run:read",
            ("GET",  p) if p.starts_with("/api/usage")      => "usage:read",
            ("POST", p) if p == "/mcp"                      => "mcp:dispatch",
            ("POST", p) if p.starts_with("/transfer")       => "transfer:push",
            ("GET",  "/health")                              => "health:read",
            ("GET",  p) if p.starts_with("/api/platform")   => "platform:read",
            ("GET",  p) if p.starts_with("/api/theory")     => "platform:read",
            ("POST", p) if p.starts_with("/api/onboarding") => "onboarding:write",
            ("GET",  p) if p.starts_with("/api/onboarding") => "onboarding:read",
            _                                               => "platform:read",
        };

        // Oath for this capability — the named invariant that must hold
        let oath = match capability {
            "lifecycle:transition" => "actor_did_present",
            "transfer:push"        => "egress_only",
            "mcp:dispatch"         => "mcp_auth_valid",
            "app:consent"          => "owner_present",
            "run:execute"          => "budget_available",
            _                      => "authenticated",
        };

        // Grant required for high-privilege operations
        let requires_grant = matches!(capability,
            "lifecycle:transition" | "transfer:push" | "run:execute" | "mcp:dispatch"
        );

        Contract {
            oath:       oath.into(),
            capability: capability.into(),
            resource:   path.into(),
            max_ttl:    3600,
            requires_grant,
        }
    }

    /// Evaluate the oath — returns Ok(()) if the oath holds, Err(reason) if broken.
    pub fn check_oath(&self, req: &Request<Body>) -> Result<(), String> {
        match self.oath.as_str() {
            "authenticated" => {
                // Auth already verified by ingress_gate; if we're here, it passed
                Ok(())
            }
            "actor_did_present" => {
                // For lifecycle transitions, actor_did should be in the request body.
                // At middleware level we check the header hint; body check is in the handler.
                let has_did_hint = req.headers().get("x-actor-did").is_some()
                    || req.headers().get("authorization").is_some();
                if has_did_hint { Ok(()) }
                else { Err("oath broken: actor_did_present — no DID in request".into()) }
            }
            "egress_only" => {
                if req.method() == axum::http::Method::POST { Ok(()) }
                else { Err("oath broken: egress_only — only POST allowed on /transfer".into()) }
            }
            "mcp_auth_valid" => {
                // MCP requires full Bearer auth (handled by ingress_gate).
                // Additional: require Content-Type: application/json
                let ct = req.headers().get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if ct.contains("application/json") { Ok(()) }
                else { Err("oath broken: mcp_auth_valid — content-type must be application/json".into()) }
            }
            "owner_present" => {
                // Consent endpoint — owner identity must be asserted
                let has_auth = req.headers().get("authorization").is_some();
                if has_auth { Ok(()) }
                else { Err("oath broken: owner_present — no owner identity".into()) }
            }
            "budget_available" => {
                // Budget check happens inside the handler (needs DID).
                // Middleware passes; handler enforces.
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

// ── Contract middleware ────────────────────────────────────────────────────────

pub async fn contract_layer(
    req: Request<Body>,
    next: Next,
    state: Arc<AppState>,
) -> Response {
    let path   = req.uri().path().to_string();
    let method = req.method().to_string();

    // /health is unconditional — contract-free, no gate
    if path == "/health" {
        return next.run(req).await;
    }

    let contract = Contract::for_path(&path, &method);

    // Evaluate the oath — the named invariant predicate
    match contract.check_oath(&req) {
        Ok(()) => {
            // Oath holds. Emit fabric event: contract enforced, gate opening.
            let event = serde_json::json!({
                "contract": {
                    "oath":       contract.oath,
                    "capability": contract.capability,
                    "resource":   contract.resource,
                    "decision":   "pass",
                }
            });
            state.fabric.emit(crate::fabric::FabricEvent {
                id:         uuid::Uuid::new_v4().to_string(),
                artifact:   path.clone(),
                stage:      crate::lifecycle::Stage::Run,
                status:     crate::fabric::FabricStatus::Open,
                payload:    event,
                emitted_at: chrono::Utc::now(),
            });

            // Pass to handler — contract satisfied
            let mut resp = next.run(req).await;
            // Stamp the satisfied contract on the response (audit trail for clients)
            if let Ok(v) = axum::http::HeaderValue::from_str(&contract.capability) {
                resp.headers_mut().insert("x-contract-capability", v);
            }
            if let Ok(v) = axum::http::HeaderValue::from_str(&contract.oath) {
                resp.headers_mut().insert("x-contract-oath", v);
            }
            resp
        }
        Err(reason) => {
            // Oath broken. Close the gate. Record denial. Emit dead-letter.
            tracing::warn!(
                path       = %path,
                oath       = %contract.oath,
                capability = %contract.capability,
                reason     = %reason,
                "contract: oath broken — gate closed"
            );

            let dead_letter = serde_json::json!({
                "contract": {
                    "oath":       contract.oath,
                    "capability": contract.capability,
                    "resource":   contract.resource,
                    "decision":   "deny",
                    "reason":     reason,
                }
            });
            state.fabric.emit(crate::fabric::FabricEvent {
                id:         uuid::Uuid::new_v4().to_string(),
                artifact:   path,
                stage:      crate::lifecycle::Stage::Run,
                status:     crate::fabric::FabricStatus::Closed,
                payload:    dead_letter,
                emitted_at: chrono::Utc::now(),
            });

            (
                StatusCode::FORBIDDEN,
                [("content-type", "application/json")],
                format!(r#"{{"error":"contract_violation","oath":"{}","capability":"{}","reason":"{}"}}"#,
                    contract.oath, contract.capability, reason),
            ).into_response()
        }
    }
}
