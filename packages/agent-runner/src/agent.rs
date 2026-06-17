// Agent runtime — ReAct loop, fully provider-independent.
// LLM backend is selected at call time from model string + env vars.
// Each run is an isolated Tokio task; state is shared via AppState.
//
// Model modes:
//   "eq:<expr>"        — equation agent: arithmetic expression; no LLM. 10k+ concurrent.
//   "rule:<json>"      — rule agent: JSON decision tree; no LLM.
//   "unicode:<op>"     — unicode agent: text analysis, script detection, normalization; no LLM.
//                        ops: info | scripts | graphemes | fold | normalize | words | bytes
//   anything else      — LLM agent: ReAct loop via configured provider.

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
        if req.model.starts_with("unicode:") || req.model == "unicode" {
            unicode_run(state, req).await;
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

// ── Unicode agent — text intelligence, no LLM ────────────────────────────────
//
// model = "unicode:<op>"   e.g. "unicode:scripts", "unicode:info"
// model = "unicode"        + task contains the text to analyse
//
// Operations (op):
//   info       — full Unicode profile of the text
//   scripts    — which Unicode scripts are present and their distribution
//   graphemes  — count code points, chars, words, bytes; identify non-ASCII ranges
//   fold       — case-fold to lowercase (Unicode-aware via char::to_lowercase)
//   normalize  — trim whitespace, collapse runs, remove control characters
//   words      — word frequency map (whitespace-split, case-folded)
//   bytes      — UTF-8 byte distribution and encoding info
//
// Enables multilingual agents, internationalization pipelines, and text routing
// without any LLM call. Scales to 10k+ concurrent agents at microsecond latency.

async fn unicode_run(state: Arc<AppState>, req: RunRequest) {
    let emit = |step_type: &str, content: &str| {
        state.add_run_step(&req.run_id, step_type, content);
        let msg = json!({ "type": step_type, "content": content, "runId": &req.run_id }).to_string();
        state.broadcast_to_run(&req.run_id, &msg);
    };

    let (op, text) = if req.model.starts_with("unicode:") {
        (req.model["unicode:".len()..].to_string(), req.task.clone())
    } else {
        // "unicode" model — first word of task is op, rest is text
        let mut parts = req.task.splitn(2, ' ');
        let op  = parts.next().unwrap_or("info").to_string();
        let txt = parts.next().unwrap_or("").to_string();
        (op, txt)
    };

    emit("thought", &format!("unicode agent: op={op} text_len={}", text.chars().count()));

    let result = unicode_op(&op, &text);
    emit("result", &serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()));
    state.finish_run(&req.run_id, RunStatus::Completed);
}

fn unicode_op(op: &str, text: &str) -> Value {
    match op {
        "scripts" => {
            let mut script_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            for ch in text.chars() {
                let script = unicode_script_name(ch);
                *script_counts.entry(script).or_insert(0) += 1;
            }
            json!({
                "op":          "scripts",
                "total_chars": text.chars().count(),
                "scripts":     script_counts,
            })
        }
        "fold" => {
            let folded: String = text.chars().flat_map(|c| c.to_lowercase()).collect();
            json!({ "op": "fold", "result": folded, "changed": folded != text })
        }
        "normalize" => {
            // trim, collapse whitespace runs, strip ASCII control chars
            let normalized: String = text.chars()
                .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            json!({ "op": "normalize", "result": normalized, "original_len": text.len(), "result_len": normalized.len() })
        }
        "words" => {
            let mut freq: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for word in text.split_whitespace() {
                let folded: String = word.chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(|c| c.to_lowercase())
                    .collect();
                if !folded.is_empty() { *freq.entry(folded).or_insert(0) += 1; }
            }
            let total = freq.values().sum::<usize>();
            let mut sorted: Vec<_> = freq.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            sorted.truncate(50);
            json!({ "op": "words", "unique_words": sorted.len(), "total_words": total, "top": sorted })
        }
        "bytes" => {
            let bytes = text.as_bytes().len();
            let chars = text.chars().count();
            let ascii  = text.chars().filter(|c| c.is_ascii()).count();
            let non_ascii = chars - ascii;
            json!({
                "op":           "bytes",
                "utf8_bytes":   bytes,
                "code_points":  chars,
                "ascii_chars":  ascii,
                "non_ascii":    non_ascii,
                "avg_bytes_per_char": if chars > 0 { bytes as f64 / chars as f64 } else { 0.0 },
            })
        }
        "graphemes" | "info" | _ => {
            let chars    = text.chars().count();
            let bytes    = text.as_bytes().len();
            let words    = text.split_whitespace().count();
            let lines    = text.lines().count();
            let ascii    = text.chars().filter(|c| c.is_ascii()).count();
            let emoji    = text.chars().filter(|c| is_emoji(*c)).count();
            let numeric  = text.chars().filter(|c| c.is_numeric()).count();
            let alpha    = text.chars().filter(|c| c.is_alphabetic()).count();
            let mut scripts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            for ch in text.chars() {
                *scripts.entry(unicode_script_name(ch)).or_insert(0) += 1;
            }
            json!({
                "op":          "info",
                "code_points": chars,
                "utf8_bytes":  bytes,
                "words":       words,
                "lines":       lines,
                "ascii_chars": ascii,
                "non_ascii":   chars - ascii,
                "alphabetic":  alpha,
                "numeric":     numeric,
                "emoji":       emoji,
                "scripts":     scripts,
                "is_ascii_only": ascii == chars,
                "bom_present": text.starts_with('\u{FEFF}'),
            })
        }
    }
}

fn unicode_script_name(c: char) -> &'static str {
    let cp = c as u32;
    match cp {
        0x0000..=0x007F => "Latin/ASCII",
        0x0080..=0x00FF => "Latin Extended",
        0x0100..=0x024F => "Latin Extended-A/B",
        0x0370..=0x03FF => "Greek",
        0x0400..=0x04FF => "Cyrillic",
        0x0500..=0x052F => "Cyrillic Supplement",
        0x0590..=0x05FF => "Hebrew",
        0x0600..=0x06FF => "Arabic",
        0x0700..=0x074F => "Syriac",
        0x0900..=0x097F => "Devanagari",
        0x0980..=0x09FF => "Bengali",
        0x0A00..=0x0A7F => "Gurmukhi",
        0x0A80..=0x0AFF => "Gujarati",
        0x0B00..=0x0B7F => "Oriya",
        0x0B80..=0x0BFF => "Tamil",
        0x0C00..=0x0C7F => "Telugu",
        0x0C80..=0x0CFF => "Kannada",
        0x0D00..=0x0D7F => "Malayalam",
        0x0E00..=0x0E7F => "Thai",
        0x0E80..=0x0EFF => "Lao",
        0x0F00..=0x0FFF => "Tibetan",
        0x1000..=0x109F => "Myanmar",
        0x10A0..=0x10FF => "Georgian",
        0x1100..=0x11FF => "Hangul Jamo",
        0x1E00..=0x1EFF => "Latin Extended Additional",
        0x1F00..=0x1FFF => "Greek Extended",
        0x2000..=0x206F => "General Punctuation",
        0x2070..=0x209F => "Superscripts/Subscripts",
        0x20A0..=0x20CF => "Currency Symbols",
        0x2100..=0x214F => "Letterlike Symbols",
        0x2190..=0x21FF => "Arrows",
        0x2200..=0x22FF => "Mathematical Operators",
        0x2600..=0x26FF => "Miscellaneous Symbols",
        0x2700..=0x27BF => "Dingbats",
        0x3000..=0x303F => "CJK Symbols/Punctuation",
        0x3040..=0x309F => "Hiragana",
        0x30A0..=0x30FF => "Katakana",
        0x3100..=0x312F => "Bopomofo",
        0x3130..=0x318F => "Hangul Compatibility",
        0x3400..=0x4DBF => "CJK Ext-A",
        0x4E00..=0x9FFF => "CJK Unified",
        0xA000..=0xA48F => "Yi",
        0xAC00..=0xD7AF => "Hangul Syllables",
        0xF900..=0xFAFF => "CJK Compatibility",
        0xFB00..=0xFB4F => "Alphabetic Presentation",
        0xFB50..=0xFDFF => "Arabic Presentation-A",
        0xFE30..=0xFE4F => "CJK Compatibility Forms",
        0xFE70..=0xFEFF => "Arabic Presentation-B",
        0xFF00..=0xFFEF => "Halfwidth/Fullwidth",
        0x1F300..=0x1F5FF => "Emoji/Symbols",
        0x1F600..=0x1F64F => "Emoji Faces",
        0x1F680..=0x1F6FF => "Transport Emoji",
        0x1F700..=0x1F77F => "Alchemical Symbols",
        0x1F900..=0x1F9FF => "Supplemental Symbols",
        0x20000..=0x2A6DF => "CJK Ext-B",
        _ => "Other",
    }
}

fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        0x1F300..=0x1F9FF | 0x2600..=0x27BF |
        0x1F000..=0x1F02F | 0x1F0A0..=0x1F0FF |
        0x1FA00..=0x1FA9F | 0x231A..=0x231B |
        0x23E9..=0x23F3  | 0x25AA..=0x25AB |
        0x25B6 | 0x25C0  | 0x25FB..=0x25FE |
        0x2614..=0x2615  | 0x2648..=0x2653 |
        0x267F | 0x2693  | 0x26A1 | 0x26AA..=0x26AB |
        0x26BD..=0x26BE  | 0x26C4..=0x26C5 |
        0x26CE | 0x26D4  | 0x26EA | 0x26F2..=0x26F3 |
        0x26F5 | 0x26FA  | 0x26FD | 0x2702 |
        0x2705 | 0x2708..=0x270D | 0x270F | 0x2712 |
        0x2714 | 0x2716  | 0x271D | 0x2721 |
        0x2728 | 0x2733..=0x2734 | 0x2744 | 0x2747 |
        0x274C | 0x274E  | 0x2753..=0x2755 | 0x2757 |
        0x2763..=0x2764  | 0x2795..=0x2797 | 0x27A1 |
        0x27B0 | 0x27BF  | 0x2934..=0x2935
    )
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
