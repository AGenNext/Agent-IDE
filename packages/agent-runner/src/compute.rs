// Compute Core — the engine that runs agents.
//
// Every agent needs compute. Compute has cost. Cost is metered. Metering is real-time.
// The compute core is where theory meets reality — where the declared agent runs.
//
// "Compute core" = the minimal execution unit:
//   1. Receive task (prompt + context + tools)
//   2. Dispatch to intelligence provider (Anthropic, OpenAI, Ollama, local)
//   3. Stream output (tokens → fabric → WebSocket)
//   4. Record usage (tokens_in, tokens_out, compute_ms, cost_usd)
//   5. Return result + usage record
//
// Provider-agnostic. Model-agnostic. Cost-transparent.
// Same interface whether you're on claude-opus-4-8 or a local GGUF.
//
// Compute profiles:
//   Server:   any model, high throughput, cloud GPU or CPU
//   Edge:     local models only (Ollama), low latency, offline-capable
//   Desktop:  local + remote, IDE-integrated, interactive
//   Embedded: tiny local model only, single inference at a time
//
// "Build for all devices" — the compute core adapts to the device class.
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

// ── Compute request ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRequest {
    pub run_id:    String,
    pub agent_id:  String,
    pub model:     String,
    pub provider:  ComputeProvider,
    pub task:      String,
    pub context:   Value,
    pub tools:     Vec<Value>,
    pub max_tokens: Option<u64>,
    pub stream:    bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeProvider {
    Anthropic,    // Claude — claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5
    OpenAi,       // GPT-4, GPT-4o, o1, o3
    Ollama,       // Local: llama3, mistral, phi3, qwen2 — $0 token cost
    Groq,         // Groq cloud: llama3-70b at 800 tok/s
    Mistral,      // Mistral: mixtral-8x22b, mistral-large
    Google,       // Gemini: gemini-1.5-pro, gemini-flash
    Custom,       // Any OpenAI-compatible endpoint (CUSTOM_LLM_BASE_URL)
    Local,        // GGUF via llama.cpp — fully offline, embedded-capable
}

impl ComputeProvider {
    pub fn from_model(model: &str) -> Self {
        let m = model.to_lowercase();
        if m.starts_with("claude") || m.contains("anthropic") {
            return ComputeProvider::Anthropic;
        }
        if m.starts_with("gpt") || m.starts_with("o1") || m.starts_with("o3") {
            return ComputeProvider::OpenAi;
        }
        if m.starts_with("gemini") {
            return ComputeProvider::Google;
        }
        if m.starts_with("mixtral") || m.starts_with("mistral") {
            return ComputeProvider::Mistral;
        }
        if std::env::var("OLLAMA_HOST").is_ok() || m.contains("llama") || m.contains("phi") || m.contains("qwen") {
            return ComputeProvider::Ollama;
        }
        ComputeProvider::Custom
    }

    pub fn base_url(&self) -> Option<String> {
        match self {
            ComputeProvider::Anthropic => Some("https://api.anthropic.com".into()),
            ComputeProvider::OpenAi    => Some("https://api.openai.com".into()),
            ComputeProvider::Ollama    => Some(
                std::env::var("OLLAMA_HOST")
                    .unwrap_or_else(|_| "http://localhost:11434".into())
            ),
            ComputeProvider::Groq      => Some("https://api.groq.com/openai".into()),
            ComputeProvider::Mistral   => Some("https://api.mistral.ai".into()),
            ComputeProvider::Google    => Some("https://generativelanguage.googleapis.com".into()),
            ComputeProvider::Custom    => std::env::var("CUSTOM_LLM_BASE_URL").ok(),
            ComputeProvider::Local     => None,
        }
    }

    pub fn cost_per_1m_tokens_usd(&self, model: &str) -> (f64, f64) {
        // (input $/1M tokens, output $/1M tokens) — from published rates
        let m = model.to_lowercase();
        match self {
            ComputeProvider::Anthropic => {
                if m.contains("opus-4-8") || m.contains("fable") { (5.0, 25.0) }
                else if m.contains("sonnet-4") { (3.0, 15.0) }
                else if m.contains("haiku-4") { (1.0, 5.0) }
                else { (3.0, 15.0) }
            }
            ComputeProvider::OpenAi => {
                if m.contains("gpt-4o") { (5.0, 15.0) }
                else if m.contains("o1") { (15.0, 60.0) }
                else { (5.0, 15.0) }
            }
            ComputeProvider::Ollama | ComputeProvider::Local => (0.0, 0.0),
            ComputeProvider::Groq    => (0.59, 0.79),
            ComputeProvider::Mistral => (2.0, 6.0),
            ComputeProvider::Google  => (3.5, 10.5),
            ComputeProvider::Custom  => (0.0, 0.0),
        }
    }
}

// ── Compute result ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResult {
    pub run_id:      String,
    pub output:      String,
    pub tokens_in:   u64,
    pub tokens_out:  u64,
    pub compute_ms:  u64,
    pub cost_usd:    f64,
    pub provider:    ComputeProvider,
    pub model:       String,
    pub steps:       Vec<ComputeStep>,
    pub tool_calls:  Vec<Value>,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeStep {
    pub kind:    String,   // "thinking", "tool_use", "text", "tool_result"
    pub content: Value,
    pub at_ms:   u64,
}

// ── Compute engine ────────────────────────────────────────────────────────────

pub struct ComputeEngine;

impl ComputeEngine {
    /// Execute a compute request against the configured intelligence provider.
    /// Returns a result with token usage and cost — always.
    ///
    /// Phase 1: structured stub that returns realistic output shapes.
    /// Phase 2: real HTTP dispatch to each provider's API.
    pub async fn execute(req: ComputeRequest) -> ComputeResult {
        let t0 = Instant::now();

        // Select provider from model string if not specified
        let provider = if req.provider == ComputeProvider::Custom {
            ComputeProvider::from_model(&req.model)
        } else {
            req.provider.clone()
        };

        tracing::info!(
            run_id   = %req.run_id,
            agent_id = %req.agent_id,
            model    = %req.model,
            provider = ?provider,
            task_len = req.task.len(),
            "compute: executing"
        );

        // Dispatch to provider
        let (output, tokens_in, tokens_out, finish_reason) =
            Self::dispatch(&provider, &req).await;

        let compute_ms = t0.elapsed().as_millis() as u64;
        let (cost_in, cost_out) = provider.cost_per_1m_tokens_usd(&req.model);
        let cost_usd = (tokens_in as f64 * cost_in + tokens_out as f64 * cost_out) / 1_000_000.0;

        tracing::info!(
            run_id      = %req.run_id,
            tokens_in   = tokens_in,
            tokens_out  = tokens_out,
            cost_usd    = cost_usd,
            compute_ms  = compute_ms,
            "compute: complete"
        );

        ComputeResult {
            run_id:       req.run_id,
            output,
            tokens_in,
            tokens_out,
            compute_ms,
            cost_usd,
            provider,
            model:        req.model,
            steps:        vec![],
            tool_calls:   vec![],
            finish_reason,
        }
    }

    async fn dispatch(
        provider: &ComputeProvider,
        req: &ComputeRequest,
    ) -> (String, u64, u64, String) {
        match provider {
            ComputeProvider::Anthropic => Self::dispatch_anthropic(req).await,
            ComputeProvider::Ollama    => Self::dispatch_ollama(req).await,
            _                          => Self::dispatch_stub(req),
        }
    }

    /// Anthropic Claude — streaming messages API.
    /// Uses ANTHROPIC_API_KEY from env. No key = informative error.
    async fn dispatch_anthropic(req: &ComputeRequest) -> (String, u64, u64, String) {
        let api_key = match std::env::var("ANTHROPIC_API_KEY") {
            Ok(k) => k,
            Err(_) => {
                return (
                    "ANTHROPIC_API_KEY not set. Set it to enable Claude inference.".into(),
                    0, 0, "error".into(),
                );
            }
        };

        // Build messages API request
        let body = json!({
            "model":      req.model,
            "max_tokens": req.max_tokens.unwrap_or(8096),
            "messages": [
                { "role": "user", "content": req.task }
            ],
            "system": format!(
                "You are agent '{}'. Run ID: {}. Context: {}",
                req.agent_id, req.run_id, req.context
            ),
        });

        let client = reqwest::Client::new();
        match client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                match resp.json::<Value>().await {
                    Ok(v) => {
                        let output = v["content"][0]["text"]
                            .as_str().unwrap_or("").to_string();
                        let tokens_in  = v["usage"]["input_tokens"].as_u64().unwrap_or(0);
                        let tokens_out = v["usage"]["output_tokens"].as_u64().unwrap_or(0);
                        let stop = v["stop_reason"].as_str().unwrap_or("end_turn").to_string();
                        (output, tokens_in, tokens_out, stop)
                    }
                    Err(e) => (format!("parse error: {}", e), 0, 0, "error".into()),
                }
            }
            Err(e) => (format!("http error: {}", e), 0, 0, "error".into()),
        }
    }

    /// Ollama — local inference, $0 token cost.
    /// OLLAMA_HOST defaults to http://localhost:11434
    async fn dispatch_ollama(req: &ComputeRequest) -> (String, u64, u64, String) {
        let host = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".into());

        let body = json!({
            "model":  req.model,
            "prompt": req.task,
            "stream": false,
        });

        let client = reqwest::Client::new();
        match client
            .post(format!("{}/api/generate", host))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                match resp.json::<Value>().await {
                    Ok(v) => {
                        let output     = v["response"].as_str().unwrap_or("").to_string();
                        let tokens_in  = v["prompt_eval_count"].as_u64().unwrap_or(0);
                        let tokens_out = v["eval_count"].as_u64().unwrap_or(0);
                        (output, tokens_in, tokens_out, "stop".into())
                    }
                    Err(e) => (format!("parse error: {}", e), 0, 0, "error".into()),
                }
            }
            Err(e) => (
                format!("Ollama not reachable at {}. Is it running? ollama serve", host),
                0, 0, "error".into(),
            ),
        }
    }

    /// Stub — returns structured placeholder when provider not yet implemented.
    fn dispatch_stub(req: &ComputeRequest) -> (String, u64, u64, String) {
        // Realistic token estimates from task length
        let tokens_in  = (req.task.len() / 4) as u64 + 50;
        let tokens_out = tokens_in / 2 + 100;
        let output = format!(
            "[{}] Task received: {}. Full provider dispatch in Phase 2.",
            req.model, &req.task[..req.task.len().min(80)]
        );
        (output, tokens_in, tokens_out, "end_turn".into())
    }
}

// ── Compute summary ───────────────────────────────────────────────────────────

pub fn engine_summary() -> Value {
    let anthropic_ready = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let openai_ready    = std::env::var("OPENAI_API_KEY").is_ok();
    let ollama_host     = std::env::var("OLLAMA_HOST").ok();
    let groq_ready      = std::env::var("GROQ_API_KEY").is_ok();

    json!({
        "compute_core": {
            "version": env!("CARGO_PKG_VERSION"),
            "dispatch": "provider-agnostic — same interface for all models",
        },
        "providers": {
            "anthropic": {
                "ready":  anthropic_ready,
                "models": ["claude-opus-4-8", "claude-sonnet-4-6", "claude-haiku-4-5"],
                "cost":   "metered at gate — pass-through, no markup",
            },
            "openai": {
                "ready":  openai_ready,
                "models": ["gpt-4o", "o1", "o3-mini"],
            },
            "ollama": {
                "ready":  ollama_host.is_some(),
                "host":   ollama_host,
                "models": ["llama3", "mistral", "phi3", "qwen2", "gemma3"],
                "cost":   "$0 — your compute, your models, your data",
            },
            "groq": {
                "ready":  groq_ready,
                "models": ["llama3-70b-8192", "mixtral-8x7b-32768"],
                "speed":  "800 tok/s",
            },
        },
        "device_profiles": {
            "server":   "all providers, high throughput",
            "edge":     "ollama preferred, low latency, offline-capable",
            "desktop":  "ollama + remote providers, IDE-integrated",
            "embedded": "local GGUF only, single inference",
        },
        "everything_is_possible": true,
    })
}
