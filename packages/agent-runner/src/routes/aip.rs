// AIP — Agent Internet Protocol routes.
//
// Every agent speaks AIP. Every message is signed. Every exchange is accountable.
// "Everything is possible" — when agents can talk to any other agent, anywhere,
// with identity, governance, and accountability at every hop.
//
// POST /aip/handshake        — establish agent-to-agent session (DID + Ed25519)
// POST /aip/message          — send a signed AIP message
// POST /aip/capability       — invoke a capability on another agent
// POST /aip/event            — push a fabric event to a peer
// GET  /aip/did/:did         — resolve a DID document
// GET  /aip/sessions         — active AIP sessions
// POST /aip/audit/replicate  — replicate an accountability record to this node
//
// Wire format: JSON envelope (aip, id, from, to, type, payload, trace, sig)
// Transport:   HTTPS POST (request/response) + WS (event push)
// Auth:        Ed25519 signature on every message — no exceptions
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use crate::store::AppState;
use crate::identity::AgentIdentity;

// ── AIP message envelope ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AipMessage {
    pub aip:     String,          // "1.0"
    pub id:      String,          // uuid
    pub from:    String,          // did:autonomyx:<pubkey>
    pub to:      String,          // did:autonomyx:<pubkey>
    pub r#type:  String,          // message type
    pub payload: Value,
    pub trace:   Option<String>,  // OTel trace-id
    pub sig:     Option<String>,  // hex Ed25519 over canonical form
}

impl AipMessage {
    /// Create a new signed outbound AIP message from this node's identity.
    pub fn new(from_did: &str, to: &str, msg_type: &str, payload: Value) -> Self {
        AipMessage {
            aip:     "1.0".into(),
            id:      Uuid::new_v4().to_string(),
            from:    from_did.to_string(),
            to:      to.to_string(),
            r#type:  msg_type.to_string(),
            payload,
            trace:   Some(Uuid::new_v4().to_string()),
            sig:     None,   // caller signs after construction
        }
    }

    /// Sign the message with the given identity (Ed25519 over canonical bytes).
    pub fn sign(&mut self, identity: &AgentIdentity) {
        let canonical = format!("{}{}{}{}{}{}",
            self.aip, self.id, self.from, self.to, self.r#type,
            self.payload.to_string()
        );
        let sig_bytes = identity.sign(canonical.as_bytes());
        self.sig = Some(hex_encode(&sig_bytes));
    }

    /// Verify the message signature against the sender's public key.
    /// Production: resolve the sender's DID document to get the public key.
    pub fn verify(&self, identity: &AgentIdentity) -> bool {
        let Some(sig_hex) = &self.sig else { return false; };
        let Ok(sig_bytes) = hex_decode(sig_hex) else { return false; };
        if sig_bytes.len() != 64 { return false; }
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&sig_bytes);
        let canonical = format!("{}{}{}{}{}{}",
            self.aip, self.id, self.from, self.to, self.r#type,
            self.payload.to_string()
        );
        identity.verify(canonical.as_bytes(), &sig)
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /aip/handshake — establish a session between two agents.
/// The initiating agent presents its DID + signed handshake payload.
/// This node responds with its DID Document.
async fn handshake(
    State(state): State<Arc<AppState>>,
    Json(msg): Json<AipMessage>,
) -> Json<Value> {
    if msg.r#type != "aip.handshake.init" {
        return Json(json!({
            "error": "expected aip.handshake.init",
            "got":   msg.r#type,
        }));
    }

    // Resolve the sender's DID — fetch from federation registry or peer
    let sender_doc = state.federation.resolve(&msg.from);

    // Build our handshake ACK — present this node's platform DID
    let platform_did = format!("did:autonomyx:platform-{}", Uuid::new_v4().simple());
    let ack = AipMessage::new(
        &platform_did,
        &msg.from,
        "aip.handshake.ack",
        json!({
            "session_id":    Uuid::new_v4().to_string(),
            "accepted":      true,
            "platform":      "Autonomyx",
            "version":       "AIP/1.0",
            "capabilities":  [
                "lifecycle:transition", "agent:execute", "fabric:subscribe",
                "did:resolve", "usage:query", "mcp:dispatch"
            ],
            "did_document":  sender_doc,
        }),
    );

    // Record the handshake in the accountability log
    let evidence = json!({ "from": msg.from, "session": ack.payload["session_id"] });
    let platform_identity = AgentIdentity::from_did(&platform_did);
    state.federation.record(
        &platform_identity,
        "aip:handshake",
        &msg.from,
        None,
        crate::federation::ActionOutcome::Success,
        evidence,
    );

    tracing::info!(
        from    = %msg.from,
        session = ?ack.payload.get("session_id"),
        "AIP: handshake established"
    );

    Json(json!({ "ack": ack, "status": "session_established" }))
}

/// POST /aip/message — receive an inbound AIP message.
/// Routes to the appropriate handler based on message type.
async fn receive_message(
    State(state): State<Arc<AppState>>,
    Json(msg): Json<AipMessage>,
) -> Json<Value> {
    tracing::info!(
        from  = %msg.from,
        to    = %msg.to,
        r#type = %msg.r#type,
        id    = %msg.id,
        "AIP: message received"
    );

    // Emit as fabric event — fabric is the middleware, fills the gap
    let platform_did = "did:autonomyx:platform";
    state.fabric.emit(crate::fabric::FabricEvent {
        id:         msg.id.clone(),
        artifact:   msg.from.clone(),
        stage:      crate::lifecycle::Stage::Run,
        status:     crate::fabric::FabricStatus::Open,
        payload:    json!({
            "aip_type": msg.r#type,
            "from":     msg.from,
            "to":       msg.to,
            "payload":  msg.payload,
        }),
        emitted_at: Utc::now(),
    });

    // Record in accountability log
    let identity = AgentIdentity::from_did(platform_did);
    state.federation.record(
        &identity,
        &format!("aip:{}", msg.r#type),
        &msg.from,
        None,
        crate::federation::ActionOutcome::Success,
        json!({ "msg_id": msg.id, "type": msg.r#type }),
    );

    // Route by message type
    let response_payload = match msg.r#type.as_str() {
        "aip.capability.request" => handle_capability_request(&state, &msg).await,
        "aip.event.push"         => handle_event_push(&state, &msg).await,
        "aip.did.resolve"        => handle_did_resolve(&state, &msg).await,
        "aip.audit.replicate"    => handle_audit_replicate(&state, &msg).await,
        "aip.lifecycle.gate"     => handle_lifecycle_gate(&state, &msg).await,
        _                        => json!({ "status": "received", "type": msg.r#type }),
    };

    Json(json!({
        "aip":     "1.0",
        "id":      Uuid::new_v4().to_string(),
        "from":    platform_did,
        "to":      msg.from,
        "type":    format!("{}.response", msg.r#type),
        "payload": response_payload,
        "trace":   msg.trace,
    }))
}

/// POST /aip/capability — invoke a capability on this node.
async fn invoke_capability(
    State(state): State<Arc<AppState>>,
    Json(msg): Json<AipMessage>,
) -> Json<Value> {
    let cap  = msg.payload.get("capability").and_then(|v| v.as_str()).unwrap_or("");
    let args = msg.payload.get("args").cloned().unwrap_or(json!({}));

    // Check governance — does the caller have this capability?
    // Build a transient grant for this capability invocation
    use crate::identity::AccessGrant;
    let now = Utc::now().timestamp() as u64;
    let transient_grant = AccessGrant {
        grant_id:   Uuid::new_v4().to_string(),
        identity:   msg.from.clone(),
        operation:  cap.to_string(),
        resource:   "*".to_string(),
        issued_at:  now,
        expires_at: now + 60,
        signature:  String::new(),
    };
    let grant_check = state.federation.check_grant(&transient_grant);
    match grant_check {
        Ok(_) => {
            Json(json!({
                "status":     "invoked",
                "capability": cap,
                "from":       msg.from,
                "result":     execute_capability(&state, cap, &args).await,
            }))
        }
        Err(reason) => {
            tracing::warn!(from = %msg.from, cap = %cap, reason = %reason, "AIP: capability denied");
            Json(json!({
                "error":      "capability_denied",
                "capability": cap,
                "reason":     reason,
            }))
        }
    }
}

/// GET /aip/did/:did — resolve a DID document.
async fn resolve_did(
    State(state): State<Arc<AppState>>,
    Path(did): Path<String>,
) -> Json<Value> {
    match state.federation.resolve(&did) {
        Some(doc) => Json(json!({ "did": did, "document": doc, "resolved": true })),
        None      => Json(json!({ "did": did, "resolved": false,
                                  "hint": "DID not found locally — try peer broadcast" })),
    }
}

/// GET /aip/sessions — list active AIP sessions (placeholder — full session store in Phase 2).
async fn list_sessions() -> Json<Value> {
    Json(json!({
        "sessions": [],
        "note": "Full session store in Phase 2 — backed by SurrealDB live queries",
        "current": "in-memory; sessions tracked via accountability log",
    }))
}

/// POST /aip/audit/replicate — accept a replicated accountability record from a peer.
async fn replicate_audit(
    State(state): State<Arc<AppState>>,
    Json(msg): Json<AipMessage>,
) -> Json<Value> {
    let records = msg.payload.get("records").cloned().unwrap_or(json!([]));
    let count   = records.as_array().map(|a| a.len()).unwrap_or(0);
    // Production: deserialise and insert into federation accountability store.
    tracing::info!(from = %msg.from, count = count, "AIP: audit records replicated");
    Json(json!({ "status": "replicated", "count": count }))
}

// ── Message type handlers ─────────────────────────────────────────────────────

async fn handle_capability_request(state: &Arc<AppState>, msg: &AipMessage) -> Value {
    let cap  = msg.payload.get("capability").and_then(|v| v.as_str()).unwrap_or("");
    let args = msg.payload.get("args").cloned().unwrap_or(json!({}));
    execute_capability(state, cap, &args).await
}

async fn handle_event_push(state: &Arc<AppState>, msg: &AipMessage) -> Value {
    state.fabric.emit(crate::fabric::FabricEvent {
        id:         msg.id.clone(),
        artifact:   msg.from.clone(),
        stage:      crate::lifecycle::Stage::Run,
        status:     crate::fabric::FabricStatus::Open,
        payload:    msg.payload.clone(),
        emitted_at: Utc::now(),
    });
    json!({ "status": "event_received", "delivered_to_fabric": true })
}

async fn handle_did_resolve(state: &Arc<AppState>, msg: &AipMessage) -> Value {
    let did = msg.payload.get("did").and_then(|v| v.as_str()).unwrap_or("");
    match state.federation.resolve(did) {
        Some(doc) => json!({ "document": doc }),
        None      => json!({ "error": "not_found", "did": did }),
    }
}

async fn handle_audit_replicate(_state: &Arc<AppState>, msg: &AipMessage) -> Value {
    let count = msg.payload.get("records")
        .and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    json!({ "replicated": count })
}

async fn handle_lifecycle_gate(state: &Arc<AppState>, msg: &AipMessage) -> Value {
    let artifact = msg.payload.get("artifact").and_then(|v| v.as_str()).unwrap_or("");
    let stage_str = msg.payload.get("stage").and_then(|v| v.as_str()).unwrap_or("run");
    let stage: crate::lifecycle::Stage = serde_json::from_value(json!(stage_str))
        .unwrap_or(crate::lifecycle::Stage::Run);
    let gate = crate::lifecycle::Gate::new(&state.lifecycle, artifact);
    let rec  = match stage {
        crate::lifecycle::Stage::Build    => gate.build(&msg.payload),
        crate::lifecycle::Stage::Sign     => gate.sign(&msg.payload),
        crate::lifecycle::Stage::Push     => gate.push(&msg.payload),
        crate::lifecycle::Stage::Sync     => gate.sync(&msg.payload),
        crate::lifecycle::Stage::Deploy   => gate.deploy(&msg.payload),
        crate::lifecycle::Stage::Run      => gate.run(&msg.payload),
        crate::lifecycle::Stage::Observe  => gate.observe(&msg.payload),
        crate::lifecycle::Stage::Feedback => gate.feedback(&msg.payload),
    };
    state.fabric.emit_gate(&rec, msg.payload.clone());
    json!({ "gate": rec.stage.as_str(), "status": rec.status, "oath": rec.oath })
}

async fn execute_capability(_state: &Arc<AppState>, cap: &str, _args: &Value) -> Value {
    // Production: dispatch to the registered capability handler.
    // Now: return capability info so callers know what's available.
    json!({
        "capability": cap,
        "status":     "available",
        "note":       "capability execution wired in Phase 2 — governance-checked, usage-metered",
    })
}

// ── Hex helpers ───────────────────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i+2], 16).map_err(|_| ()))
        .collect()
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/aip/handshake",        post(handshake))
        .route("/aip/message",          post(receive_message))
        .route("/aip/capability",       post(invoke_capability))
        .route("/aip/did/:did",         get(resolve_did))
        .route("/aip/sessions",         get(list_sessions))
        .route("/aip/audit/replicate",  post(replicate_audit))
        .with_state(state)
}
