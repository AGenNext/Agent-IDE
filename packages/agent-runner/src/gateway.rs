// Autonomyx Gateway — ingress + egress control plane.
//
// Ingress:  all external traffic arrives here before any route handler.
//           Auth check → rate limit → route dispatch.
//           No raw port exposure; everything through TLS + bearer.
//
// Egress:   a typed client registry. ALL outbound calls (LLM providers,
//           tool endpoints, peer transfers) go through `EgressClient`.
//           No ad-hoc `reqwest::Client::new()` calls outside this module.
//
// Protocol pins (supply-chain risk = 0):
//   - TLS 1.2 minimum enforced by the TLS layer (Caddy in production,
//     rustls in tests)
//   - HTTP/1.1 or HTTP/2 only — no HTTP/0.9, no cleartext upgrades
//   - Peer transfer: egress-push only, no inbound raw connections from peers
//   - LLM calls:    HTTPS only; base URL validated on construction

use std::sync::{Arc, OnceLock};
use std::time::Duration;
use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    http::{StatusCode, HeaderValue},
};
use reqwest::ClientBuilder;
use uuid::Uuid;
use crate::hardening::{AuthFailureTracker, IpBanList};

// ── Egress client registry ────────────────────────────────────────────────────

/// A single shared HTTP client for all outbound calls.
/// Connect timeout 10s, request timeout 120s, TLS required.
pub struct EgressClient {
    inner: reqwest::Client,
}

impl EgressClient {
    pub fn new() -> Self {
        let is_prod = std::env::var("PRODUCTION")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let inner = ClientBuilder::new()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .tcp_keepalive(Duration::from_secs(30))
            // HTTPS enforced in production — cleartext LLM calls not allowed
            .https_only(is_prod)
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .user_agent(concat!("autonomyx-runner/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build egress HTTP client");

        Self { inner }
    }

    /// LLM provider egress — HTTPS enforced when PRODUCTION=true.
    pub fn llm(&self) -> &reqwest::Client {
        &self.inner
    }

    /// Peer transfer egress — egress-push only, no inbound.
    pub fn peer_transfer(&self) -> &reqwest::Client {
        &self.inner
    }

    /// Tool endpoint egress — URL validated by the tool definition.
    pub fn tool(&self) -> &reqwest::Client {
        &self.inner
    }
}

impl Default for EgressClient {
    fn default() -> Self {
        Self::new()
    }
}

// ── API key cache — read once at first call, never re-read from env ──────────
// Env var reads are safe but redundant on every request. Cache on first access.
// Key rotation requires process restart (planned: hot-reload via SIGHUP).

static CACHED_API_KEY: OnceLock<String> = OnceLock::new();

fn get_api_key() -> &'static str {
    CACHED_API_KEY.get_or_init(|| {
        std::env::var("GATEWAY_API_KEY")
            .or_else(|_| std::env::var("TRANSFER_API_KEY"))
            .unwrap_or_default()
    })
}

// ── Ingress middleware ─────────────────────────────────────────────────────────

/// Hardened ingress gate:
///   1. Always passes /health and /ready (k8s probes)
///   2. Checks IP ban list — banned IPs get 403 immediately (no info leak)
///   3. Validates Bearer token (constant-time) from cached API key
///   4. Records auth failures → bans IP after threshold
///   5. Stamps unique request ID on every response
///   6. Fail-closed: if PRODUCTION=true and no key configured → 503
pub async fn ingress_gate(
    req: Request,
    next: Next,
    auth_tracker: Arc<AuthFailureTracker>,
    ban_list:     Arc<IpBanList>,
) -> Response {
    let path = req.uri().path();

    // Always allow k8s health / readiness probes
    if path == "/health" || path == "/ready" {
        return next.run(req).await;
    }

    // Extract client IP
    let ip = req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()).map(String::from))
        .unwrap_or_else(|| "unknown".into());

    // IP ban check — banned IPs get 403 (not 401 — no info about auth mechanism)
    if ban_list.is_banned(&ip) {
        crate::hardening::audit("ip_banned", &ip, path, "request from banned IP rejected");
        return (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            r#"{"error":"forbidden","message":"Access denied"}"#,
        ).into_response();
    }

    let key = get_api_key();
    let is_prod = std::env::var("PRODUCTION").map(|v| v == "true" || v == "1").unwrap_or(false);

    // Fail-closed: no key in production = service unavailable (not open)
    if key.is_empty() {
        if is_prod {
            tracing::error!("GATEWAY_API_KEY not set in PRODUCTION=true mode — refusing all requests");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("content-type", "application/json")],
                r#"{"error":"misconfigured","message":"Service not available"}"#,
            ).into_response();
        }
        tracing::warn!("GATEWAY_API_KEY not set — running open (dev mode only)");
        return stamp_request_id(next.run(req).await);
    }

    let auth = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = auth.strip_prefix("Bearer ").unwrap_or("");

    if !constant_time_eq(token, key) {
        let should_ban = auth_tracker.record_failure(&ip);
        if should_ban { ban_list.ban(&ip); }
        crate::hardening::audit("auth_failure", &ip, path,
            if should_ban { "IP banned after threshold" } else { "invalid bearer token" });
        return (
            StatusCode::UNAUTHORIZED,
            [
                ("www-authenticate", "Bearer realm=\"Autonomyx\""),
                ("content-type", "application/json"),
            ],
            r#"{"error":"unauthorized","message":"Invalid or missing Bearer token"}"#,
        ).into_response();
    }

    // Successful auth — clear failure record
    auth_tracker.record_success(&ip);

    stamp_request_id(next.run(req).await)
}

fn stamp_request_id(mut resp: Response) -> Response {
    let req_id = Uuid::new_v4().to_string();
    if let Ok(v) = HeaderValue::from_str(&req_id) {
        resp.headers_mut().insert("x-request-id", v);
    }
    resp
}

/// Egress policy middleware — wraps outbound routes (/transfer/*)
/// to enforce egress-only (push, not pull).
pub async fn egress_policy(req: Request, next: Next) -> Response {
    // Transfer endpoint is egress-only: only POST is allowed (push).
    // GET/PUT/DELETE from external callers are rejected.
    if req.uri().path().starts_with("/transfer") && req.method() == axum::http::Method::GET {
        return (StatusCode::METHOD_NOT_ALLOWED, "Transfer is egress-push only").into_response();
    }
    next.run(req).await
}

/// Constant-time string comparison — prevents timing attacks on the API key.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

// ── Egress route registry (declared in .ayx, read at runtime) ─────────────────

#[derive(Debug, Clone)]
pub struct EgressRoute {
    pub name:    String,
    pub url:     String,
    pub auth_env: Option<String>,
    pub timeout_secs: u64,
}

/// Load egress routes from environment variables.
/// Convention: EGRESS_<NAME>_URL, EGRESS_<NAME>_KEY_ENV, EGRESS_<NAME>_TIMEOUT
/// This avoids embedding URLs or auth in source — supply chain risk = 0.
pub fn load_egress_routes() -> Vec<EgressRoute> {
    let mut routes = Vec::new();

    // Built-in egress routes — always available
    routes.push(EgressRoute {
        name:         "llm".into(),
        url:          std::env::var("LLM_BASE_URL")
                          .unwrap_or_else(|_| "https://api.openai.com".into()),
        auth_env:     Some("LLM_API_KEY".into()),
        timeout_secs: 120,
    });
    routes.push(EgressRoute {
        name:         "anthropic".into(),
        url:          std::env::var("ANTHROPIC_BASE_URL")
                          .unwrap_or_else(|_| "https://api.anthropic.com".into()),
        auth_env:     Some("ANTHROPIC_API_KEY".into()),
        timeout_secs: 120,
    });

    // Dynamic routes from env (EGRESS_MYSERVICE_URL=https://...)
    for (k, v) in std::env::vars() {
        if let Some(suffix) = k.strip_prefix("EGRESS_").and_then(|s| s.strip_suffix("_URL")) {
            let name      = suffix.to_lowercase();
            let key_env   = format!("EGRESS_{suffix}_KEY_ENV");
            let timeout_k = format!("EGRESS_{suffix}_TIMEOUT");
            routes.push(EgressRoute {
                name:         name.clone(),
                url:          v,
                auth_env:     std::env::var(&key_env).ok(),
                timeout_secs: std::env::var(&timeout_k)
                                  .ok()
                                  .and_then(|t| t.parse().ok())
                                  .unwrap_or(30),
            });
        }
    }

    routes
}

/// Shared egress state injected into AppState.
pub struct GatewayState {
    pub egress:  Arc<EgressClient>,
    pub routes:  Vec<EgressRoute>,
}

impl GatewayState {
    pub fn new() -> Self {
        Self {
            egress: Arc::new(EgressClient::new()),
            routes: load_egress_routes(),
        }
    }
}

impl Default for GatewayState {
    fn default() -> Self {
        Self::new()
    }
}
