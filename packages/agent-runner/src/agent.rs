// Agent runtime — ReAct loop, fully provider-independent.
// LLM backend is selected at call time from model string + env vars.
// Each run is an isolated Tokio task; state is shared via AppState.
//
// Model modes:
//   "eq:<expr>"   — equation agent: evaluates an arithmetic expression; no LLM call.
//                   Scales to 10,000+ concurrent agents at near-zero cost.
//   "rule:<json>" — rule agent: evaluates a JSON rule tree; no LLM call.
//   anything else — LLM agent: ReAct loop via configured provider.

use std::sync::Arc;
use serde_json::{json, Value};
use crate::store::{AppState, RunStatus};
use crate::providers;
use crate::provider_cert;

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

/// Spawn the run as a Tokio task — non-blocking, fully parallel.
/// Certified providers only: the cert gate runs before any work is dispatched.
pub fn spawn_run(state: Arc<AppState>, req: RunRequest) {
    tokio::spawn(async move {
        // Equation/rule agents are self-contained — no external provider to certify.
        if req.model.starts_with("eq:") || req.model == "equation" {
            equation_run(state, req).await;
            return;
        }
        if req.model.starts_with("rule:") {
            rule_run(state, req).await;
            return;
        }

        // ── Provider certification gate ───────────────────────────────────────
        // Certified providers only. Run is rejected immediately if cert fails.
        let cert = provider_cert::certify(&req.model, &state);
        if !cert.is_ok() {
            let reason = cert.reject_reason.as_deref().unwrap_or("certification failed");
            state.add_run_step(&req.run_id, "cert_fail",
                &format!("provider not certified — {reason}"));
            let msg = json!({
                "type":    "cert_fail",
                "content": reason,
                "cert":    serde_json::to_value(&cert).unwrap_or_default(),
                "runId":   &req.run_id,
            }).to_string();
            state.broadcast_to_run(&req.run_id, &msg);
            state.finish_run(&req.run_id, RunStatus::Failed);
            return;
        }

        // Attach cert metadata to first step so the run record is traceable
        state.add_run_step(&req.run_id, "cert_ok",
            &format!("provider certified: {} (trust={:.2}, cert={})",
                cert.provider_id, cert.trust_score, cert.cert_id));

        run_loop(state, req).await;
    });
}

// ── Equation agent — no LLM, scales to 10k+ concurrent ──────────────────────
//
// model = "eq:<expr>"      e.g. "eq:runs.completed / runs.total * 100"
// model = "equation"       + task contains the expression
//
// Evaluates arithmetic using platform_vars (25+ live platform metrics).
// Zero cost. Microsecond latency. No API key required.

async fn equation_run(state: Arc<AppState>, req: RunRequest) {
    let emit = |step_type: &str, content: &str| {
        state.add_run_step(&req.run_id, step_type, content);
        let msg = json!({ "type": step_type, "content": content, "runId": &req.run_id }).to_string();
        state.broadcast_to_run(&req.run_id, &msg);
    };

    let expr = if req.model.starts_with("eq:") {
        req.model["eq:".len()..].to_string()
    } else {
        req.task.clone()
    };

    emit("thought", &format!("equation agent: eval `{expr}`"));

    let vars = crate::arithmetic::platform_vars(&state);
    match crate::arithmetic::eval_expr(&expr, &vars) {
        Ok(result) => {
            emit("result", &format!("{}", result));
            state.finish_run(&req.run_id, RunStatus::Completed);
        }
        Err(e) => {
            emit("error", &format!("equation error: {e}"));
            state.finish_run(&req.run_id, RunStatus::Failed);
        }
    }
}

// ── Rule agent — JSON decision tree, no LLM ──────────────────────────────────
//
// model = "rule:<json>"   e.g. "rule:{\"if\":\"runs.failed > 5\",\"then\":\"alert\",\"else\":\"ok\"}"
// task  = JSON rule tree (alternative to encoding in model string)
//
// Rule format: { "if": "<expr>", "then": "<string>", "else": "<string>" }
// Nested: "then" can itself be a rule object for chaining.

async fn rule_run(state: Arc<AppState>, req: RunRequest) {
    let emit = |step_type: &str, content: &str| {
        state.add_run_step(&req.run_id, step_type, content);
        let msg = json!({ "type": step_type, "content": content, "runId": &req.run_id }).to_string();
        state.broadcast_to_run(&req.run_id, &msg);
    };

    let rule_json = if req.model.starts_with("rule:") {
        req.model["rule:".len()..].to_string()
    } else {
        req.task.clone()
    };

    let rule: Value = match serde_json::from_str(&rule_json) {
        Ok(v) => v,
        Err(e) => {
            emit("error", &format!("rule parse error: {e}"));
            state.finish_run(&req.run_id, RunStatus::Failed);
            return;
        }
    };

    emit("thought", "rule agent: evaluating decision tree");

    let vars = crate::arithmetic::platform_vars(&state);
    let result = eval_rule(&rule, &vars);
    emit("result", &result);
    state.finish_run(&req.run_id, RunStatus::Completed);
}

fn eval_rule(rule: &Value, vars: &std::collections::HashMap<String, f64>) -> String {
    if let (Some(cond), Some(then_branch), Some(else_branch)) = (
        rule.get("if").and_then(|v| v.as_str()),
        rule.get("then"),
        rule.get("else"),
    ) {
        let passed = crate::arithmetic::eval_expr(cond, vars)
            .map(|v| v != 0.0)
            .unwrap_or(false);
        let branch = if passed { then_branch } else { else_branch };
        if branch.is_object() {
            eval_rule(branch, vars)
        } else {
            branch.as_str().unwrap_or(&branch.to_string()).to_string()
        }
    } else {
        rule.as_str().unwrap_or(&rule.to_string()).to_string()
    }
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
