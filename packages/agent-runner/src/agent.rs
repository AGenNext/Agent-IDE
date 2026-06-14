// Agent runtime — ReAct loop, fully provider-independent.
// LLM backend is selected at call time from model string + env vars.
// Each run is an isolated Tokio task; state is shared via AppState.

use std::sync::Arc;
use serde_json::{json, Value};
use crate::store::{AppState, RunStatus};
use crate::providers;

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub run_id:     String,
    pub agent_id:   String,
    pub agent_name: String,
    pub model:      String,
    pub task:       String,
    pub api_key:    String,   // optional — env fallback chain applied in providers::resolve_key
    pub max_iter:   usize,
}

const SYSTEM_PROMPT: &str = "\
You are a helpful Autonomyx agent. Think step by step.\
 If you need a tool, respond with exactly one JSON object: \
{\"tool\": \"<name>\", \"input\": {...}}.\
 When you have the final answer respond with: {\"result\": \"<answer>\"}.\
 Do not mix prose with JSON in the same response.";

/// Spawn the ReAct loop as a Tokio task — non-blocking, fully parallel.
pub fn spawn_run(state: Arc<AppState>, req: RunRequest) {
    tokio::spawn(async move {
        run_loop(state, req).await;
    });
}

async fn run_loop(state: Arc<AppState>, req: RunRequest) {
    let emit = |step_type: &str, content: &str| {
        state.add_run_step(&req.run_id, step_type, content);
        let msg = json!({
            "type":    step_type,
            "content": content,
            "runId":   &req.run_id,
        }).to_string();
        state.broadcast_to_run(&req.run_id, &msg);
    };

    emit("thought", &format!("Starting task: {}", req.task));
    tracing::debug!(run_id = %req.run_id, model = %req.model, "run_loop started");

    let client = reqwest::Client::new();
    let mut history: Vec<Value> = vec![
        json!({ "role": "user", "content": req.task }),
    ];

    for iter in 0..req.max_iter {
        emit("thought", &format!("Iteration {}/{}", iter + 1, req.max_iter));

        match providers::complete(
            &client,
            &req.model,
            &req.api_key,
            SYSTEM_PROMPT,
            &history,
            2048,
        ).await {
            Err(e) => {
                let msg = format!("LLM error: {e}");
                tracing::warn!(%msg);
                emit("error", &msg);

                // Demo mode when no key / no endpoint configured
                emit("result", &format!("(demo) Task noted: {}", req.task));
                state.finish_run(&req.run_id, RunStatus::Completed);
                return;
            }

            Ok(llm) => {
                let text = llm.text;

                // Try to parse as structured JSON (tool call or final result)
                if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                    if let Some(tool) = parsed.get("tool") {
                        let tool_name  = tool.as_str().unwrap_or("unknown");
                        let tool_input = &parsed["input"];
                        emit("tool_call", &format!("{tool_name}({tool_input})"));

                        let obs = crate::tools::invoke(tool_name, tool_input).await;
                        emit("observation", &obs);

                        history.push(json!({ "role": "assistant", "content": &text }));
                        history.push(json!({ "role": "user",      "content": format!("Tool result: {obs}") }));
                        continue;
                    }

                    if let Some(result) = parsed.get("result") {
                        let answer = result.as_str().unwrap_or(&result.to_string()).to_string();
                        emit("result", &answer);
                        state.finish_run(&req.run_id, RunStatus::Completed);
                        return;
                    }
                }

                // Plain-text response — keep as context and loop
                emit("thought", &text);
                history.push(json!({ "role": "assistant", "content": &text }));

                // If model naturally stopped and gave plain text, treat as done
                if llm.stopped && iter > 0 {
                    emit("result", &text);
                    state.finish_run(&req.run_id, RunStatus::Completed);
                    return;
                }
            }
        }
    }

    emit("error", "Max iterations reached without a final result");
    state.finish_run(&req.run_id, RunStatus::Failed);
}
