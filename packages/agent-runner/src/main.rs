mod store;
mod gate;
mod tools;
mod agent;
mod transfer;
mod routes;
mod cli;

use axum::{middleware, Router};
use cli::{Cli, Commands};
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt, EnvFilter};
use std::sync::Arc;

pub use store::AppState;

#[tokio::main]
async fn main() {
    // Headless CLI mode — if subcommand given, run and exit
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

    // ── Stack assembly ────────────────────────────────────────────────────────
    // Layers compose bottom-up; each sub-router is independently testable.
    let app = Router::new()
        .merge(routes::health::router())
        .nest("/api",      routes::runs::router(state.clone()))
        .nest("/api",      routes::agents::router(state.clone()))
        .nest("/api",      routes::tools::router(state.clone()))
        .nest("/api",      routes::peers::router(state.clone()))
        .nest("/api",      routes::infra::router())
        .nest("/transfer", transfer::router(state.clone()))
        .nest("/ws",       routes::ws::router(state.clone()))
        // Gate: auth check on every request (no-op Phase 2, full JWT Phase 3)
        .layer(middleware::from_fn(gate::require_auth))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await.expect("bind failed");

    tracing::info!("agent-runner listening on :{port}");
    axum::serve(listener, app).await.expect("serve failed");
}
