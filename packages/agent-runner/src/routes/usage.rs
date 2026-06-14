// Usage routes — fair and transparent pricing.
// Every cost is visible. Nothing hidden. Pay for what you use.
// Freedom, not free: choose any provider, any model, any infra.
// Self-hosted = $0 token cost. Cloud providers = published rates.

use axum::{extract::{Path, State}, routing::get, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

async fn usage_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    let summary  = state.usage.summary();
    let ledgers  = state.usage.all_ledgers();
    Json(json!({
        "pricing_philosophy": {
            "model":       "usage_based",
            "principle":   "freedom_not_free",
            "transparent": true,
            "fair":        "pay exactly what you use — no seat fees, no platform tax",
            "self_hosted": "$0 token cost on Ollama / vllm / local k8s",
            "cloud":       "provider rates pass-through, no markup"
        },
        "usage":   summary,
        "budgets": ledgers,
    }))
}

async fn usage_by_did(
    State(state): State<Arc<AppState>>,
    Path(did): Path<String>,
) -> Json<Value> {
    let records = state.usage.records_for(&did);
    let ledger  = state.usage.ledger_for(&did);
    let total: f64 = records.iter().map(|r| r.total_usd()).sum();
    Json(json!({
        "did":     did,
        "total_usd": total,
        "budget":  ledger,
        "records": records,
    }))
}

async fn usage_all(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!(state.usage.all_records()))
}

async fn pricing_info() -> Json<Value> {
    Json(json!({
        "model": "usage_based",
        "philosophy": "Freedom, not free. Fair and transparent.",
        "published_rates": {
            "anthropic": {
                "claude-opus-4-8": { "input_per_1m_usd": 5.00, "output_per_1m_usd": 25.00 }
            },
            "openai": {
                "gpt-4o": { "input_per_1m_usd": 5.00, "output_per_1m_usd": 15.00 }
            },
            "ollama_local": {
                "any_model": { "input_per_1m_usd": 0.00, "output_per_1m_usd": 0.00,
                               "note": "self-hosted — your compute, your cost, no token fees" }
            }
        },
        "what_we_dont_charge": [
            "seat_licenses", "platform_tax", "api_call_surcharges",
            "egress_from_self_hosted", "gate_transitions", "fabric_events"
        ],
        "what_you_pay": [
            "provider_tokens_at_published_rates",
            "managed_cloud_compute_if_using_openautonomyx_com",
            "support_tier_if_above_community"
        ],
        "visibility": {
            "before_run":  "estimated cost shown before gate opens",
            "during_run":  "running total in /usage/:did",
            "after_run":   "full breakdown in usage record + gate audit log",
            "in_bom":      "cost recorded in artifact BOM for full provenance"
        }
    }))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/usage",           get(usage_summary).with_state(state.clone()))
        .route("/usage/all",       get(usage_all).with_state(state.clone()))
        .route("/usage/pricing",   get(pricing_info))
        .route("/usage/:did",      get(usage_by_did).with_state(state))
}
