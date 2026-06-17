// Platform route — the platform makes things real.
// GET /api/platform     — platform identity, axioms, cloud context, capabilities
// GET /api/multiserver  — peer bridge status

use axum::{routing::get, Json, Router, extract::State};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::cloud::PlatformIdentity;
use crate::AppState;

async fn platform_identity() -> Json<Value> {
    let identity = PlatformIdentity::new();
    Json(json!({
        "platform": {
            "name":        identity.name,
            "version":     identity.version,
            "did_method":  identity.did_method,
            "protocol":    identity.protocol,
            "homepage":    identity.homepage,
            "philosophy":  identity.philosophy,
        },
        "runtime": {
            "provider":      identity.cloud.provider,
            "region":        identity.cloud.region,
            "zone":          identity.cloud.zone,
            "instance_type": identity.cloud.instance_type,
            "node":          identity.cloud.node_name,
            "cluster":       identity.cloud.cluster,
            "namespace":     identity.cloud.namespace,
        },
        "capabilities": identity.capabilities,
        "ecosystems":   identity.ecosystems,
        "gates": ["build", "sign", "push", "sync", "deploy", "run", "observe", "feedback"],
        "agent_properties": {
            "real":         "hardware-backed identity (HSM/TPM/DID)",
            "unique":       "did:autonomyx:<pubkey> — cannot be cloned",
            "identifiable": "globally addressable via AIP",
            "governed":     "GovernancePolicy in DID Document — JIT access",
            "autonomous":   "self-directed within governance bounds",
            "federal":      "local-first DID registry — no central authority",
            "accountable":  "Ed25519-signed accountability log — non-repudiable",
            "intelligent":  "any LLM provider — OpenAI, Anthropic, Ollama, any OpenAI-compatible",
        },
        // The axioms live here — one world, one theory, declared at the platform root.
        // Verified academic alignment lives at GET /api/theory
        "axioms": {
            "1": "Every entity — person, device, service, sensor, model, process — is an agent",
            "2": "Every agent has a unique DID. Cannot be cloned. Cannot be forged.",
            "3": "Every action is signed. Non-repudiable. Traceable to origin.",
            "4": "Every gate is idempotent. Same input → same output. Safe at infinite scale.",
            "5": "Every provider is replaceable. No lock-in at any layer.",
            "6": "Every event flows through the fabric. No polling. No gaps.",
            "7": "Governance is per-DID. A million agents → a million independent policies.",
            "8": "Supply chain risk is zero. BOM at every gate. Signed. Verified.",
        },
        "world_model": {
            "scope":    "multi-ecosystem",
            "space":    "single DID namespace",
            "protocol": "single AIP wire protocol",
            "lifecycle":"single 8-gate lifecycle",
            "scale":    "infinite — k8s + federation + idempotency",
        },
        "makes_real": "declare it in .ayx → the platform instantiates it — real agent, real DID, real gates, real fabric"
    }))
}

async fn multiserver_status(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(crate::multiserver::bridge_summary(&s))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/platform",    get(platform_identity))
        .route("/multiserver", get(multiserver_status))
        .with_state(state)
}
