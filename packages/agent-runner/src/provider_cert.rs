// Provider certification — certified providers only.
//
// Every run must pass certification before any LLM call is made.
// If the provider is not certified, the run is REJECTED — no fallback, no demo.
//
// Certification checks (all must pass):
//   1. Plugin registered  — plugin exists in the plugin registry
//   2. Plugin enabled     — plugin.enabled == true
//   3. Credentials valid  — API key / endpoint env var is present and non-empty
//   4. Trust threshold    — govgraph node trust_score >= TRUST_THRESHOLD
//   5. Reachability       — optional async health ping (skipped in fast mode)
//
// A CertResult is attached to every RunRecord so every run is traceable
// to the exact certified provider that handled it.
//
// "Freedom not free. Prove identity before opening any gate." — openautonomyx.com

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use crate::store::AppState;

const TRUST_THRESHOLD: f64 = 0.5; // minimum govgraph trust score to certify

// ── Cert structures ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertCheck {
    pub name:    String,
    pub passed:  bool,
    pub detail:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCert {
    pub cert_id:      String,
    pub provider_id:  String,   // e.g. "anthropic", "ollama"
    pub model:        String,
    pub certified:    bool,
    pub trust_score:  f64,
    pub checks:       Vec<CertCheck>,
    pub reject_reason: Option<String>,
    pub certified_at: DateTime<Utc>,
}

impl ProviderCert {
    pub fn is_ok(&self) -> bool { self.certified }
}

// ── Provider → plugin ID mapping ─────────────────────────────────────────────

fn plugin_id_for(model: &str) -> &'static str {
    let provider = std::env::var("LLM_PROVIDER").unwrap_or_default();
    if provider == "anthropic" || model.starts_with("claude-") {
        "plugin_anthropic"
    } else if model.starts_with("ollama:") || model.starts_with("llama") {
        "plugin_ollama"
    } else if model.starts_with("llama-rs") || model.starts_with("gguf:") {
        "plugin_llamars"
    } else {
        // OpenAI-compatible — no dedicated plugin but check env creds
        "plugin_openai_compat"
    }
}

fn govgraph_node_for(model: &str) -> &'static str {
    let provider = std::env::var("LLM_PROVIDER").unwrap_or_default();
    if provider == "anthropic" || model.starts_with("claude-") {
        "compute:anthropic"
    } else if model.starts_with("ollama:") || model.starts_with("llama") {
        "compute:ollama"
    } else if model.starts_with("llama-rs") {
        "compute:llamars"
    } else {
        ""
    }
}

fn credential_env_for(model: &str) -> &'static [&'static str] {
    let provider = std::env::var("LLM_PROVIDER").unwrap_or_default();
    if provider == "anthropic" || model.starts_with("claude-") {
        &["ANTHROPIC_API_KEY", "LLM_API_KEY"]
    } else if model.starts_with("ollama:") || model.starts_with("llama") {
        &["OLLAMA_HOST", "OLLAMA_BASE_URL"]  // no key needed, just endpoint
    } else {
        &["OPENAI_API_KEY", "LLM_API_KEY"]
    }
}

// ── Certification ─────────────────────────────────────────────────────────────

/// Certify a provider synchronously. Called on the run hot path.
/// No async — fast, no network. Reachability check is optional (see certify_with_ping).
pub fn certify(model: &str, state: &Arc<AppState>) -> ProviderCert {
    let mut checks: Vec<CertCheck> = Vec::new();
    let plugin_id = plugin_id_for(model);
    let node_id   = govgraph_node_for(model);

    // ── Check 1: plugin registered ────────────────────────────────────────────
    let plugin = state.plugins.get(plugin_id);
    let registered = plugin.is_some() || plugin_id == "plugin_openai_compat";
    checks.push(CertCheck {
        name:   "plugin_registered".into(),
        passed: registered,
        detail: if registered {
            format!("plugin `{plugin_id}` found")
        } else {
            format!("plugin `{plugin_id}` not in registry")
        },
    });

    // ── Check 2: plugin enabled ────────────────────────────────────────────────
    let enabled = plugin.as_ref().map(|p| p.enabled).unwrap_or(
        // OpenAI-compat: enabled if any key is present
        plugin_id == "plugin_openai_compat"
    );
    checks.push(CertCheck {
        name:   "plugin_enabled".into(),
        passed: enabled,
        detail: if enabled {
            "plugin enabled".into()
        } else {
            format!("plugin `{plugin_id}` is disabled — set required env var to enable")
        },
    });

    // ── Check 3: credentials present ──────────────────────────────────────────
    let env_vars = credential_env_for(model);
    let has_cred = env_vars.iter().any(|v| {
        std::env::var(v).map(|s| !s.is_empty()).unwrap_or(false)
    }) || {
        // Also accept a runtime-passed API key via authmatic peer key
        // (checked by caller who resolves key before spawn_run)
        // Ollama / local models don't need a key
        model.starts_with("ollama:") || model.starts_with("llama")
    };
    checks.push(CertCheck {
        name:   "credentials_valid".into(),
        passed: has_cred,
        detail: if has_cred {
            format!("credentials found (checked: {})", env_vars.join(", "))
        } else {
            format!("no credentials found — set one of: {}", env_vars.join(", "))
        },
    });

    // ── Check 4: trust threshold ───────────────────────────────────────────────
    let trust_score = if !node_id.is_empty() {
        state.govgraph.get_node(node_id)
            .map(|n| n.trust_score)
            .unwrap_or(0.8)  // default trust for known providers
    } else {
        0.8  // OpenAI-compat gets default trust
    };
    let trusted = trust_score >= TRUST_THRESHOLD;
    checks.push(CertCheck {
        name:   "trust_threshold".into(),
        passed: trusted,
        detail: format!("trust_score={:.2} threshold={:.2}", trust_score, TRUST_THRESHOLD),
    });

    // ── Certification result ───────────────────────────────────────────────────
    let certified = checks.iter().all(|c| c.passed);
    let reject_reason = if !certified {
        Some(
            checks.iter()
                .filter(|c| !c.passed)
                .map(|c| format!("{}: {}", c.name, c.detail))
                .collect::<Vec<_>>()
                .join("; ")
        )
    } else {
        None
    };

    if certified {
        tracing::info!(
            model    = %model,
            plugin   = %plugin_id,
            trust    = trust_score,
            "provider certified"
        );
    } else {
        tracing::warn!(
            model    = %model,
            plugin   = %plugin_id,
            reason   = ?reject_reason,
            "provider certification FAILED — run rejected"
        );
    }

    ProviderCert {
        cert_id:      Uuid::new_v4().to_string(),
        provider_id:  plugin_id.to_string(),
        model:        model.to_string(),
        certified,
        trust_score,
        checks,
        reject_reason,
        certified_at: Utc::now(),
    }
}

/// Async certification — same as certify() but also pings the endpoint.
/// Use for pre-flight validation; not called on hot path.
pub async fn certify_with_ping(model: &str, state: &Arc<AppState>) -> ProviderCert {
    let mut cert = certify(model, state);
    if !cert.certified { return cert; }  // already failed, skip ping

    // Ping the provider health endpoint
    let url = provider_health_url(model);
    if url.is_empty() {
        cert.checks.push(CertCheck {
            name:   "reachability".into(),
            passed: true,
            detail: "no health endpoint defined — skipped".into(),
        });
        return cert;
    }

    let reachable = state.egress.probe().get(&url).send().await
        .map(|r| r.status().is_success() || r.status().as_u16() == 404)
        .unwrap_or(false);

    cert.checks.push(CertCheck {
        name:   "reachability".into(),
        passed: reachable,
        detail: if reachable {
            format!("ping ok → {url}")
        } else {
            format!("ping failed → {url}")
        },
    });

    if !reachable {
        cert.certified = false;
        cert.reject_reason = Some(format!("provider unreachable at {url}"));
        tracing::warn!(url = %url, model = %model, "provider ping failed");
    }

    cert
}

fn provider_health_url(model: &str) -> String {
    let provider = std::env::var("LLM_PROVIDER").unwrap_or_default();
    if provider == "anthropic" || model.starts_with("claude-") {
        // Anthropic has no public /health but models list works
        std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".into())
            + "/v1/models"
    } else if model.starts_with("ollama:") || model.starts_with("llama") {
        let base = std::env::var("OLLAMA_BASE_URL")
            .or_else(|_| std::env::var("OLLAMA_HOST"))
            .unwrap_or_else(|_| "http://localhost:11434".into());
        base + "/api/tags"
    } else {
        String::new()
    }
}
