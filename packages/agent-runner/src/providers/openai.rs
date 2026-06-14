// OpenAI-compatible wire format — works with:
//   OpenAI (gpt-*), Ollama, vLLM, LM Studio, Together, Groq, Mistral, any proxy.
//
// Base URL resolution (first wins):
//   OPENAI_BASE_URL  — generic override (any OpenAI-compatible endpoint)
//   LLM_BASE_URL     — Autonomyx platform override
//   OLLAMA_BASE_URL  — convenience alias for local Ollama
//   https://api.openai.com — default

use serde_json::{json, Value};
use super::LlmResponse;

fn base_url() -> String {
    for var in ["OPENAI_BASE_URL", "LLM_BASE_URL", "OLLAMA_BASE_URL"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v.trim_end_matches('/').to_string();
            }
        }
    }
    "https://api.openai.com".into()
}

/// Convert Anthropic-style messages to OpenAI chat format.
/// Both already use {role, content} objects, so this is mostly a pass-through.
/// If a "system" role snuck into history it stays; the separate `system` param
/// is prepended as the first user-facing system message.
fn build_messages(system: &str, messages: &[Value]) -> Value {
    let mut out: Vec<Value> = vec![json!({ "role": "system", "content": system })];
    out.extend_from_slice(messages);
    json!(out)
}

pub async fn complete(
    client:     &reqwest::Client,
    model:      &str,
    api_key:    &str,
    system:     &str,
    messages:   &[Value],
    max_tokens: u32,
) -> anyhow::Result<LlmResponse> {
    let url  = format!("{}/v1/chat/completions", base_url());
    let body = json!({
        "model":      model,
        "max_tokens": max_tokens,
        "messages":   build_messages(system, messages),
    });

    let mut req = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&body);

    // API key is optional — self-hosted Ollama and vLLM often skip auth.
    if !api_key.is_empty() {
        req = req.header("authorization", format!("Bearer {api_key}"));
    }

    let resp   = req.send().await?;
    let status = resp.status();

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM {status}: {body}");
    }

    let j: Value  = resp.json().await?;
    let text      = j["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let finish    = j["choices"][0]["finish_reason"].as_str().unwrap_or("");
    let stopped   = finish == "stop";

    Ok(LlmResponse { text, stopped })
}
