use axum::{Router, routing::{get, post}, extract::{State, Path}, Json};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/auth/enroll",           post(issue_enrollment))
        .route("/auth/enroll/redeem",    post(redeem_enrollment))
        .route("/auth/rotate",           post(rotate_key))
        .route("/auth/verify",           post(verify_token))
        .route("/auth/revoke/:key_id",   post(revoke_key))
        .route("/auth/peer/:peer_id",    post(issue_peer_key))
        .route("/auth/summary",          get(summary))
        .with_state(state)
}

#[derive(Deserialize)]
struct EnrollReq { for_did: String }

async fn issue_enrollment(
    State(s): State<Arc<AppState>>,
    Json(req): Json<EnrollReq>,
) -> Json<Value> {
    // Only callable with root-level auth (enforced by ingress_gate)
    let (token, key_id) = s.authmatic.issue_enrollment(
        &req.for_did, "platform"
    );
    Json(json!({
        "enrollment_token": token,  // show ONCE — not stored
        "key_id":           key_id,
        "expires_in":       "5 minutes",
        "warning":          "Store this token immediately. It cannot be retrieved again.",
        "next":             "POST /api/auth/enroll/redeem with this token to receive an agent key",
    }))
}

#[derive(Deserialize)]
struct RedeemReq {
    enrollment_token: String,
    agent_did:        String,
    scope:            Option<Vec<String>>,
}

async fn redeem_enrollment(
    State(s): State<Arc<AppState>>,
    Json(req): Json<RedeemReq>,
) -> Json<Value> {
    let scope = req.scope.unwrap_or_else(|| vec![
        "agents:read".into(), "runs:write".into(), "goals:write".into(),
    ]);
    match s.authmatic.enroll(&req.enrollment_token, &req.agent_did, scope) {
        Ok((token, cred)) => Json(json!({
            "agent_token": token,   // show ONCE
            "key_id":      cred.key_id,
            "subject":     cred.subject,
            "scope":       cred.scope,
            "expires_at":  cred.expires_at,
            "warning":     "Store this token securely. It expires in 24h and can be rotated.",
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Deserialize)]
struct RotateReq { token: String }

async fn rotate_key(
    State(s): State<Arc<AppState>>,
    Json(req): Json<RotateReq>,
) -> Json<Value> {
    match s.authmatic.rotate(&req.token) {
        Ok((token, cred)) => Json(json!({
            "new_token":  token,
            "key_id":     cred.key_id,
            "expires_at": cred.expires_at,
            "note":       "Old token is now revoked.",
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Deserialize)]
struct VerifyReq { token: String, scope: Option<String> }

async fn verify_token(
    State(s): State<Arc<AppState>>,
    Json(req): Json<VerifyReq>,
) -> Json<Value> {
    match s.authmatic.verify(&req.token, req.scope.as_deref()) {
        Ok(cred) => Json(json!({
            "valid":   true,
            "key_id":  cred.key_id,
            "subject": cred.subject,
            "kind":    cred.kind,
            "scope":   cred.scope,
            "expires_at": cred.expires_at,
        })),
        Err(e) => Json(json!({ "valid": false, "error": e })),
    }
}

async fn revoke_key(
    State(s): State<Arc<AppState>>,
    Path(key_id): Path<String>,
) -> Json<Value> {
    let ok = s.authmatic.revoke(&key_id);
    Json(json!({ "key_id": key_id, "revoked": ok }))
}

#[derive(Deserialize)]
struct PeerKeyReq { peer_did: String }

async fn issue_peer_key(
    State(s): State<Arc<AppState>>,
    Path(peer_id): Path<String>,
    Json(req): Json<PeerKeyReq>,
) -> Json<Value> {
    let (token, key_id) = s.authmatic.issue_peer_key(&peer_id, &req.peer_did);
    Json(json!({
        "peer_token": token,
        "key_id":     key_id,
        "scope":      ["transfer:push", "aip:message"],
        "expires_in": "72 hours",
    }))
}

async fn summary(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(s.authmatic.summary())
}
