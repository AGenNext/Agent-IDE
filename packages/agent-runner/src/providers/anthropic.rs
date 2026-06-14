// Anthropic Messages API wire format — claude-* models.
// Base URL: ANTHROPIC_BASE_URL env or https://api.anthropic.com

use serde_json::{json, Value};
use super::LlmResponse;

fn base_url() -> String {
    std::env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".into())
}

pub async fn complete(
    client:     &reqwest::Client,
    model:      &str,
    api_key:    &str,
    system:     &str,
    messages:   &[Value],
    max_tokens: u32,
) -> anyhow::Result<LlmResponse> {
    let url  = format!("{}/v1/messages", base_url());
    let body = json!({
        "model":      model,
        "max_tokens": max_tokens,
        "system":     system,
        "messages":   messages,
    });

    let resp = client
        .post(&url)
        .header("x-api-key",          api_key)
        .header("anthropic-version",   "2023-06-01")
        .header("content-type",        "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic {status}: {body}");
    }

    let j: Value  = resp.json().await?;
    let text      = j["content"][0]["text"].as_str().unwrap_or("").to_string();
    let stopped   = j["stop_reason"].as_str().unwrap_or("") == "end_turn";

    Ok(LlmResponse { text, stopped })
}
