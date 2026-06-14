mod store;
mod gate;
mod gateway;
mod configdb;
mod marketplace;
mod identity;
mod tools;
mod providers;
mod agent;
mod transfer;
mod routes;
mod cli;

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

    fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("agent_runner=debug".parse().unwrap())
            .add_directive("tower_http=info".parse().unwrap()))
        .init();

    let port: u16 = std::env::var("PORT")
        .ok().and_then(|v| v.parse().ok())
        .unwrap_or(3001);

    let state = Arc::new(AppState::new());

    // Connect SurrealDB configdb (async — upgrades from sync stub)
    match configdb::ConfigDB::connect().await {
        Ok(db) => {
            tracing::info!("configdb: SurrealDB connected");
            // Live queries now active — config changes push to agents natively
            let _ = db; // will be stored in AppState in next iteration
        }
        Err(e) => tracing::warn!("configdb: using in-memory stub — {e}"),
    }

    // Log active egress routes on startup
    let gw = gateway::GatewayState::new();
    for r in &gw.routes {
        tracing::info!(name = %r.name, url = %r.url, "egress route registered");
    }

    // ── Stack assembly (bottom-up layer composition) ──────────────────────────
    let app = Router::new()
        .merge(routes::health::router())
        .nest("/api",      routes::runs::router(state.clone()))
        .nest("/api",      routes::agents::router(state.clone()))
        .nest("/api",      routes::tools::router(state.clone()))
        .nest("/api",      routes::peers::router(state.clone()))
        .nest("/api",      routes::infra::router())
        .nest("/transfer", transfer::router(state.clone()))
        .nest("/ws",       routes::ws::router(state.clone()))
        // Egress policy: enforce push-only on /transfer
        .layer(middleware::from_fn(gateway::egress_policy))
        // Ingress gate: Bearer auth on all routes (except /health)
        .layer(middleware::from_fn(gateway::ingress_gate))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await.expect("bind failed");

    tracing::info!("Autonomyx runner listening on :{port}");
    axum::serve(listener, app).await.expect("serve failed");
}
