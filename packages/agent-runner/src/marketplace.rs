// Autonomyx Marketplace Router — runtime profile selection.
//
// All providers, operators, and profiles declared in .ayx files are compiled
// into a marketplace registry. At request time, the caller specifies which
// profile to use via:
//   - X-Autonomyx-Profile:   "openai/openai-direct/gpt4o"
//   - X-Autonomyx-Operator:  "openai-direct" + X-Autonomyx-Provider: "openai"
//   - Default: first profile where default=true, or lowest-cost available
//
// ALL profiles share the same AppState (runs, agents, peers, workspace PVC).
// The marketplace only controls which LLM endpoint and credentials to use.
// This means you can switch from gpt-4o to llama3 mid-session without
// losing context — the conversation history lives in AppState, not the provider.
//
// GitOps: this registry is rebuilt on startup from MARKETPLACE_CONFIG env var
// (path to a compiled .ayx JSON) — change the file in git → ArgoCD syncs →
// new pod picks up new registry. No downtime, no manual restarts.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceProfile {
    pub provider:    String,
    pub operator:    String,
    pub name:        String,
    pub model:       String,
    pub base_url:    Option<String>,
    pub api_key_env: Option<String>,
    pub max_tokens:  u32,
    pub context:     u32,
    pub is_default:  bool,
    pub tags:        Vec<String>,
}

impl MarketplaceProfile {
    /// Resolve the API key for this profile from env vars.
    pub fn resolve_key(&self) -> String {
        if let Some(env_var) = &self.api_key_env {
            if let Ok(v) = std::env::var(env_var) {
                if !v.is_empty() {
                    return v;
                }
            }
        }
        // Fallback chain
        crate::providers::resolve_key("")
    }

    /// Resolve the base URL for this profile (env var or compiled-in value).
    pub fn resolve_base_url(&self) -> Option<String> {
        self.base_url.as_ref().map(|url| {
            // Substitute $ENV_VAR references in the URL
            if url.starts_with('$') {
                std::env::var(&url[1..]).unwrap_or_else(|_| url.clone())
            } else {
                url.clone()
            }
        })
    }

    /// Full profile key: "provider/operator/profile"
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.provider, self.operator, self.name)
    }
}

#[derive(Default)]
pub struct MarketplaceRegistry {
    profiles: HashMap<String, MarketplaceProfile>,
    default:  Option<String>,
}

impl MarketplaceRegistry {
    pub fn new() -> Self {
        let mut reg = Self::default();
        reg.load_builtin();
        reg.load_from_env();
        reg
    }

    /// Built-in profiles — always available, no config file needed.
    fn load_builtin(&mut self) {
        // Auto-detect available providers from env
        let profiles = vec![
            // OpenAI
            MarketplaceProfile {
                provider:    "openai".into(),
                operator:    "openai-direct".into(),
                name:        "gpt4o".into(),
                model:       "gpt-4o".into(),
                base_url:    Some("https://api.openai.com".into()),
                api_key_env: Some("OPENAI_API_KEY".into()),
                max_tokens:  4096,
                context:     128000,
                is_default:  false,
                tags:        vec!["llm".into()],
            },
            // Anthropic
            MarketplaceProfile {
                provider:    "anthropic".into(),
                operator:    "anthropic-direct".into(),
                name:        "opus".into(),
                model:       "claude-opus-4-8".into(),
                base_url:    Some("https://api.anthropic.com".into()),
                api_key_env: Some("ANTHROPIC_API_KEY".into()),
                max_tokens:  4096,
                context:     1000000,
                is_default:  false,
                tags:        vec!["llm".into(), "best-quality".into()],
            },
            // Ollama (local — no key)
            MarketplaceProfile {
                provider:    "ollama".into(),
                operator:    "local".into(),
                name:        "llama3".into(),
                model:       "llama3".into(),
                base_url:    None,  // resolved from OLLAMA_BASE_URL
                api_key_env: None,
                max_tokens:  2048,
                context:     8192,
                is_default:  false,
                tags:        vec!["llm".into(), "local".into(), "free".into()],
            },
        ];

        for p in profiles {
            let key = p.key();
            self.profiles.insert(key, p);
        }

        // Set default: first profile whose key env var is populated
        self.default = self.profiles
            .values()
            .find(|p| p.api_key_env.as_ref()
                .map(|e| std::env::var(e).map(|v| !v.is_empty()).unwrap_or(false))
                .unwrap_or(true))
            .map(|p| p.key());
    }

    /// Load additional profiles from MARKETPLACE_<N>_* env vars.
    /// This lets Kubernetes ConfigMaps inject profiles at deploy time.
    fn load_from_env(&mut self) {
        // MARKETPLACE_PROFILE_0_PROVIDER, MARKETPLACE_PROFILE_0_OPERATOR, etc.
        let mut i = 0usize;
        loop {
            let prefix = format!("MARKETPLACE_PROFILE_{i}");
            let provider = match std::env::var(format!("{prefix}_PROVIDER")) {
                Ok(v) => v,
                Err(_) => break,
            };
            let operator    = std::env::var(format!("{prefix}_OPERATOR")).unwrap_or_default();
            let name        = std::env::var(format!("{prefix}_NAME")).unwrap_or_default();
            let model       = std::env::var(format!("{prefix}_MODEL")).unwrap_or_default();
            let base_url    = std::env::var(format!("{prefix}_BASE_URL")).ok();
            let api_key_env = std::env::var(format!("{prefix}_API_KEY_ENV")).ok();
            let is_default  = std::env::var(format!("{prefix}_DEFAULT"))
                .map(|v| v == "true").unwrap_or(false);

            let p = MarketplaceProfile {
                provider, operator, name, model,
                base_url, api_key_env, is_default,
                max_tokens: std::env::var(format!("{prefix}_MAX_TOKENS"))
                    .ok().and_then(|v| v.parse().ok()).unwrap_or(2048),
                context: std::env::var(format!("{prefix}_CONTEXT"))
                    .ok().and_then(|v| v.parse().ok()).unwrap_or(8192),
                tags: vec![],
            };

            if is_default {
                self.default = Some(p.key());
            }
            self.profiles.insert(p.key(), p);
            i += 1;
        }
    }

    /// Look up a profile by "provider/operator/name" key.
    pub fn get(&self, key: &str) -> Option<&MarketplaceProfile> {
        self.profiles.get(key)
    }

    /// Get the default profile (first env-available one).
    pub fn default_profile(&self) -> Option<&MarketplaceProfile> {
        self.default.as_ref().and_then(|k| self.profiles.get(k))
    }

    /// List all profiles (for the /api/marketplace endpoint).
    pub fn list(&self) -> Vec<&MarketplaceProfile> {
        let mut v: Vec<_> = self.profiles.values().collect();
        v.sort_by_key(|p| p.key());
        v
    }

    /// Parse the X-Autonomyx-Profile header: "provider/operator/name"
    pub fn resolve_from_header(header: Option<&str>) -> Option<String> {
        header.map(|h| h.trim().to_string())
    }
}
