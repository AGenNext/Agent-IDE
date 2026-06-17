// Surface hardening — defense-in-depth at the ingress boundary.
//
// Layers (applied in order, outermost first):
//   1. CatchPanic               — handler panics return 500, not server crash
//   2. Request timeout          — every request dies at 30s (configurable)
//   3. Body size limit          — 1MB default; 16KB for MCP/onboarding chat
//   4. Security headers         — HSTS, X-Frame-Options, X-Content-Type-Options, CSP
//   5. Rate limiter             — per-IP token bucket, expensive ops cost more
//   6. IP ban list              — blocks IPs after N consecutive auth failures
//   7. Auth (gateway.rs)        — Bearer token, constant-time, cached key, unique req-id
//   8. Egress policy (gateway.rs) — push-only on /transfer
//
// Every layer is independently configurable via env vars.
// Deny by default. Prove identity before opening any gate.
// Production precheck: call hardening::production_precheck() at startup.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    http::{HeaderValue, StatusCode},
};

// ── Security headers ──────────────────────────────────────────────────────────
// Injected on every response. Non-negotiable in production.

pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let headers  = resp.headers_mut();

    // Prevent MIME-type sniffing (XSS vector)
    headers.insert("x-content-type-options",
        HeaderValue::from_static("nosniff"));

    // Prevent clickjacking
    headers.insert("x-frame-options",
        HeaderValue::from_static("DENY"));

    // Reflect only origin, no path (prevents referrer leakage)
    headers.insert("referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"));

    // Restrict API access to same origin + explicit CORS allowlist
    // Content-Security-Policy: API only — no scripts, no inline content
    headers.insert("content-security-policy",
        HeaderValue::from_static(
            "default-src 'none'; frame-ancestors 'none'"
        ));

    // HSTS — force HTTPS for 1 year (only effective when behind TLS)
    // Set MAX_AGE_SECS=0 in env to disable (e.g. local dev without TLS)
    let max_age = std::env::var("HSTS_MAX_AGE_SECS")
        .ok().and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(31_536_000);
    if max_age > 0 {
        if let Ok(val) = HeaderValue::from_str(
            &format!("max-age={max_age}; includeSubDomains; preload")
        ) {
            headers.insert("strict-transport-security", val);
        }
    }

    // Tag every response with platform identifier (non-sensitive)
    headers.insert("x-powered-by",
        HeaderValue::from_static("Autonomyx — openautonomyx.com"));

    // Remove server fingerprint (don't advertise what we're running)
    headers.remove("server");

    resp
}

// ── Rate limiter — per-IP token bucket ───────────────────────────────────────
// Simple in-memory rate limiter. For multi-node: replace with Redis/SurrealDB.
// Default: 60 requests per 60-second window per IP.

#[derive(Debug)]
struct Bucket {
    tokens:     u32,
    last_refill: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    buckets:        Arc<RwLock<HashMap<String, Bucket>>>,
    max_tokens:     u32,
    refill_secs:    u64,
    // Higher limits for specific paths (e.g. /health probes)
}

impl RateLimiter {
    pub fn new() -> Self {
        let max_tokens  = std::env::var("RATE_LIMIT_RPM")
            .ok().and_then(|v| v.parse().ok())
            .unwrap_or(60u32);
        let refill_secs = std::env::var("RATE_LIMIT_WINDOW_SECS")
            .ok().and_then(|v| v.parse().ok())
            .unwrap_or(60u64);
        Self {
            buckets:     Arc::new(RwLock::new(HashMap::new())),
            max_tokens,
            refill_secs,
        }
    }

    /// Returns true if request is allowed; false if rate-limited.
    pub fn check(&self, ip: &str, cost: u32) -> bool {
        let mut map   = self.buckets.write().unwrap();
        let now       = Instant::now();
        let window    = Duration::from_secs(self.refill_secs);
        let max       = self.max_tokens;

        let bucket = map.entry(ip.to_string()).or_insert(Bucket {
            tokens:      max,
            last_refill: now,
        });

        // Refill tokens if window has elapsed
        if now.duration_since(bucket.last_refill) >= window {
            bucket.tokens      = max;
            bucket.last_refill = now;
        }

        if bucket.tokens >= cost {
            bucket.tokens -= cost;
            true
        } else {
            false
        }
    }

    /// Prune stale buckets to prevent unbounded memory growth.
    pub fn prune(&self) {
        let window = Duration::from_secs(self.refill_secs * 2);
        let now    = Instant::now();
        self.buckets.write().unwrap()
            .retain(|_, b| now.duration_since(b.last_refill) < window);
    }
}

// Path costs — expensive operations burn more rate-limit tokens.
fn path_cost(path: &str) -> u32 {
    if path.starts_with("/api/onboarding") && path.ends_with("/chat") {
        10  // LLM call — expensive
    } else if path.starts_with("/api/runs") && path.ends_with('/') || path == "/api/runs" {
        8   // agent run dispatch — may trigger LLM
    } else if path == "/mcp" {
        5   // tool dispatch
    } else if path.starts_with("/api/lifecycle") {
        3   // gate transition
    } else if path.starts_with("/api/providers/certify") {
        3   // provider probe
    } else if path == "/health" || path == "/ready" {
        0   // free — always allow probes
    } else {
        1
    }
}

pub async fn rate_limit(
    req: Request,
    next: Next,
    limiter: Arc<RateLimiter>,
) -> Response {
    // Skip rate limiting on /health — must always respond to k8s probes
    if req.uri().path() == "/health" {
        return next.run(req).await;
    }

    // Extract client IP — prefer X-Forwarded-For (behind load balancer)
    let ip = req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let cost = path_cost(req.uri().path());

    if !limiter.check(&ip, cost) {
        tracing::warn!(
            ip   = %ip,
            path = %req.uri().path(),
            "rate limit exceeded"
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [
                ("retry-after", "60"),
                ("x-ratelimit-limit", "60"),
                ("content-type", "application/json"),
            ],
            r#"{"error":"rate_limit_exceeded","retry_after":60,"message":"Too many requests. Retry after 60 seconds."}"#,
        ).into_response();
    }

    next.run(req).await
}

// ── Request guard — body size + suspicious patterns ───────────────────────────
// Enforce per-route body size limits before passing to handlers.

pub async fn request_guard(req: Request<Body>, next: Next) -> Response {
    let path  = req.uri().path().to_string();
    let limit = body_size_limit(&path);

    // axum's DefaultBodyLimit handles this cleanly if wired at the router level,
    // but we also log rejections explicitly here.
    // The actual enforcement is done by tower's DefaultBodyLimit layer in main.rs.
    // This middleware adds logging and structured error responses.

    // Reject suspicious path patterns (path traversal, null bytes)
    if path.contains("..") || path.contains('\0') || path.len() > 512 {
        tracing::warn!(path = %path, "request_guard: suspicious path rejected");
        return (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            r#"{"error":"bad_request","message":"Invalid request path"}"#,
        ).into_response();
    }

    // Add size limit hint to response headers for clients
    let mut resp = next.run(req).await;
    if let Ok(v) = HeaderValue::from_str(&limit.to_string()) {
        resp.headers_mut().insert("x-max-body-bytes", v);
    }
    resp
}

fn body_size_limit(path: &str) -> usize {
    if path.ends_with("/chat") || path == "/mcp" {
        16 * 1024          // 16KB — chat messages should be concise
    } else if path.starts_with("/transfer") {
        4 * 1024 * 1024    // 4MB — artifact transfers
    } else {
        1 * 1024 * 1024    // 1MB default
    }
}

// ── Auth failure tracker ──────────────────────────────────────────────────────
// Count consecutive auth failures per IP. Ban after threshold.
// In-memory; resets on restart. Production: persist to SurrealDB.

#[derive(Clone, Default)]
pub struct AuthFailureTracker {
    failures: Arc<RwLock<HashMap<String, (u32, Instant)>>>,
}

impl AuthFailureTracker {
    pub fn new() -> Self { Self::default() }

    /// Record a failure. Returns true if IP should be temporarily blocked.
    pub fn record_failure(&self, ip: &str) -> bool {
        let threshold = std::env::var("AUTH_FAILURE_THRESHOLD")
            .ok().and_then(|v| v.parse().ok())
            .unwrap_or(10u32);
        let ban_secs  = std::env::var("AUTH_BAN_SECS")
            .ok().and_then(|v| v.parse().ok())
            .unwrap_or(300u64);

        let mut map = self.failures.write().unwrap();
        let now     = Instant::now();
        let entry   = map.entry(ip.to_string()).or_insert((0, now));

        // Reset if ban period has elapsed
        if now.duration_since(entry.1) > Duration::from_secs(ban_secs) {
            *entry = (0, now);
        }

        entry.0 += 1;
        entry.0 >= threshold
    }

    pub fn record_success(&self, ip: &str) {
        self.failures.write().unwrap().remove(ip);
    }
}

// ── IP ban list — blocks IPs after N consecutive auth failures ────────────────
// Separate from RateLimiter; survives rate-limit resets.
// Checked before rate limit so banned IPs don't consume rate limit slots.

#[derive(Clone, Default)]
pub struct IpBanList {
    banned: Arc<RwLock<HashMap<String, Instant>>>,
}

impl IpBanList {
    pub fn new() -> Self { Self::default() }

    pub fn is_banned(&self, ip: &str) -> bool {
        let ban_secs = std::env::var("AUTH_BAN_SECS")
            .ok().and_then(|v| v.parse().ok())
            .unwrap_or(300u64);
        let now = Instant::now();
        if let Some(&banned_at) = self.banned.read().unwrap().get(ip) {
            now.duration_since(banned_at) < Duration::from_secs(ban_secs)
        } else {
            false
        }
    }

    pub fn ban(&self, ip: &str) {
        self.banned.write().unwrap().insert(ip.to_string(), Instant::now());
        tracing::warn!(ip = %ip, "IP banned after repeated auth failures");
    }

    pub fn unban(&self, ip: &str) {
        self.banned.write().unwrap().remove(ip);
    }

    pub fn prune(&self) {
        let ban_secs = std::env::var("AUTH_BAN_SECS")
            .ok().and_then(|v| v.parse().ok())
            .unwrap_or(300u64);
        let now = Instant::now();
        self.banned.write().unwrap()
            .retain(|_, t| now.duration_since(*t) < Duration::from_secs(ban_secs * 2));
    }
}

// ── Audit log — structured record of every security-relevant event ────────────

pub fn audit(event: &str, ip: &str, path: &str, detail: &str) {
    tracing::warn!(
        audit   = true,
        event   = %event,
        ip      = %ip,
        path    = %path,
        detail  = %detail,
        "security audit"
    );
}

// ── Production precheck — fail fast on unsafe configuration ──────────────────
// Call at startup before binding the listener.
// Returns a list of critical issues; caller should abort if non-empty.

pub fn production_precheck() -> Vec<String> {
    let mut issues = Vec::new();

    let is_prod = std::env::var("PRODUCTION")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if !is_prod {
        return issues; // dev mode: skip
    }

    // GATEWAY_API_KEY must be set and long enough
    match std::env::var("GATEWAY_API_KEY") {
        Err(_) => issues.push("GATEWAY_API_KEY is not set — server will run open (FATAL in PRODUCTION=true)".into()),
        Ok(k) if k.len() < 32 => issues.push(format!(
            "GATEWAY_API_KEY is only {} chars — minimum 32 required for production", k.len()
        )),
        _ => {}
    }

    // HSTS must be enabled (max_age > 0)
    if let Ok(v) = std::env::var("HSTS_MAX_AGE_SECS") {
        if v == "0" {
            issues.push("HSTS_MAX_AGE_SECS=0 disables HSTS — not safe for production".into());
        }
    }

    // CORS must not be wildcard in production
    let cors = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();
    if cors.is_empty() {
        issues.push("CORS_ALLOWED_ORIGINS not set — defaulting to wildcard '*' is not safe in production".into());
    }

    // Body limit must be sane (not too large)
    if let Ok(v) = std::env::var("BODY_LIMIT_BYTES") {
        if let Ok(n) = v.parse::<usize>() {
            if n > 64 * 1024 * 1024 {
                issues.push(format!("BODY_LIMIT_BYTES={n} exceeds 64MB — risk of OOM under attack"));
            }
        }
    }

    // Request timeout must be set
    if let Ok(v) = std::env::var("REQUEST_TIMEOUT_SECS") {
        if let Ok(n) = v.parse::<u64>() {
            if n > 120 {
                issues.push(format!("REQUEST_TIMEOUT_SECS={n} is very high — slow requests will hold threads"));
            }
        }
    }

    // Platform identity should be set for signed accountability records
    if std::env::var("AUTONOMYX_IDENTITY_KEY").is_err() {
        issues.push("AUTONOMYX_IDENTITY_KEY not set — accountability records will be unsigned".into());
    }

    issues
}

// ── CORS policy ───────────────────────────────────────────────────────────────
// Production: restrict to known origins via CORS_ALLOWED_ORIGINS env var.
// Dev mode: allow all (CORS_ALLOWED_ORIGINS not set).

pub fn cors_origins() -> Vec<String> {
    std::env::var("CORS_ALLOWED_ORIGINS")
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|_| vec![])
}

// ── Circuit breaker — build for failure ───────────────────────────────────────
// When downstream dependencies fail (LLM providers, SurrealDB, peer nodes),
// the gate stays open for idempotent reads, closes for writes that require them.
// Partial failure = graceful degradation, not total outage.
//
// States:  Closed (healthy) → Open (failing) → HalfOpen (probing) → Closed
// "Build for failure" — every external call assumes it will fail.

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState { Closed, Open, HalfOpen }

pub struct CircuitBreaker {
    state:          std::sync::Mutex<CircuitState>,
    failures:       std::sync::atomic::AtomicU32,
    last_failure:   std::sync::Mutex<Option<Instant>>,
    threshold:      u32,
    reset_secs:     u64,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, reset_secs: u64) -> Self {
        Self {
            state:        std::sync::Mutex::new(CircuitState::Closed),
            failures:     std::sync::atomic::AtomicU32::new(0),
            last_failure: std::sync::Mutex::new(None),
            threshold,
            reset_secs,
        }
    }

    pub fn is_open(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        match *state {
            CircuitState::Closed  => false,
            CircuitState::Open    => {
                // Check if reset window has passed → transition to HalfOpen
                let last = self.last_failure.lock().unwrap();
                if last.map(|t| t.elapsed().as_secs() > self.reset_secs).unwrap_or(false) {
                    *state = CircuitState::HalfOpen;
                    false
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => false,  // allow one probe through
        }
    }

    pub fn record_success(&self) {
        self.failures.store(0, std::sync::atomic::Ordering::Relaxed);
        *self.state.lock().unwrap() = CircuitState::Closed;
    }

    pub fn record_failure(&self) {
        let n = self.failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        *self.last_failure.lock().unwrap() = Some(Instant::now());
        if n >= self.threshold {
            *self.state.lock().unwrap() = CircuitState::Open;
            tracing::warn!(failures = n, "circuit breaker opened — downstream dependency failing");
        }
    }
}

// Minimal attack surface checklist — logged at startup.
pub fn log_surface() {
    tracing::info!(
        exposed = "health, api/*, ws/*, mcp, transfer, support",
        auth    = "Bearer token on all routes except /health",
        rate    = "60 req/min per IP default (RATE_LIMIT_RPM env)",
        body    = "4MB hard cap (BODY_LIMIT_BYTES env)",
        timeout = "30s per request (REQUEST_TIMEOUT_SECS env)",
        headers = "HSTS, X-Frame-Options, X-Content-Type-Options, CSP",
        egress  = "push-only /transfer, typed EgressClient only",
        crypto  = "Ed25519 signatures, constant-time Bearer comparison",
        "surface hardened"
    );
}
