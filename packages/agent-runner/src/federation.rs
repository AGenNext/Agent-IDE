// Autonomyx Federation — the identity layer of the platform.
//
// Every agent is:
//   Real        — hardware-backed Ed25519 keypair (HSM/TPM in production)
//   Unique      — globally unique DID: did:autonomyx:<base58-pubkey>
//   Identifiable — resolvable DID Document; any peer can look up any agent
//   Governed    — GovernancePolicy per DID; caps, TTL bounds, operators
//   Autonomous  — self-sovereign; holds own keys; signs own claims
//   Federal     — federated across peers; no central authority
//   Accountable — signed audit log; every action is non-repudiable
//   Intelligent — DID binds to a marketplace profile → LLM provider
//
// Federation model:
//   Each Autonomyx node maintains a local DID registry (SurrealDB).
//   Peers exchange DID documents via the /transfer push protocol.
//   A DID can be resolved by: local lookup → peer broadcast → fail.
//   No central resolver. The network IS the registry.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::identity::{AccessGrant, AgentIdentity};

// ── DID Document (W3C DID Core 1.1 subset) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidDocument {
    #[serde(rename = "@context")]
    pub context:              Vec<String>,
    pub id:                   String,          // did:autonomyx:<pubkey>
    pub controller:           String,          // same as id (self-sovereign)
    pub verification_method:  Vec<VerificationMethod>,
    pub authentication:       Vec<String>,     // refs to verification_method ids
    pub assertion_method:     Vec<String>,     // for signing claims
    pub capability_delegation: Vec<String>,    // who can delegate on behalf
    pub service:              Vec<ServiceEndpoint>,
    pub created:              DateTime<Utc>,
    pub updated:              DateTime<Utc>,
    pub governance:           GovernancePolicy,
    pub intelligence:         IntelligenceBinding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationMethod {
    pub id:                  String,   // did:autonomyx:<pubkey>#key-1
    #[serde(rename = "type")]
    pub kind:                String,   // "Ed25519VerificationKey2020"
    pub controller:          String,   // the DID
    pub public_key_base58:   String,   // base58-encoded public key
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub id:              String,
    #[serde(rename = "type")]
    pub kind:            ServiceKind,
    pub service_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ServiceKind {
    AutonomyxRunner,      // the agent's runtime API endpoint
    AutonomyxFabric,      // fabric event subscription endpoint
    AutonomyxPeer,        // peer-to-peer transfer endpoint
    AutonomyxMarketplace, // marketplace profile resolution
}

// ── Governance Policy — rules per DID ────────────────────────────────────────
//
// Governs what an agent is allowed to do.
// Enforced at the gate level: if a grant exceeds policy, gate closes.
// Policy is declared in the DID Document — self-sovereign governance.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernancePolicy {
    pub max_grant_ttl_secs: u64,          // no grant can exceed this TTL
    pub allowed_capabilities: Vec<String>, // e.g. ["tool:web_search", "profile:*"]
    pub allowed_operators:   Vec<String>, // e.g. ["openai", "anthropic", "ollama"]
    pub require_mfa:         bool,        // require multi-factor for high-risk ops
    pub audit_all:           bool,        // log every action to the accountability log
    pub federation_level:    FederationLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationLevel {
    Local,    // only within this node
    Cluster,  // within the k8s cluster
    Peer,     // across registered peer nodes
    Global,   // any node that resolves this DID
}

impl Default for GovernancePolicy {
    fn default() -> Self {
        Self {
            max_grant_ttl_secs:   300,   // 5 minutes maximum JIT grant
            allowed_capabilities: vec!["tool:*".into(), "profile:*".into()],
            allowed_operators:    vec!["openai".into(), "anthropic".into(), "ollama".into()],
            require_mfa:          false,
            audit_all:            true,
            federation_level:     FederationLevel::Peer,
        }
    }
}

// ── Intelligence Binding — DID → LLM provider ────────────────────────────────
//
// An agent's intelligence is a first-class property of its identity.
// The DID document declares which provider and profile the agent uses.
// Changing the binding updates the DID Document (version bump, re-signed).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceBinding {
    pub provider:    String,   // "openai" | "anthropic" | "ollama" | ...
    pub operator:    String,   // "direct" | "azure" | "bedrock" | ...
    pub profile:     String,   // "gpt-4o" | "claude-opus-4-8" | "llama3" | ...
    pub reasoning:   ReasoningMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    React,      // Reason + Act loop (default)
    Chain,      // chain-of-thought, no tools
    Adaptive,   // model decides (claude thinking / o1 reasoning)
    Direct,     // single-shot, no loop
}

// ── Accountability Log — signed, append-only ──────────────────────────────────
//
// Every action an agent takes is recorded here.
// The record is signed by the agent's key — non-repudiable.
// The log is replicated to SurrealDB (live queries alert on anomalies).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountabilityRecord {
    pub id:          String,
    pub did:         String,         // agent that acted
    pub action:      String,         // what was done
    pub resource:    String,         // what it was done to
    pub grant_id:    Option<String>, // the JIT grant authorising the action
    pub outcome:     ActionOutcome,
    pub evidence:    serde_json::Value, // proof — gate record, OTel trace, etc.
    pub signature:   String,         // hex Ed25519 sig over the record fields
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Success,
    Denied,    // grant refused or policy violation
    Failed,    // attempt made but failed
    Partial,   // partial completion (circuit breaker fired)
}

impl AccountabilityRecord {
    pub fn payload_bytes(&self) -> Vec<u8> {
        format!("{}:{}:{}:{}:{:?}:{}",
            self.id, self.did, self.action, self.resource,
            self.outcome, self.recorded_at.timestamp()
        ).into_bytes()
    }

    pub fn sign_with(&mut self, identity: &AgentIdentity) {
        let sig = identity.sign(&self.payload_bytes());
        self.signature = sig.iter().map(|b| format!("{b:02x}")).collect();
    }

    pub fn verify_with(&self, identity: &AgentIdentity) -> bool {
        let sig_bytes: Vec<u8> = (0..self.signature.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&self.signature[i..i+2], 16).ok())
            .collect();
        if sig_bytes.len() != 64 { return false; }
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&sig_bytes);
        identity.verify(&self.payload_bytes(), &sig)
    }
}

// ── Federation Registry ────────────────────────────────────────────────────────
//
// Maintains the local DID Document store.
// DID resolution: local → peer broadcast → fail.
// Documents are exchanged via the /transfer push protocol (egress-push only).

struct FederationStore {
    documents:      HashMap<String, DidDocument>,       // did → document
    accountability: HashMap<String, Vec<AccountabilityRecord>>, // did → log
}

#[derive(Clone)]
pub struct FederationRegistry {
    inner: Arc<RwLock<FederationStore>>,
}

impl FederationRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(FederationStore {
                documents:      HashMap::new(),
                accountability: HashMap::new(),
            })),
        }
    }

    // ── Registration ──────────────────────────────────────────────────────────

    /// Register a new agent identity — creates and stores its DID Document.
    pub fn register(
        &self,
        identity:  &AgentIdentity,
        endpoint:  &str,    // the agent's runtime API URL
        policy:    GovernancePolicy,
        intel:     IntelligenceBinding,
    ) -> DidDocument {
        let key_id  = format!("{}#key-1", identity.did);
        let pub_b58 = base58_encode(&identity.public_key);

        let doc = DidDocument {
            context: vec![
                "https://www.w3.org/ns/did/v1".into(),
                "https://w3id.org/security/suites/ed25519-2020/v1".into(),
                "https://autonomyx.io/did/v1".into(),
            ],
            id:          identity.did.clone(),
            controller:  identity.did.clone(),
            verification_method: vec![VerificationMethod {
                id:                key_id.clone(),
                kind:              "Ed25519VerificationKey2020".into(),
                controller:        identity.did.clone(),
                public_key_base58: pub_b58,
            }],
            authentication:        vec![key_id.clone()],
            assertion_method:      vec![key_id.clone()],
            capability_delegation: vec![key_id.clone()],
            service: vec![ServiceEndpoint {
                id:               format!("{}#runner", identity.did),
                kind:             ServiceKind::AutonomyxRunner,
                service_endpoint: endpoint.into(),
            }],
            created:    Utc::now(),
            updated:    Utc::now(),
            governance: policy,
            intelligence: intel,
        };

        self.inner.write().unwrap().documents.insert(identity.did.clone(), doc.clone());
        tracing::info!(did = %identity.did, "federation: agent registered");
        doc
    }

    // ── Resolution ────────────────────────────────────────────────────────────

    /// Resolve a DID Document locally.
    pub fn resolve(&self, did: &str) -> Option<DidDocument> {
        self.inner.read().unwrap().documents.get(did).cloned()
    }

    /// Ingest a DID Document from a peer (federation push).
    pub fn ingest_peer_document(&self, doc: DidDocument) {
        tracing::info!(did = %doc.id, "federation: peer DID document ingested");
        self.inner.write().unwrap().documents.insert(doc.id.clone(), doc);
    }

    /// All known DIDs (local + federated peers).
    pub fn list_dids(&self) -> Vec<String> {
        self.inner.read().unwrap().documents.keys().cloned().collect()
    }

    // ── Governance enforcement ────────────────────────────────────────────────

    /// Check if a grant is within the agent's governance policy.
    pub fn check_grant(&self, grant: &AccessGrant) -> Result<(), String> {
        let store = self.inner.read().unwrap();
        let doc = store.documents.get(&grant.identity)
            .ok_or_else(|| format!("DID not registered: {}", grant.identity))?;

        let ttl = grant.expires_at.saturating_sub(grant.issued_at);
        if ttl > doc.governance.max_grant_ttl_secs {
            return Err(format!(
                "grant TTL {ttl}s exceeds policy max {}s for {}",
                doc.governance.max_grant_ttl_secs, grant.identity
            ));
        }

        let cap_allowed = doc.governance.allowed_capabilities.iter().any(|allowed| {
            allowed == "*"
            || allowed == &grant.operation
            || (allowed.ends_with(":*") && grant.operation.starts_with(&allowed[..allowed.len()-1]))
        });
        if !cap_allowed {
            return Err(format!(
                "capability '{}' not in governance policy for {}",
                grant.operation, grant.identity
            ));
        }

        Ok(())
    }

    // ── Accountability ────────────────────────────────────────────────────────

    /// Record an action in the signed accountability log.
    pub fn record(
        &self,
        identity:  &AgentIdentity,
        action:    &str,
        resource:  &str,
        grant_id:  Option<String>,
        outcome:   ActionOutcome,
        evidence:  serde_json::Value,
    ) -> AccountabilityRecord {
        let mut rec = AccountabilityRecord {
            id:          Uuid::new_v4().to_string(),
            did:         identity.did.clone(),
            action:      action.into(),
            resource:    resource.into(),
            grant_id,
            outcome,
            evidence,
            signature:   String::new(),
            recorded_at: Utc::now(),
        };
        rec.sign_with(identity);

        tracing::info!(
            did    = %rec.did,
            action = %rec.action,
            record = %rec.id,
            "accountability: action recorded"
        );

        self.inner.write().unwrap()
            .accountability.entry(identity.did.clone())
            .or_default()
            .push(rec.clone());

        rec
    }

    /// Full accountability log for a DID.
    pub fn audit_log(&self, did: &str) -> Vec<AccountabilityRecord> {
        self.inner.read().unwrap()
            .accountability.get(did)
            .cloned()
            .unwrap_or_default()
    }

    /// All records across all agents (ops/compliance view).
    pub fn full_audit_log(&self) -> Vec<AccountabilityRecord> {
        let store = self.inner.read().unwrap();
        let mut all: Vec<_> = store.accountability.values()
            .flat_map(|v| v.iter().cloned())
            .collect();
        all.sort_by_key(|r| r.recorded_at);
        all
    }
}

impl Default for FederationRegistry {
    fn default() -> Self { Self::new() }
}

// ── base58 helper (re-export from the local impl) ────────────────────────────

fn base58_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut result = Vec::new();
    let mut num = bytes.to_vec();
    while num.iter().any(|&b| b != 0) {
        let mut rem = 0u32;
        let mut next = Vec::new();
        for &b in &num {
            let cur = rem * 256 + b as u32;
            next.push((cur / 58) as u8);
            rem = cur % 58;
        }
        result.push(ALPHA[rem as usize]);
        num = next.into_iter().skip_while(|&b| b == 0).collect();
    }
    for &b in bytes { if b == 0 { result.push(ALPHA[0]); } else { break; } }
    result.reverse();
    String::from_utf8(result).unwrap_or_default()
}
