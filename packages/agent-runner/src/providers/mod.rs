// Provider abstraction — plug in any LLM backend.
// Model string drives provider selection; env vars override everything.
//
// Selection order:
//   1. LLM_PROVIDER=anthropic|openai|ollama (explicit override)
//   2. model starts with "claude-" → Anthropic wire format
//   3. everything else              → OpenAI-compatible wire format
//
// Key resolution (first wins):
//   1. api_key argument passed to RunRequest
//   2. LLM_API_KEY env var
//   3. ANTHROPIC_API_KEY env var (legacy compat)
//   4. OPENAI_API_KEY env var
//   5. empty string (self-hosted / no auth required)
//
// Base URL resolution:
//   - Anthropic path: ANTHROPIC_BASE_URL or https://api.anthropic.com
//   - OpenAI path:    OPENAI_BASE_URL or LLM_BASE_URL or https://api.openai.com
//   - Ollama:         OLLAMA_BASE_URL or http://localhost:11434

pub mod anthropic;
pub mod openai;

use serde_json::Value;

/// Unified response from any LLM provider.
pub struct LlmResponse {
    pub text:    String,
    pub stopped: bool,   // true = natural stop, false = length limit hit
}

/// Resolve which provider to use for a given model string.
pub enum ProviderKind {
    Anthropic,
    OpenAI,
}

pub fn resolve_provider(model: &str) -> ProviderKind {
    match std::env::var("LLM_PROVIDER").as_deref() {
        Ok("anthropic") => return ProviderKind::Anthropic,
        Ok(_)           => return ProviderKind::OpenAI,
        Err(_)          => {}
    }
    if model.starts_with("claude-") {
        ProviderKind::Anthropic
    } else {
        ProviderKind::OpenAI
    }
}

/// Resolve the API key from the call-site value or env fallback chain.
pub fn resolve_key(api_key: &str) -> String {
    if !api_key.is_empty() {
        return api_key.to_string();
    }
    for var in ["LLM_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    String::new()  // self-hosted / no-auth endpoint
}

/// Single call to whatever LLM backend is configured.
/// `messages` must already be in the shape the callee expects (role/content pairs).
pub async fn complete(
    client:    &reqwest::Client,
    model:     &str,
    api_key:   &str,
    system:    &str,
    messages:  &[Value],
    max_tokens: u32,
) -> anyhow::Result<LlmResponse> {
    let key = resolve_key(api_key);
    match resolve_provider(model) {
        ProviderKind::Anthropic => {
            anthropic::complete(client, model, &key, system, messages, max_tokens).await
        }
        ProviderKind::OpenAI => {
            openai::complete(client, model, &key, system, messages, max_tokens).await
        }
    }
}
