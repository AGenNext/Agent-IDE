// Blockchain routes — Autonomyx on-chain.
//
// Every agent is an NFT. Every accountability record is an on-chain event.
// Every usage cost is a micro-payment. Governance is a smart contract.
// The blockchain is the ultimate source of immutable fact.
//
// GET  /api/chain             — chain context, contract addresses, wallet
// GET  /api/chain/summary     — pending + submitted events, chain state
// POST /api/chain/anchor      — anchor a DID to the AgentRegistry contract
// POST /api/chain/settle      — settle usage cost on-chain (ERC-20)
// POST /api/chain/emit        — emit accountability event on-chain
// GET  /api/chain/governance  — check on-chain governance for a capability
// GET  /api/chain/agent/:did  — resolve agent NFT from chain
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::store::AppState;

#[derive(Deserialize)]
struct AnchorReq {
    did:          String,
    pubkey_hex:   Option<String>,
    manifest_uri: Option<String>,
}

#[derive(Deserialize)]
struct SettleReq {
    from_did:   String,
    artifact:   String,
    amount_mc:  i64,
}

#[derive(Deserialize)]
struct EmitReq {
    artifact:      String,
    action:        String,
    actor_did:     String,
    evidence_hash: Option<String>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/chain",                    get(chain_context))
        .route("/chain/summary",            get(chain_summary))
        .route("/chain/anchor",             post(anchor_did))
        .route("/chain/settle",             post(settle_usage))
        .route("/chain/emit",               post(emit_accountability))
        .route("/chain/governance/:did/:cap", get(check_governance))
        .route("/chain/agent/:did",         get(resolve_agent))
        .with_state(state)
}

async fn chain_context(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(state.blockchain.summary())
}

async fn chain_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pending   = state.blockchain.pending();
    let submitted = state.blockchain.submitted();
    Json(json!({
        "chain":     state.blockchain.summary(),
        "pending":   pending,
        "submitted": submitted.iter().rev().take(20).collect::<Vec<_>>(),
        "philosophy": {
            "past_is_fact":        "On-chain events are immutable. The blockchain enforces what Autonomyx promises.",
            "real_is_fact":        "No retroactive alteration. Every event is final once mined.",
            "accountability":      "Every agent action is an on-chain event. Non-repudiable. Forever.",
            "freedom_not_free":    "Every usage cost settled on-chain. No hidden fees. Transparent to the wei.",
        },
    }))
}

async fn anchor_did(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnchorReq>,
) -> Json<Value> {
    let pubkey      = req.pubkey_hex.as_deref().unwrap_or("0x");
    let default_uri = format!("https://openautonomyx.com/agents/{}", req.did);
    let uri         = req.manifest_uri.as_deref().unwrap_or(&default_uri);
    let result      = state.blockchain.anchor_did(&req.did, pubkey, uri);

    // Record accountability for the on-chain anchor
    let identity = crate::identity::AgentIdentity::from_did(&req.did);
    state.federation.record(
        &identity,
        "chain:anchor_did",
        &req.did,
        None,
        crate::federation::ActionOutcome::Success,
        json!({ "pubkey": pubkey, "manifest_uri": uri }),
    );

    Json(json!({
        "did":     req.did,
        "anchor":  result,
        "federation": "registered off-chain DID document",
    }))
}

async fn settle_usage(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SettleReq>,
) -> Json<Value> {
    let result = state.blockchain.settle_usage(&req.from_did, &req.artifact, req.amount_mc);
    Json(result)
}

async fn emit_accountability(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmitReq>,
) -> Json<Value> {
    let evidence_hash = req.evidence_hash.as_deref().unwrap_or("0x");
    let result = state.blockchain.emit_accountability(
        &req.artifact, &req.action, &req.actor_did, evidence_hash,
    );
    Json(result)
}

async fn check_governance(
    State(state): State<Arc<AppState>>,
    Path((did, cap)): Path<(String, String)>,
) -> Json<Value> {
    let on_chain = state.blockchain.check_governance(&did, &cap);

    // Also check off-chain governance
    use crate::identity::AccessGrant;
    let now = chrono::Utc::now().timestamp() as u64;
    let grant = AccessGrant {
        grant_id:   uuid::Uuid::new_v4().to_string(),
        identity:   did.clone(),
        operation:  cap.clone(),
        resource:   "*".into(),
        issued_at:  now,
        expires_at: now + 60,
        signature:  String::new(),
    };
    let off_chain = match state.federation.check_grant(&grant) {
        Ok(_)  => json!({ "allowed": true,  "reason": "off-chain policy satisfied" }),
        Err(e) => json!({ "allowed": false, "reason": e }),
    };

    Json(json!({
        "did":        did,
        "capability": cap,
        "on_chain":   on_chain,
        "off_chain":  off_chain,
        "verdict":    off_chain["allowed"],
    }))
}

async fn resolve_agent(
    State(state): State<Arc<AppState>>,
    Path(did): Path<String>,
) -> Json<Value> {
    let did_doc   = state.federation.resolve(&did);
    let on_chain  = state.blockchain.check_governance(&did, "agent:identity");
    let lifecycle = state.lifecycle.log_for(&did);
    let usage     = state.usage.records_for(&did);
    let cost_usd: f64 = usage.iter().map(|r| r.total_usd()).sum();

    Json(json!({
        "did":         did,
        "did_document": did_doc,
        "on_chain":    on_chain,
        "lifecycle": {
            "gates_passed": lifecycle.len(),
            "current_stage": state.lifecycle.stage_of(&did).as_ref().map(|s| s.as_str()),
        },
        "cost_usd":    cost_usd,
        "provenance":  "Autonomyx accountability log — append-only, Ed25519-signed, on-chain anchored",
        "nft_note":    "Agent NFT = DID + manifest + accountability. List on any ERC-721 marketplace.",
    }))
}
