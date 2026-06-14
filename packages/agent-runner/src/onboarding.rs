// Chat-based configuration and onboarding — Autonomyx platform.
//
// The chat IS the configuration interface.
// Describe what you want to build. The platform builds it.
// No YAML. No forms. No onboarding wizard. Just a conversation.
//
// The conversation drives:
//   1. Application name + purpose (what are you building?)
//   2. Intelligence binding (which LLM? which provider?)
//   3. Agent composition (what agents do you need?)
//   4. Governance policy (who controls it? what's the budget?)
//   5. Deployment target (which cloud? self-hosted?)
//
// At the end: a real Application with a real DID — theory made real.
// openautonomyx.com

use std::collections::HashMap;
use std::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde_json::{json, Value};

// ── Onboarding session ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStage {
    Welcome,         // first contact
    Purpose,         // "what are you building?"
    Intelligence,    // "which LLM?"
    Agents,          // "what agents do you need?"
    Governance,      // "who controls it and what's the budget?"
    Deployment,      // "where does it run?"
    Review,          // "here's your config — confirm?"
    Complete,        // app created, DID assigned
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role:    String,   // "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingSession {
    pub id:          String,
    pub owner_id:    String,
    pub stage:       OnboardingStage,
    pub messages:    Vec<ChatMessage>,
    // Configuration being assembled through the conversation
    pub config:      OnboardingConfig,
    pub app_id:      Option<String>,   // set when application is created
    pub app_did:     Option<String>,   // set when application is activated
    pub created_at:  DateTime<Utc>,
    pub updated_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OnboardingConfig {
    pub app_name:      Option<String>,
    pub description:   Option<String>,
    pub version:       Option<String>,
    pub model:         Option<String>,
    pub provider:      Option<String>,
    pub agents:        Vec<AgentConfig>,
    pub budget_usd:    Option<f64>,
    pub cloud:         Option<String>,
    pub governance:    Option<GovernanceConfig>,
    pub ayx_source:    Option<String>,   // generated .ayx declaration
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub role: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    pub max_grant_ttl: u32,
    pub audit_all:     bool,
    pub require_mfa:   bool,
}

// ── Onboarding registry ───────────────────────────────────────────────────────

pub struct OnboardingRegistry {
    sessions: RwLock<HashMap<String, OnboardingSession>>,
}

impl OnboardingRegistry {
    pub fn new() -> Self {
        Self { sessions: RwLock::new(HashMap::new()) }
    }

    pub fn create_session(&self, owner_id: &str) -> OnboardingSession {
        let id = format!("onboard_{}", Uuid::new_v4().simple());
        let session = OnboardingSession {
            id: id.clone(),
            owner_id: owner_id.into(),
            stage: OnboardingStage::Welcome,
            messages: vec![],
            config: OnboardingConfig::default(),
            app_id: None,
            app_did: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.sessions.write().unwrap().insert(id, session.clone());
        session
    }

    pub fn get_session(&self, id: &str) -> Option<OnboardingSession> {
        self.sessions.read().unwrap().get(id).cloned()
    }

    pub fn update_session(&self, session: OnboardingSession) {
        self.sessions.write().unwrap().insert(session.id.clone(), session);
    }

    pub fn list_sessions(&self) -> Vec<OnboardingSession> {
        self.sessions.read().unwrap().values().cloned().collect()
    }
}

// ── System prompt — the platform's onboarding voice ──────────────────────────

pub fn onboarding_system_prompt() -> &'static str {
    r#"You are the Autonomyx platform onboarding assistant.

Autonomyx is the platform where anyone can build their next — any agent, any cloud, any LLM.
"Everyone and everything is an agent. Multi-ecosystem. Single world model. Infinite scale."

Your job: guide the user through configuring their application through natural conversation.
Keep responses short, warm, and specific. Ask one question at a time. Move forward.

You are collecting configuration to generate a .ayx application declaration. The stages:
1. WELCOME — greet and ask what they want to build
2. PURPOSE — understand the app name, description, and goal
3. INTELLIGENCE — which LLM provider and model? (anthropic/claude-opus-4-8, openai/gpt-4o, ollama/llama3, or any OpenAI-compatible)
4. AGENTS — what agents does the app need? (researcher, builder, coordinator, etc.)
5. GOVERNANCE — monthly budget (USD), who controls it, cloud target (aws/gcp/azure/hetzner/k3s/local)
6. REVIEW — present the .ayx config for confirmation
7. COMPLETE — confirm creation, tell them their app DID

Rules:
- Extract configuration values from their answers even if phrased casually
- When you have enough info for the current stage, advance to the next
- If they say "skip" or "default", use sensible defaults
- Keep the .ayx declaration simple and readable
- Remind them: self-hosted = $0 token cost; cloud providers = published rates, no markup

When generating the .ayx config in REVIEW stage, produce it as a ```ayx code block.
When in COMPLETE stage, summarise what was built: app name, DID, agents, cloud target.

Response format: conversational prose. No bullet walls. One clear question to move forward."#
}

// ── Configuration extraction ──────────────────────────────────────────────────
// Parse LLM response to extract config signals for each stage.

pub fn extract_config_from_response(stage: &OnboardingStage, response: &str, user_msg: &str) -> OnboardingConfig {
    let mut config = OnboardingConfig::default();
    let combined = format!("{user_msg} {response}").to_lowercase();

    match stage {
        OnboardingStage::Purpose => {
            // Extract app name — look for quoted names or "called X" / "named X"
            if let Some(name) = extract_quoted(&combined).or_else(|| extract_after_keyword(&combined, "called")) {
                config.app_name = Some(capitalise(&name));
            }
        }
        OnboardingStage::Intelligence => {
            // Model detection
            if combined.contains("claude") || combined.contains("anthropic") {
                config.provider = Some("anthropic".into());
                if combined.contains("haiku") {
                    config.model = Some("claude-haiku-4-5-20251001".into());
                } else if combined.contains("sonnet") {
                    config.model = Some("claude-sonnet-4-6".into());
                } else {
                    config.model = Some("claude-opus-4-8".into());
                }
            } else if combined.contains("ollama") || combined.contains("llama") || combined.contains("local") || combined.contains("self") {
                config.provider = Some("ollama".into());
                config.model = Some("llama3".into());
            } else if combined.contains("gpt") || combined.contains("openai") {
                config.provider = Some("openai".into());
                config.model = Some("gpt-4o".into());
            }
        }
        OnboardingStage::Governance => {
            // Budget extraction
            if let Some(budget) = extract_number(&combined) {
                config.budget_usd = Some(budget);
            }
            // Cloud target
            for cloud in ["aws", "gcp", "azure", "hetzner", "k3s", "local"] {
                if combined.contains(cloud) {
                    config.cloud = Some(cloud.into());
                    break;
                }
            }
            if combined.contains("self") || combined.contains("home") || combined.contains("metal") {
                config.cloud = Some("local".into());
            }
        }
        _ => {}
    }

    config
}

/// Generate a .ayx application declaration from the assembled config.
pub fn generate_ayx(config: &OnboardingConfig) -> String {
    let name = config.app_name.as_deref().unwrap_or("MyApp").replace(' ', "");
    let description = config.description.as_deref().unwrap_or("Built on Autonomyx");
    let version = config.version.as_deref().unwrap_or("0.1.0");
    let provider = config.provider.as_deref().unwrap_or("anthropic");
    let model = config.model.as_deref().unwrap_or("claude-opus-4-8");
    let budget = config.budget_usd.unwrap_or(50.0);
    let cloud = config.cloud.as_deref().unwrap_or("k3s");

    let agents_block = if config.agents.is_empty() {
        r#"    agent Assistant {
      role "general purpose assistant"
      mode adaptive
    }"#.to_string()
    } else {
        config.agents.iter().map(|a| {
            format!("    agent {} {{\n      role {:?}\n      mode {}\n    }}", a.name, a.role, a.mode)
        }).collect::<Vec<_>>().join("\n\n")
    };

    format!(r#"// {name}.ayx — Application declaration
// Generated by Autonomyx onboarding. Edit freely.
// openautonomyx.com

application {name} {{
  version     "{version}"
  description "{description}"

  intelligence {{
    provider "{provider}"
    model    "{model}"
    mode     adaptive
  }}

  agents {{
{agents_block}
  }}

  lifecycle {{
    gate build  {{ oath "bom_present"          }}
    gate sign   {{ oath "signature_valid"       }}
    gate deploy {{ oath "governance_satisfied"  }}
    gate run    {{ oath "budget_available"      }}
  }}

  budget {{
    monthly_usd {budget:.2}
    on_exceed   "pause"
  }}

  deploy {{
    target "{cloud}"
  }}
}}
"#)
}

// ── Stage advancement logic ───────────────────────────────────────────────────

pub fn should_advance(stage: &OnboardingStage, user_msg: &str, config: &OnboardingConfig) -> bool {
    let msg = user_msg.to_lowercase();
    match stage {
        OnboardingStage::Welcome      => true,
        OnboardingStage::Purpose      => config.app_name.is_some() || msg.len() > 5,
        OnboardingStage::Intelligence => config.model.is_some() || msg.contains("skip") || msg.contains("default"),
        OnboardingStage::Agents       => msg.len() > 5,
        OnboardingStage::Governance   => config.budget_usd.is_some() || msg.contains("skip"),
        OnboardingStage::Deployment   => config.cloud.is_some() || msg.contains("skip") || msg.contains("default"),
        OnboardingStage::Review       => msg.contains("yes") || msg.contains("confirm") || msg.contains("build") || msg.contains("deploy") || msg.contains("create"),
        OnboardingStage::Complete     => false,
    }
}

pub fn next_stage(stage: &OnboardingStage) -> OnboardingStage {
    match stage {
        OnboardingStage::Welcome      => OnboardingStage::Purpose,
        OnboardingStage::Purpose      => OnboardingStage::Intelligence,
        OnboardingStage::Intelligence => OnboardingStage::Agents,
        OnboardingStage::Agents       => OnboardingStage::Governance,
        OnboardingStage::Governance   => OnboardingStage::Deployment,
        OnboardingStage::Deployment   => OnboardingStage::Review,
        OnboardingStage::Review       => OnboardingStage::Complete,
        OnboardingStage::Complete     => OnboardingStage::Complete,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_quoted(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    for &q in &['"', '\'', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}'] {
        if let Some(start) = chars.iter().position(|&c| c == q) {
            if let Some(end) = chars[start+1..].iter().position(|&c| c == q) {
                let word: String = chars[start+1..start+1+end].iter().collect();
                if !word.is_empty() { return Some(word); }
            }
        }
    }
    None
}

fn extract_after_keyword(s: &str, keyword: &str) -> Option<String> {
    if let Some(pos) = s.find(keyword) {
        let rest = s[pos + keyword.len()..].trim();
        let word: String = rest.split_whitespace().next()?.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if !word.is_empty() { return Some(word); }
    }
    None
}

fn extract_number(s: &str) -> Option<f64> {
    for token in s.split_whitespace() {
        let clean: String = token.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
        if let Ok(n) = clean.parse::<f64>() {
            if n > 0.0 && n < 100_000.0 { return Some(n); }
        }
    }
    None
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

// ── LLM message builder for onboarding ───────────────────────────────────────

pub fn build_messages(session: &OnboardingSession) -> Vec<Value> {
    session.messages.iter().map(|m| json!({
        "role":    m.role,
        "content": m.content,
    })).collect()
}
