// Autonomyx Platform — Production Binary
//
// Single binary. All gates. All fabric. All federation. MCP server.
// Runs on any OS (Linux/macOS/Windows), any cloud, any infra.
// Hardened surface: rate limit, body limit, security headers, auth failure tracking.
#![recursion_limit = "512"]
// Deny by default. Prove identity before opening any gate.
// Freedom, not free. openautonomyx.com

mod store;
mod gate;
mod gateway;
mod configdb;
mod marketplace;
mod identity;
mod lifecycle;
mod fabric;
mod bom;
mod core;
mod federation;
mod usage;
mod tools;
mod providers;
mod agent;
mod transfer;
mod routes;
mod cli;
mod cloud;
mod onboarding;
mod mcp;
mod hardening;
mod contract;
mod controller;
mod blockchain;
mod compute;
mod storage;
mod govgraph;
mod computekube;
mod goals;
mod dashboard;
mod plugin;
mod search;
mod loop_coordinator;
mod optin;
mod optin_middleware;
mod authmatic;
mod arithmetic;
mod multiserver;
mod provider_cert;
mod reconciler;
mod megaverse;
mod teams;
mod cncf;

use axum::{middleware, Router};
use axum::extract::DefaultBodyLimit;
use cli::Cli;
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::timeout::TimeoutLayer;
use tracing_subscriber::{fmt, EnvFilter};
use std::sync::Arc;
use std::time::Duration;

pub use store::AppState;

// ── Intelligent port assignment ───────────────────────────────────────────────
// Scans a port range and returns the first port not already in use.
// Enables multiple platform instances on the same host without config.
fn find_free_port(start: u16, end: u16) -> Option<u16> {
    for port in start..=end {
        if std::net::TcpListener::bind(("0.0.0.0", port)).is_ok() {
            return Some(port);
        }
    }
    None
}

#[tokio::main]
async fn main() {
    let cli_args = Cli::parse();
    if cli_args.command.is_some() {
        cli::run_cli(cli_args).await.expect("CLI error");
        return;
    }

    // ── Logging ───────────────────────────────────────────────────────────────
    fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("agent_runner=info".parse().unwrap())
            .add_directive("tower_http=warn".parse().unwrap()))
        .init();

    // ── Intelligent port assignment ───────────────────────────────────────────
    // Priority: PORT env var → find first free port in preferred range → 3001
    let port: u16 = std::env::var("PORT")
        .ok().and_then(|v| v.parse().ok())
        .unwrap_or_else(|| find_free_port(3001, 3099).unwrap_or(3001));

    // ── Production startup banner ─────────────────────────────────────────────
    let cloud_ctx = cloud::CloudContext::detect();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        port    = port,
        cloud   = ?cloud_ctx.provider,
        region  = ?cloud_ctx.region,
        "Autonomyx platform starting"
    );

    // ── Hardening ─────────────────────────────────────────────────────────────
    let rate_limiter = Arc::new(hardening::RateLimiter::new());

    // Prune stale rate-limit buckets every 5 minutes (prevent memory growth)
    {
        let rl = rate_limiter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop { interval.tick().await; rl.prune(); }
        });
    }

    // Request timeout — kill slow requests before they exhaust threads
    let request_timeout_secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
        .ok().and_then(|v| v.parse().ok())
        .unwrap_or(30);

    // Body size limit — 4MB hard cap, path-specific limits in request_guard
    let body_limit_bytes: usize = std::env::var("BODY_LIMIT_BYTES")
        .ok().and_then(|v| v.parse().ok())
        .unwrap_or(4 * 1024 * 1024);

    // ── State ─────────────────────────────────────────────────────────────────
    let state = Arc::new(AppState::new());

    // ── ConfigDB (SurrealDB) — async upgrade from in-memory stub ─────────────
    match configdb::ConfigDB::connect().await {
        Ok(db) => {
            tracing::info!("configdb: SurrealDB connected — live queries active");
            state.fabric.wire_surreal(std::sync::Arc::new(db));
        }
        Err(e) => tracing::warn!(error = %e, "configdb: in-memory stub (SurrealDB not reachable)"),
    }

    // ── Platform identity ─────────────────────────────────────────────────────
    // Load or generate the platform's own DID — it signs all accountability records.
    if let Some(identity) = identity::AgentIdentity::from_env() {
        tracing::info!(did = %identity.did, "platform identity loaded from AUTONOMYX_IDENTITY_KEY");
    } else {
        tracing::warn!("AUTONOMYX_IDENTITY_KEY not set — generating ephemeral identity (not for production)");
    }

    // ── Egress routes ─────────────────────────────────────────────────────────
    let gw = gateway::GatewayState::new();
    for r in &gw.routes {
        tracing::info!(name = %r.name, url = %r.url, "egress route registered");
    }

    // ── Stack assembly (bottom-up layer composition) ──────────────────────────
    // Every route is:
    //   1. Auth-gated (Bearer token via ingress_gate — except /health)
    //   2. Egress-controlled (push-only on /transfer via egress_policy)
    //   3. Traced (OTel spans via TraceLayer)
    //   4. CORS-enabled (any origin — agents come from anywhere)
    let app = Router::new()
        .merge(routes::health::router())
        // Core platform
        .nest("/api", routes::agents::router(state.clone()))
        .nest("/api", routes::apps::router(state.clone()))
        .nest("/api", routes::runs::router(state.clone()))
        // Lifecycle + fabric
        .nest("/api", routes::lifecycle::router(state.clone()))
        .nest("/api", routes::fabric::router(state.clone()))
        // Identity + federation
        .nest("/api", routes::peers::router(state.clone()))
        // Observability + tools
        .nest("/api", routes::tools::router(state.clone()))
        .nest("/api", routes::infra::router())
        // Usage + billing
        .nest("/api", routes::usage::router(state.clone()))
        // Platform identity + world model
        .nest("/api", routes::platform::router(state.clone()))
        // Onboarding — chat-based configuration
        .nest("/api", routes::onboarding::router(state.clone()))
        // Support + health
        .nest("/", routes::support::router(state.clone()))
        // Peer transfer (egress-push only)
        .nest("/transfer", transfer::router(state.clone()))
        // WebSocket: run stream, fabric stream, unified stream
        .nest("/ws", routes::ws::router(state.clone()))
        // MCP server — platform as AI tool
        .nest("/", routes::mcp::router(state.clone()))
        // AIP — Agent Internet Protocol (agent-to-agent, DID + Ed25519)
        .nest("/api", routes::aip::router(state.clone()))
        // Blockchain — on-chain DID, accountability, usage settlement, agent NFTs
        .nest("/api", routes::blockchain::router(state.clone()))
        // Storage — distributed, policy-driven, milestone-bound, project lifecycle
        .nest("/api", routes::storage::router(state.clone()))
        // Governance graph — compute core wired to governance at every edge
        .nest("/api", routes::govgraph::router(state.clone()))
        // ComputeKube — governed k8s job execution for graph nodes
        .nest("/api", routes::computekube::router(state.clone()))
        // Goals — purpose-driven agents; alignment → activation → impact → loop
        .nest("/api", routes::goals::router(state.clone()))
        // Dashboards — custom views over the full platform world model
        .nest("/api", routes::dashboard::router(state.clone()))
        // Plugins — everything extendable; built-in + custom; contribute nodes/sources
        .nest("/api", routes::plugin::router(state.clone()))
        // Universal open search — one query across the full world model
        .nest("/api", routes::search::router(state.clone()))
        // Opt-in — extend the platform or align intent with 7 values
        .nest("/api", routes::optin::router(state.clone()))
        // OpenAPI spec + route discovery — self-documenting platform
        .merge(routes::openapi::router())
        // Landing page — GET / serves the full route map as HTML
        .merge(routes::landing::router())
        // Auth-matic — JIT keys, enrollment, rotation, peer credentials
        .nest("/api", routes::authmatic::router(state.clone()))
        // Arithmetic — expression eval, platform stats, named formulas
        .nest("/api", routes::arithmetic::router(state.clone()))
        // Feedback gate — closes the loop, every run produces signal
        .nest("/api", routes::feedback::router(state.clone()))
        // Provider certification — certified providers only
        .nest("/api", routes::providers::router(state.clone()))
        // Megaverse — unified world model
        .nest("/api", routes::megaverse::router(state.clone()))
        // Teams — institutional agent teams: universal cross-sector collaboration
        .nest("/api", routes::teams::router(state.clone()))
        // CNCF alignment — cloud-native conformance map + gap analysis
        .nest("/api", routes::cncf::router())
        // ── Middleware stack (applied outer-in) ──────────────────────────────
        // Layer order: last added = outermost (first to run on request, last on response)
        .layer(middleware::from_fn({
            let s = state.clone();
            move |req, next| contract::contract_layer(req, next, s.clone())
        }))                                                        // 6. contract: oath + governance + fabric
        .layer(middleware::from_fn(gateway::egress_policy))       // 5. egress: push-only /transfer
        .layer(middleware::from_fn(gateway::ingress_gate))        // 4. auth: Bearer token, constant-time
        .layer(middleware::from_fn({
            let rl = rate_limiter.clone();
            move |req, next| {
                let rl = rl.clone();
                hardening::rate_limit(req, next, rl)
            }
        }))                                                        // 3. rate: per-IP token bucket
        .layer(middleware::from_fn(hardening::request_guard))     // 2. guard: path + size check
        .layer(middleware::from_fn(optin_middleware::optin_gap_filler)) // 0. gap: unknown routes → optin
        .layer(middleware::from_fn(hardening::security_headers))  // 1. headers: HSTS, CSP, X-Frame
        .layer(DefaultBodyLimit::max(body_limit_bytes))           //    body: 4MB hard cap
        .layer(TimeoutLayer::new(Duration::from_secs(request_timeout_secs))) // timeout: 30s
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await.expect("bind failed");

    // ── Controller — CRD reconciliation loop ─────────────────────────────────
    // Watches AutonomyxAgent + AutonomyxApplication CRDs and drives gates.
    // End to end: declaration → build → sign → push → deploy → run → observe.
    // ── Plugin governance nodes — wire enabled plugin nodes into govgraph ────
    {
        use crate::govgraph::{GovernanceNode, NodeKind, NodePolicy};
        for pn in state.plugins.all_nodes() {
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
                created_at:   chrono::Utc::now(),
                updated_at:   chrono::Utc::now(),
            };
            state.govgraph.add_node(node);
        }
    }

    controller::start(state.clone());
    loop_coordinator::start(state.clone());
    multiserver::start(state.clone());
    reconciler::start(state.clone());
    megaverse::start_live(state.clone()); // megaverse updates instantly via fabric

    // Immediate full rebase on startup — sync all surfaces to ground truth
    reconciler::rebase_all(&state);

    // Log hardened attack surface at boot
    hardening::log_surface();

    tracing::info!(
        port    = port,
        routes  = "health, agents, apps, runs, lifecycle, fabric, peers, tools, infra, usage, platform, onboarding, support, transfer, ws, mcp",
        mcp     = "POST /mcp (JSON-RPC 2.0)",
        stream  = "WS /ws/stream (unified), /ws/fabric, /ws/:run_id",
        fabric  = "middleware between all gates — fills every gap, no polling",
        "Autonomyx platform ready — openautonomyx.com"
    );

    axum::serve(listener, app).await.expect("serve failed");
}
