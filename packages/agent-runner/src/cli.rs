// Headless CLI mode — run an agent task without the IDE or HTTP server.
// Output is JSON to stdout; artifacts (PNG) written to --output path.
//
// Usage:
//   agent-runner run \
//     --task "Summarize the top 3 AI papers this week" \
//     --model claude-sonnet-4-6 \
//     --api-key $ANTHROPIC_API_KEY \
//     --output ./trace.json

use clap::{Parser, Subcommand};
use std::sync::Arc;
use crate::store::{AppState, RunStatus};
use crate::agent;

#[derive(Parser)]
#[command(name = "agent-runner", about = "Agent IDE headless runtime")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run an agent task headlessly — outputs JSON trace to stdout
    Run {
        #[arg(long)] task:    String,
        #[arg(long, default_value = "claude-sonnet-4-6")] model: String,
        #[arg(long)] api_key: Option<String>,
        #[arg(long, default_value = "10")] max_iter: usize,
        #[arg(long)] output:  Option<String>,
    },
    /// Health check — prints JSON status and exits
    Health,
    /// List built-in tools
    Tools,
}

pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Some(Commands::Run { task, model, api_key, max_iter, output }) => {
            let state = Arc::new(AppState::new());
            let key   = api_key
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .unwrap_or_default();

            let run = state.create_run("cli", "CLI Agent", &model, &task);
            let run_id = run.run_id.clone();

            eprintln!("[agent-runner] starting headless run {run_id}");
            eprintln!("[agent-runner] task: {task}");
            eprintln!("[agent-runner] model: {model}");

            // Run the agent loop synchronously (await the task)
            let req = agent::RunRequest {
                run_id: run_id.clone(),
                agent_id: "cli".into(),
                agent_name: "CLI Agent".into(),
                model, task, api_key: key, max_iter,
            };

            // Spawn + await so we can collect the finished run
            let state2 = state.clone();
            let handle = tokio::spawn(async move {
                agent::spawn_run(state2, req);
            });
            handle.await?;

            // Wait for run to finish (poll up to 5 minutes)
            for _ in 0..300 {
                if let Some(r) = state.get_run(&run_id) {
                    if r.status != RunStatus::Running {
                        break;
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }

            let finished = state.get_run(&run_id).unwrap();
            let json = serde_json::to_string_pretty(&finished)?;

            if let Some(path) = output {
                std::fs::write(&path, &json)?;
                eprintln!("[agent-runner] trace written to {path}");
            } else {
                println!("{json}");
            }
        }

        Some(Commands::Health) => {
            println!("{}", serde_json::json!({
                "status":  "ok",
                "runtime": "rust",
                "phase":   2,
                "version": env!("CARGO_PKG_VERSION"),
            }));
        }

        Some(Commands::Tools) => {
            println!("{}", serde_json::json!([
                { "id": "http_client", "name": "HTTP Client",  "category": "api"  },
                { "id": "web_search",  "name": "Web Search",   "category": "web"  },
                { "id": "shell",       "name": "Shell",          "category": "code" },
            ]));
        }

        None => {
            // No subcommand → start HTTP server (handled in main.rs)
        }
    }
    Ok(())
}
