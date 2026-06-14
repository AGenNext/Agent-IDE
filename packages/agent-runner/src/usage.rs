// Autonomyx Usage — metered at the gate, billed by what you use.
//
// Usage is recorded at every gate transition.
// The gate is the billing unit — not the seat, not the month, not the tier.
//
// What is metered:
//   tokens_in      — input tokens consumed by LLM calls in this stage
//   tokens_out     — output tokens produced
//   compute_ms     — wall-clock time the gate was executing
//   storage_bytes  — registry push/pull, BOM storage, config writes
//   egress_bytes   — peer transfer, fabric events, AIP messages
//
// What is billed:
//   token_cost     = tokens_in × input_price + tokens_out × output_price
//   compute_cost   = compute_ms × compute_price_per_ms
//   storage_cost   = storage_bytes × storage_price_per_gb
//   egress_cost    = egress_bytes × egress_price_per_gb
//   total_cost     = sum of above
//
// Budget gate:
//   Before opening any stage, check: remaining_budget >= estimated_cost
//   If not: gate closes with GateStatus::Closed (reason: budget_exceeded)
//   The fabric routes the dead-letter to the FinOps channel
//
// Self-hosted (Ollama, k3s, bare metal):
//   token_cost = 0   (no provider API call)
//   compute_cost = 0 (your own hardware)
//   total_cost = 0
//   Usage still tracked — capacity planning, not billing

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::lifecycle::Stage;

// ── Usage record — emitted at every gate ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id:            String,
    pub did:           String,        // agent DID (who consumed)
    pub artifact:      String,        // what artifact this run is for
    pub stage:         Stage,
    pub provider:      String,        // "anthropic" | "openai" | "ollama" | "self"
    pub model:         String,        // "claude-opus-4-8" | "llama3" | ...

    // Units consumed
    pub tokens_in:     u64,
    pub tokens_out:    u64,
    pub compute_ms:    u64,
    pub storage_bytes: u64,
    pub egress_bytes:  u64,

    // Costs (in USD micro-cents: 1_000_000 = $1.00)
    pub token_cost_usd_mc:   i64,
    pub compute_cost_usd_mc: i64,
    pub storage_cost_usd_mc: i64,
    pub egress_cost_usd_mc:  i64,
    pub total_cost_usd_mc:   i64,

    pub recorded_at:   DateTime<Utc>,
    pub run_id:        Option<String>,
    pub grant_id:      Option<String>,
}

impl UsageRecord {
    /// Compute costs from raw units using a pricing config.
    pub fn with_pricing(mut self, pricing: &ProviderPricing) -> Self {
        // token costs in micro-cents (avoid float)
        self.token_cost_usd_mc = (
            (self.tokens_in  as i64 * pricing.input_per_1m_usd_mc)  / 1_000_000
          + (self.tokens_out as i64 * pricing.output_per_1m_usd_mc) / 1_000_000
        );
        self.compute_cost_usd_mc = self.compute_ms as i64 * pricing.compute_per_ms_usd_mc;
        self.storage_cost_usd_mc = (self.storage_bytes as i64 * pricing.storage_per_gb_usd_mc) / 1_073_741_824;
        self.egress_cost_usd_mc  = (self.egress_bytes  as i64 * pricing.egress_per_gb_usd_mc)  / 1_073_741_824;
        self.total_cost_usd_mc   = self.token_cost_usd_mc
                                  + self.compute_cost_usd_mc
                                  + self.storage_cost_usd_mc
                                  + self.egress_cost_usd_mc;
        self
    }

    pub fn total_usd(&self) -> f64 {
        self.total_cost_usd_mc as f64 / 1_000_000.0
    }
}

// ── Provider pricing config ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    pub provider:               String,
    pub model:                  String,
    pub input_per_1m_usd_mc:    i64,   // micro-cents per 1M input tokens
    pub output_per_1m_usd_mc:   i64,   // micro-cents per 1M output tokens
    pub compute_per_ms_usd_mc:  i64,   // micro-cents per ms of compute
    pub storage_per_gb_usd_mc:  i64,   // micro-cents per GB stored
    pub egress_per_gb_usd_mc:   i64,   // micro-cents per GB egressed
    pub self_hosted:            bool,  // if true, all costs = 0
}

impl ProviderPricing {
    /// Zero-cost pricing for self-hosted models (Ollama, vllm, local k8s).
    pub fn self_hosted(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.into(), model: model.into(),
            input_per_1m_usd_mc: 0, output_per_1m_usd_mc: 0,
            compute_per_ms_usd_mc: 0, storage_per_gb_usd_mc: 0,
            egress_per_gb_usd_mc: 0, self_hosted: true,
        }
    }

    /// Anthropic Claude Opus 4.8: $5/1M in, $25/1M out.
    pub fn anthropic_opus_4_8() -> Self {
        Self {
            provider: "anthropic".into(), model: "claude-opus-4-8".into(),
            input_per_1m_usd_mc:   5_000_000,  // $5.00 / 1M = 5_000_000 µ¢
            output_per_1m_usd_mc:  25_000_000, // $25.00 / 1M
            compute_per_ms_usd_mc: 0,
            storage_per_gb_usd_mc: 0,
            egress_per_gb_usd_mc:  0,
            self_hosted: false,
        }
    }

    /// OpenAI GPT-4o: approximate pricing.
    pub fn openai_gpt4o() -> Self {
        Self {
            provider: "openai".into(), model: "gpt-4o".into(),
            input_per_1m_usd_mc:   5_000_000,
            output_per_1m_usd_mc:  15_000_000,
            compute_per_ms_usd_mc: 0,
            storage_per_gb_usd_mc: 0,
            egress_per_gb_usd_mc:  0,
            self_hosted: false,
        }
    }

    pub fn ollama_local() -> Self {
        Self::self_hosted("ollama", "llama3")
    }
}

// ── Budget ledger — per DID ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetLedger {
    pub did:                String,
    pub budget_usd_mc:      i64,    // total budget in micro-cents
    pub consumed_usd_mc:    i64,    // consumed so far
    pub remaining_usd_mc:   i64,    // budget - consumed
    pub period:             String, // "monthly" | "per_run" | "unlimited"
    pub on_exceed:          ExceedAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceedAction {
    Reject,
    Warn,
    Downgrade,
}

impl BudgetLedger {
    pub fn new(did: &str, budget_usd: f64, period: &str, on_exceed: ExceedAction) -> Self {
        let budget_usd_mc = (budget_usd * 1_000_000.0) as i64;
        Self {
            did: did.into(), budget_usd_mc,
            consumed_usd_mc: 0,
            remaining_usd_mc: budget_usd_mc,
            period: period.into(), on_exceed,
        }
    }

    pub fn can_spend(&self, cost_usd_mc: i64) -> bool {
        self.budget_usd_mc == 0 || self.remaining_usd_mc >= cost_usd_mc
    }

    pub fn charge(&mut self, cost_usd_mc: i64) {
        self.consumed_usd_mc  += cost_usd_mc;
        self.remaining_usd_mc -= cost_usd_mc;
    }

    pub fn consumed_usd(&self) -> f64 { self.consumed_usd_mc  as f64 / 1_000_000.0 }
    pub fn remaining_usd(&self) -> f64 { self.remaining_usd_mc as f64 / 1_000_000.0 }
}

// ── Usage meter — the runtime billing engine ──────────────────────────────────

struct MeterState {
    records:  Vec<UsageRecord>,
    ledgers:  HashMap<String, BudgetLedger>,
    pricing:  HashMap<String, ProviderPricing>, // "provider:model" → pricing
}

#[derive(Clone)]
pub struct UsageMeter {
    inner: Arc<RwLock<MeterState>>,
}

impl UsageMeter {
    pub fn new() -> Self {
        let mut pricing = HashMap::new();
        // Seed built-in pricing
        for p in [
            ProviderPricing::anthropic_opus_4_8(),
            ProviderPricing::openai_gpt4o(),
            ProviderPricing::ollama_local(),
        ] {
            pricing.insert(format!("{}:{}", p.provider, p.model), p);
        }
        Self { inner: Arc::new(RwLock::new(MeterState {
            records: Vec::new(), ledgers: HashMap::new(), pricing,
        })) }
    }

    /// Register a budget for a DID.
    pub fn set_budget(&self, did: &str, budget_usd: f64, period: &str, on_exceed: ExceedAction) {
        self.inner.write().unwrap()
            .ledgers.insert(did.into(), BudgetLedger::new(did, budget_usd, period, on_exceed));
    }

    /// Check if a DID can spend the estimated cost. Returns Err if budget exceeded.
    pub fn check_budget(&self, did: &str, estimated_cost_usd_mc: i64) -> Result<(), String> {
        let state = self.inner.read().unwrap();
        match state.ledgers.get(did) {
            None => Ok(()), // no budget = unlimited
            Some(ledger) => {
                if ledger.can_spend(estimated_cost_usd_mc) { Ok(()) }
                else {
                    Err(format!(
                        "budget exceeded for {did}: remaining ${:.4} < estimated ${:.4}",
                        ledger.remaining_usd(), estimated_cost_usd_mc as f64 / 1_000_000.0
                    ))
                }
            }
        }
    }

    /// Record usage at a gate transition.
    pub fn record(&self, mut rec: UsageRecord) -> UsageRecord {
        let key = format!("{}:{}", rec.provider, rec.model);
        let pricing = {
            let state = self.inner.read().unwrap();
            state.pricing.get(&key).cloned()
                .unwrap_or_else(|| ProviderPricing::self_hosted(&rec.provider, &rec.model))
        };
        rec = rec.with_pricing(&pricing);

        let mut state = self.inner.write().unwrap();

        // Charge the DID's budget
        if let Some(ledger) = state.ledgers.get_mut(&rec.did) {
            ledger.charge(rec.total_cost_usd_mc);
            tracing::debug!(
                did    = %rec.did,
                stage  = ?rec.stage,
                cost   = rec.total_usd(),
                remaining = ledger.remaining_usd(),
                "usage: charged"
            );
        }

        tracing::info!(
            did      = %rec.did,
            stage    = ?rec.stage,
            tokens   = rec.tokens_in + rec.tokens_out,
            cost_usd = rec.total_usd(),
            "usage: recorded"
        );

        state.records.push(rec.clone());
        rec
    }

    /// New usage record builder.
    pub fn new_record(did: &str, artifact: &str, stage: Stage, provider: &str, model: &str) -> UsageRecord {
        UsageRecord {
            id: Uuid::new_v4().to_string(),
            did: did.into(), artifact: artifact.into(), stage,
            provider: provider.into(), model: model.into(),
            tokens_in: 0, tokens_out: 0, compute_ms: 0,
            storage_bytes: 0, egress_bytes: 0,
            token_cost_usd_mc: 0, compute_cost_usd_mc: 0,
            storage_cost_usd_mc: 0, egress_cost_usd_mc: 0,
            total_cost_usd_mc: 0,
            recorded_at: Utc::now(),
            run_id: None, grant_id: None,
        }
    }

    // ── Query ─────────────────────────────────────────────────────────────────

    pub fn records_for(&self, did: &str) -> Vec<UsageRecord> {
        self.inner.read().unwrap().records.iter()
            .filter(|r| r.did == did).cloned().collect()
    }

    pub fn all_records(&self) -> Vec<UsageRecord> {
        self.inner.read().unwrap().records.clone()
    }

    pub fn ledger_for(&self, did: &str) -> Option<BudgetLedger> {
        self.inner.read().unwrap().ledgers.get(did).cloned()
    }

    pub fn all_ledgers(&self) -> Vec<BudgetLedger> {
        self.inner.read().unwrap().ledgers.values().cloned().collect()
    }

    /// Summary: total spend, total tokens, by provider, by stage.
    pub fn summary(&self) -> serde_json::Value {
        let state = self.inner.read().unwrap();
        let total_usd_mc: i64  = state.records.iter().map(|r| r.total_cost_usd_mc).sum();
        let total_tokens: u64  = state.records.iter().map(|r| r.tokens_in + r.tokens_out).sum();

        let mut by_provider: HashMap<String, i64> = HashMap::new();
        let mut by_stage: HashMap<String, i64>    = HashMap::new();
        let mut by_did: HashMap<String, i64>      = HashMap::new();

        for r in &state.records {
            *by_provider.entry(r.provider.clone()).or_default() += r.total_cost_usd_mc;
            *by_stage.entry(r.stage.as_str().to_string()).or_default() += r.total_cost_usd_mc;
            *by_did.entry(r.did.clone()).or_default()           += r.total_cost_usd_mc;
        }

        serde_json::json!({
            "total_usd":    total_usd_mc as f64 / 1_000_000.0,
            "total_tokens": total_tokens,
            "record_count": state.records.len(),
            "by_provider":  by_provider.into_iter().map(|(k,v)| (k, v as f64 / 1_000_000.0)).collect::<HashMap<_,_>>(),
            "by_stage":     by_stage.into_iter().map(|(k,v)| (k, v as f64 / 1_000_000.0)).collect::<HashMap<_,_>>(),
            "by_did":       by_did.into_iter().map(|(k,v)| (k, v as f64 / 1_000_000.0)).collect::<HashMap<_,_>>(),
        })
    }
}

impl Default for UsageMeter { fn default() -> Self { Self::new() } }
