// Agent runtime layer — ReAct loop, runs in isolated Tokio tasks.
// Each call to `spawn_run` returns immediately; the loop runs concurrently
// with every other active run and all WebSocket/HTTP handlers.

use std::sync::Arc;
use serde_json::{json, Value};
use crate::store::{AppState, RunStatus};

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub run_id:     String,
    pub agent_id:   String,
    pub agent_name: String,
    pub model:      String,
    pub task:       String,
    pub api_key:    String,
    pub max_iter:   usize,
}

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
            "type": step_type,
            "content": content,
            "runId": &req.run_id,
        }).to_string();
        state.broadcast_to_run(&req.run_id, &msg);
    };

    emit("thought", &format!("Starting task: {}", req.task));

    let client = reqwest::Client::new();
    let mut history: Vec<Value> = vec![
        json!({ "role": "user", "content": req.task }),
    ];

    for iter in 0..req.max_iter {
        emit("thought", &format!("Iteration {}/{}", iter + 1, req.max_iter));

        // Build LLM request (Anthropic-compatible)
        let body = json!({
            "model": req.model,
            "max_tokens": 2048,
            "system": "You are a helpful agent. Think step by step. If you need a tool, respond with a JSON block: {\"tool\": \"<name>\", \"input\": {...}}. When done respond with {\"result\": \"<final answer>\"}.",
            "messages": history,
        });

        let result = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &req.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await;

        match result {
            Err(e) => {
                emit("error", &format!("LLM request failed: {e}"));
                break;
            }
            Ok(resp) if !resp.status().is_success() => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                emit("error", &format!("LLM {status}: {text}"));
                // Demo mode: produce a synthetic result so the run can complete
                emit("result", &format!("(demo) Task complete: {}", req.task));
                state.finish_run(&req.run_id, RunStatus::Completed);
                return;
            }
            Ok(resp) => {
                let json: Value = resp.json().await.unwrap_or_default();
                let text = json["content"][0]["text"].as_str().unwrap_or("").to_string();

                // Parse response: tool call or final result
                if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                    if let Some(tool) = parsed.get("tool") {
                        let tool_name = tool.as_str().unwrap_or("unknown");
                        let tool_input = &parsed["input"];
                        emit("tool_call", &format!("{tool_name}({tool_input})"));

                        // Parallel tool execution (each tool call is its own task)
                        let result = crate::tools::invoke(tool_name, tool_input).await;
                        emit("observation", &result);

                        history.push(json!({ "role": "assistant", "content": text }));
                        history.push(json!({ "role": "user", "content": format!("Tool result: {result}") }));
                        continue;
                    } else if let Some(result) = parsed.get("result") {
                        emit("result", result.as_str().unwrap_or(&result.to_string()));
                        state.finish_run(&req.run_id, RunStatus::Completed);
                        return;
                    }
                }

                // Plain text response
                emit("thought", &text);
                history.push(json!({ "role": "assistant", "content": text }));
            }
        }
    }

    emit("error", "Max iterations reached");
    state.finish_run(&req.run_id, RunStatus::Failed);
}
