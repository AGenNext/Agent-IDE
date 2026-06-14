// Autonomyx Platform — Production Binary
//
// Single binary. All gates. All fabric. All federation. MCP server.
// Runs on any OS (Linux/macOS/Windows), any cloud, any infra.
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

use axum::{middleware, Router};
use cli::Cli;
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt, EnvFilter};
use std::sync::Arc;

pub use store::AppState;

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

    let port: u16 = std::env::var("PORT")
        .ok().and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    // ── Production startup banner ─────────────────────────────────────────────
    let cloud_ctx = cloud::CloudContext::detect();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        port    = port,
        cloud   = ?cloud_ctx.provider,
        region  = ?cloud_ctx.region,
        "Autonomyx platform starting"
    );

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
        .nest("/api", routes::platform::router())
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
        // ── Middleware stack (applied outer-in) ──────────────────────────────
        .layer(middleware::from_fn(gateway::egress_policy))   // egress control
        .layer(middleware::from_fn(gateway::ingress_gate))    // Bearer auth
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await.expect("bind failed");

    tracing::info!(
        port    = port,
        routes  = "health, agents, apps, runs, lifecycle, fabric, peers, tools, infra, usage, platform, onboarding, support, transfer, ws, mcp",
        mcp     = "POST /mcp (JSON-RPC 2.0)",
        stream  = "WS /ws/stream (unified), /ws/fabric, /ws/:run_id",
        "Autonomyx platform ready — openautonomyx.com"
    );

    axum::serve(listener, app).await.expect("serve failed");
}
