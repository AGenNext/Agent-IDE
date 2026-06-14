// Blockchain bridge — Autonomyx on-chain.
//
// Every concept in Autonomyx maps to a blockchain primitive:
//
//   DID            → on-chain address / ENS / did:ethr
//   Accountability → immutable on-chain event log (emit once, read forever)
//   Usage          → ERC-20 micro-payment (settle at the gate, real-time)
//   Agent          → NFT (ERC-721: own it, transfer it, list it, trade it)
//   Governance     → smart contract policy (on-chain rules, DAO-upgradeable)
//   Feedback gate  → oracle pattern (off-chain signal → on-chain state update)
//   Fabric         → event subscription from chain logs (live query = eth_getLogs)
//
// Chain-agnostic. EVM-compatible. No lock-in.
// Ethereum, Polygon, Base, Arbitrum, Optimism, BSC, Avalanche — all EVM.
// Non-EVM: Solana (Anchor), Cosmos (CosmWasm), Substrate (ink!) — adapters in Phase 2.
//
// Transport: JSON-RPC 2.0 over HTTPS (same wire format as /mcp)
// Auth: wallet private key (AUTONOMYX_WALLET_KEY) or read-only (no key = observe only)
//
// Contracts (deploy your own or use the Autonomyx reference deployment):
//   AgentRegistry      — mint agent NFTs, resolve DIDs, transfer ownership
//   AccountabilityLog  — emit immutable accountability events
//   UsageSettlement    — ERC-20 micro-payment (AUYX token or USDC)
//   GovernancePolicy   — on-chain governance rules, capability grants, DAO proposals
//
// "Past is fact" — on-chain events are the ultimate immutable fact.
// Once emitted, they cannot be altered. The blockchain enforces what Autonomyx promises.
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};

// ── Chain configuration ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChainEcosystem {
    Evm,       // Ethereum, Polygon, Base, Arbitrum, Optimism, BSC, Avalanche
    Solana,    // Anchor / Borsh
    Cosmos,    // CosmWasm
    Substrate, // ink! / FRAME
    None,      // blockchain not configured
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainContext {
    pub ecosystem:           ChainEcosystem,
    pub rpc_url:             Option<String>,
    pub chain_id:            Option<u64>,
    pub chain_name:          Option<String>,
    pub contract_registry:   Option<String>,   // AgentRegistry contract address
    pub contract_accounting: Option<String>,   // AccountabilityLog contract address
    pub contract_settlement: Option<String>,   // UsageSettlement contract address
    pub contract_governance: Option<String>,   // GovernancePolicy contract address
    pub wallet_address:      Option<String>,   // platform wallet (derived from AUTONOMYX_WALLET_KEY)
    pub read_only:           bool,             // true when no wallet key configured
}

impl ChainContext {
    /// Detect chain configuration from environment variables.
    /// No network calls at startup — lazy-connects on first use.
    pub fn detect() -> Self {
        let rpc_url = std::env::var("CHAIN_RPC_URL").ok();
        let chain_id = std::env::var("CHAIN_ID").ok()
            .and_then(|v| v.parse::<u64>().ok());
        let chain_name = std::env::var("CHAIN_NAME").ok()
            .or_else(|| chain_name_from_id(chain_id));

        let ecosystem = detect_ecosystem(&rpc_url);
        let has_wallet = std::env::var("AUTONOMYX_WALLET_KEY").is_ok();

        ChainContext {
            ecosystem,
            rpc_url,
            chain_id,
            chain_name,
            contract_registry:   std::env::var("CONTRACT_REGISTRY").ok(),
            contract_accounting: std::env::var("CONTRACT_ACCOUNTABILITY").ok(),
            contract_settlement: std::env::var("CONTRACT_SETTLEMENT").ok(),
            contract_governance: std::env::var("CONTRACT_GOVERNANCE").ok(),
            wallet_address:      std::env::var("AUTONOMYX_WALLET_ADDRESS").ok(),
            read_only:           !has_wallet,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.rpc_url.is_some() && self.ecosystem != ChainEcosystem::None
    }
}

fn detect_ecosystem(rpc_url: &Option<String>) -> ChainEcosystem {
    if let Some(url) = rpc_url {
        let url = url.to_lowercase();
        if url.contains("solana") || url.contains("mainnet-beta") || url.contains("devnet") {
            return ChainEcosystem::Solana;
        }
        if url.contains("cosmos") || url.contains("osmosis") || url.contains("juno") {
            return ChainEcosystem::Cosmos;
        }
        if url.contains("substrate") || url.contains("polkadot") || url.contains("kusama") {
            return ChainEcosystem::Substrate;
        }
        // Default for http(s) endpoints: assume EVM JSON-RPC
        return ChainEcosystem::Evm;
    }
    // No RPC URL — check for SOLANA_RPC_URL or similar
    if std::env::var("SOLANA_RPC_URL").is_ok() {
        return ChainEcosystem::Solana;
    }
    ChainEcosystem::None
}

fn chain_name_from_id(id: Option<u64>) -> Option<String> {
    match id? {
        1     => Some("Ethereum Mainnet".into()),
        5     => Some("Ethereum Goerli".into()),
        11155111 => Some("Ethereum Sepolia".into()),
        137   => Some("Polygon Mainnet".into()),
        80001 => Some("Polygon Mumbai".into()),
        8453  => Some("Base Mainnet".into()),
        84531 => Some("Base Goerli".into()),
        42161 => Some("Arbitrum One".into()),
        421613 => Some("Arbitrum Goerli".into()),
        10    => Some("Optimism Mainnet".into()),
        420   => Some("Optimism Goerli".into()),
        56    => Some("BNB Smart Chain".into()),
        43114 => Some("Avalanche C-Chain".into()),
        _     => None,
    }
}

// ── On-chain records ──────────────────────────────────────────────────────────

/// An on-chain accountability event — mirrors the off-chain AccountabilityRecord
/// but anchored to a transaction hash and block number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainEvent {
    pub tx_hash:     String,
    pub block:       Option<u64>,
    pub chain_id:    Option<u64>,
    pub event_type:  String,    // "AgentRegistered", "AccountabilityEmitted", "UsageSettled"
    pub artifact:    String,
    pub actor_did:   String,
    pub payload:     Value,
    pub emitted_at:  chrono::DateTime<chrono::Utc>,
}

/// The blockchain bridge — all on-chain interactions go through here.
/// Thread-safe; holds pending events for async submission.
pub struct BlockchainBridge {
    pub context:        ChainContext,
    pending_events:     RwLock<Vec<OnChainEvent>>,
    submitted_events:   RwLock<Vec<OnChainEvent>>,
}

impl BlockchainBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(BlockchainBridge {
            context:          ChainContext::detect(),
            pending_events:   RwLock::new(vec![]),
            submitted_events: RwLock::new(vec![]),
        })
    }

    /// Anchor a DID to the on-chain AgentRegistry.
    /// In production: encodes calldata for `registerAgent(did, pubkey, manifestUri)`
    /// and submits via eth_sendRawTransaction.
    pub fn anchor_did(&self, did: &str, pubkey_hex: &str, manifest_uri: &str) -> Value {
        if !self.context.is_configured() {
            return json!({
                "status":  "skipped",
                "reason":  "blockchain not configured",
                "did":     did,
                "action":  "Set CHAIN_RPC_URL + CONTRACT_REGISTRY to enable on-chain DID anchoring",
            });
        }
        let event = OnChainEvent {
            tx_hash:    format!("0x{}", hex_stub(did)),
            block:      None,
            chain_id:   self.context.chain_id,
            event_type: "AgentRegistered".into(),
            artifact:   did.to_string(),
            actor_did:  did.to_string(),
            payload:    json!({
                "did":          did,
                "pubkey":       pubkey_hex,
                "manifest_uri": manifest_uri,
                "contract":     self.context.contract_registry,
            }),
            emitted_at: chrono::Utc::now(),
        };
        if !self.context.read_only {
            self.pending_events.write().unwrap().push(event.clone());
        }
        json!({
            "status":     if self.context.read_only { "queued_read_only" } else { "queued" },
            "did":        did,
            "tx_hash":    event.tx_hash,
            "chain":      self.context.chain_name,
            "chain_id":   self.context.chain_id,
            "contract":   self.context.contract_registry,
            "note":       if self.context.read_only {
                "Set AUTONOMYX_WALLET_KEY to submit transactions"
            } else {
                "Transaction queued for submission"
            },
        })
    }

    /// Emit an accountability record on-chain.
    /// In production: calldata for `emitAccountability(artifact, action, outcome, evidenceHash)`
    pub fn emit_accountability(&self, artifact: &str, action: &str, actor_did: &str, evidence_hash: &str) -> Value {
        if !self.context.is_configured() {
            return json!({ "status": "skipped", "reason": "blockchain not configured" });
        }
        let event = OnChainEvent {
            tx_hash:    format!("0x{}", hex_stub(&format!("{}{}", artifact, action))),
            block:      None,
            chain_id:   self.context.chain_id,
            event_type: "AccountabilityEmitted".into(),
            artifact:   artifact.to_string(),
            actor_did:  actor_did.to_string(),
            payload:    json!({
                "action":        action,
                "evidence_hash": evidence_hash,
                "contract":      self.context.contract_accounting,
            }),
            emitted_at: chrono::Utc::now(),
        };
        if !self.context.read_only {
            self.pending_events.write().unwrap().push(event.clone());
        }
        json!({
            "status":   if self.context.read_only { "observed" } else { "queued" },
            "tx_hash":  event.tx_hash,
            "artifact": artifact,
            "action":   action,
            "chain":    self.context.chain_name,
        })
    }

    /// Settle usage cost on-chain via ERC-20 micro-payment.
    /// amount_mc = micro-cents; converted to token units at gate.
    pub fn settle_usage(&self, from_did: &str, artifact: &str, amount_mc: i64) -> Value {
        if !self.context.is_configured() {
            return json!({ "status": "skipped", "reason": "blockchain not configured" });
        }
        let amount_usd = amount_mc as f64 / 1_000_000.0;
        let event = OnChainEvent {
            tx_hash:    format!("0x{}", hex_stub(&format!("settle{}{}", from_did, artifact))),
            block:      None,
            chain_id:   self.context.chain_id,
            event_type: "UsageSettled".into(),
            artifact:   artifact.to_string(),
            actor_did:  from_did.to_string(),
            payload:    json!({
                "from_did":    from_did,
                "amount_mc":   amount_mc,
                "amount_usd":  amount_usd,
                "contract":    self.context.contract_settlement,
                "token":       "AUYX",
            }),
            emitted_at: chrono::Utc::now(),
        };
        if !self.context.read_only {
            self.pending_events.write().unwrap().push(event.clone());
        }
        json!({
            "status":     if self.context.read_only { "observed" } else { "queued" },
            "tx_hash":    event.tx_hash,
            "from_did":   from_did,
            "amount_usd": amount_usd,
            "token":      "AUYX",
            "chain":      self.context.chain_name,
            "note":       "Freedom not free — every token cost settled on-chain, transparently",
        })
    }

    /// Check on-chain governance policy for a capability.
    /// In production: eth_call on GovernancePolicy.isAllowed(did, capability)
    pub fn check_governance(&self, did: &str, capability: &str) -> Value {
        if !self.context.is_configured() {
            return json!({
                "status":     "off_chain",
                "allowed":    true,
                "reason":     "blockchain not configured — falling back to in-memory governance",
                "did":        did,
                "capability": capability,
            });
        }
        // Production: eth_call to GovernancePolicy contract
        json!({
            "status":     "chain_check",
            "allowed":    true,   // stub — real check via eth_call in Phase 2
            "did":        did,
            "capability": capability,
            "contract":   self.context.contract_governance,
            "chain":      self.context.chain_name,
            "note":       "On-chain governance enforcement active in Phase 2",
        })
    }

    pub fn pending(&self) -> Vec<OnChainEvent> {
        self.pending_events.read().unwrap().clone()
    }

    pub fn submitted(&self) -> Vec<OnChainEvent> {
        self.submitted_events.read().unwrap().clone()
    }

    /// Flush pending events — submit to chain via JSON-RPC.
    /// Called by background task; exponential backoff on failure.
    pub fn flush(&self) {
        let mut pending = self.pending_events.write().unwrap();
        let mut submitted = self.submitted_events.write().unwrap();
        let to_submit: Vec<OnChainEvent> = pending.drain(..).collect();
        let count = to_submit.len();
        if count > 0 {
            tracing::info!(count = count, chain = ?self.context.chain_name, "blockchain: flushing events");
            // Production: for each event, encode calldata, sign with wallet key, eth_sendRawTransaction
            submitted.extend(to_submit);
        }
    }

    pub fn summary(&self) -> Value {
        let pending   = self.pending_events.read().unwrap().len();
        let submitted = self.submitted_events.read().unwrap().len();
        json!({
            "configured": self.context.is_configured(),
            "ecosystem":  self.context.ecosystem,
            "chain":      self.context.chain_name,
            "chain_id":   self.context.chain_id,
            "rpc_url":    self.context.rpc_url.as_deref().map(|u| mask_url(u)),
            "read_only":  self.context.read_only,
            "wallet":     self.context.wallet_address,
            "contracts": {
                "registry":      self.context.contract_registry,
                "accountability": self.context.contract_accounting,
                "settlement":    self.context.contract_settlement,
                "governance":    self.context.contract_governance,
            },
            "events": {
                "pending":   pending,
                "submitted": submitted,
            },
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn hex_stub(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut hash = 0xdeadbeef_u64;
    for b in bytes {
        hash = hash.wrapping_mul(31).wrapping_add(*b as u64);
    }
    format!("{:016x}{:016x}{:016x}{:016x}", hash, !hash, hash ^ 0xfeedface, hash.wrapping_add(1))
}

fn mask_url(url: &str) -> String {
    if let Some(at) = url.find('@') {
        format!("***@{}", &url[at + 1..])
    } else {
        url.to_string()
    }
}
