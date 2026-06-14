use axum::{Router, routing::{get, post}, extract::{State, Path}, Json};
use std::sync::Arc;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::optin::ExtensionDeclaration;
use crate::fabric::{FabricEvent, FabricStatus};
use crate::lifecycle::Stage;
use crate::govgraph::{GovernanceNode, NodeKind, NodePolicy};
use crate::plugin::{PluginDescriptor, PluginKind, PluginNode};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/optin",                  get(list_optins).post(submit_align))
        .route("/optin/summary",          get(summary))
        .route("/optin/extend",           post(submit_extend))
        .route("/optin/align",            post(submit_align_explicit))
        .route("/optin/:id",              get(get_optin))
        .route("/optin/:id/activate",     post(activate))
        .route("/optin/:id/withdraw",     post(withdraw))
        .with_state(state)
}

async fn list_optins(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "optins": s.optin.list() }))
}

async fn summary(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(s.optin.summary())
}

async fn get_optin(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match s.optin.get(&id) {
        Some(r) => Json(json!({ "optin": r })),
        None    => Json(json!({ "error": "not found", "id": id })),
    }
}

// ── Extend ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ExtendReq {
    actor_did:    Option<String>,
    name:         String,
    version:      Option<String>,
    description:  String,
    author:       Option<String>,
    capabilities: Vec<String>,
    config_keys:  Option<Vec<String>>,
    node_kind:    Option<String>,
    edges_to:     Option<Vec<String>>,
    homepage:     Option<String>,
}

async fn submit_extend(
    State(s): State<Arc<AppState>>,
    Json(req): Json<ExtendReq>,
) -> Json<Value> {
    let actor = req.actor_did.as_deref().unwrap_or("did:autonomyx:anonymous");
    let ext = ExtensionDeclaration {
        name:         req.name.clone(),
        version:      req.version.unwrap_or_else(|| "1.0".into()),
        description:  req.description.clone(),
        author:       req.author.unwrap_or_else(|| actor.to_string()),
        capabilities: req.capabilities.clone(),
        config_keys:  req.config_keys.unwrap_or_default(),
        node_kind:    req.node_kind.clone().unwrap_or_else(|| "tool".into()),
        edges_to:     req.edges_to.clone().unwrap_or_default(),
        homepage:     req.homepage.clone(),
    };

    let record = s.optin.submit_extend(actor, ext.clone());

    // If approved — wire into govgraph + plugin registry immediately
    if record.status == crate::optin::OptInStatus::Approved {
        // Wire governance node
        let kind = match ext.node_kind.as_str() {
            "source"  => NodeKind::Source,
            "sink"    => NodeKind::Sink,
            "api"     => NodeKind::Api,
            "agent"   => NodeKind::Agent,
            _         => NodeKind::Tool,
        };
        let node = GovernanceNode {
            id:           format!("ext:{}", record.id),
            kind,
            label:        ext.name.clone(),
            description:  ext.description.clone(),
            did:          Some(actor.to_string()),
            capabilities: ext.capabilities.clone(),
            requires:     ext.capabilities.first().map(|c| vec![c.clone()]).unwrap_or_default(),
            trust_score:  0.7,
            policy:       NodePolicy::default(),
            metadata:     json!({ "optin_id": record.id }),
            created_at:   chrono::Utc::now(),
            updated_at:   chrono::Utc::now(),
        };
        s.govgraph.add_node(node.clone());

        // Wire edges declared by the extension
        let cap = ext.capabilities.first().cloned().unwrap_or_default();
        for target in &ext.edges_to {
            let _ = s.govgraph.add_edge(
                &node.id,
                target,
                &cap,
                &format!("ext:{}", ext.name),
                crate::govgraph::EdgeCondition::default(),
                1.0,
            );
        }

        // Register as plugin
        let plugin_node = PluginNode {
            id:                   format!("ext:{}", record.id),
            label:                ext.name.clone(),
            kind:                 ext.node_kind.clone(),
            capabilities:         ext.capabilities.clone(),
            edges_to:             ext.edges_to.clone(),
            capability_required:  ext.capabilities.first().cloned().unwrap_or_default(),
        };
        let plugin_kind = match ext.node_kind.as_str() {
            "source" | "sink" => PluginKind::StorageBackend,
            "api"             => PluginKind::Connector,
            _                 => PluginKind::Tool,
        };
        s.plugins.register(PluginDescriptor {
            id:           format!("ext:{}", record.id),
            name:         ext.name.clone(),
            version:      ext.version.clone(),
            description:  ext.description.clone(),
            author:       ext.author.clone(),
            kind:         plugin_kind,
            capabilities: ext.capabilities.clone(),
            config_keys:  ext.config_keys.clone(),
            enabled:      true,
            loaded:       true,
            homepage:     ext.homepage.clone(),
            nodes:        vec![plugin_node],
            data_sources: vec![],
        });

        // Emit fabric event
        s.fabric.emit(FabricEvent::open(
            &record.id,
            Stage::Run,
            json!({ "event": "optin_extend_approved", "name": ext.name, "actor": actor }),
        ));
    }

    Json(json!({ "optin": record, "wired": record.status == crate::optin::OptInStatus::Approved }))
}

// ── Align ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AlignReq {
    actor_did:       Option<String>,
    name:            String,
    description:     String,
    intended_impact: Option<String>,
}

async fn submit_align(
    State(s): State<Arc<AppState>>,
    Json(req): Json<AlignReq>,
) -> Json<Value> {
    run_align(s, req).await
}

async fn submit_align_explicit(
    State(s): State<Arc<AppState>>,
    Json(req): Json<AlignReq>,
) -> Json<Value> {
    run_align(s, req).await
}

async fn run_align(s: Arc<AppState>, req: AlignReq) -> Json<Value> {
    let actor  = req.actor_did.as_deref().unwrap_or("did:autonomyx:anonymous");
    let impact = req.intended_impact.as_deref().unwrap_or(&req.description);

    let record = s.optin.submit_align(actor, &req.name, &req.description, impact);

    // Emit fabric event
    let status = if record.alignment.as_ref().map(|a| a.passes).unwrap_or(false) {
        // Also activate so trust score rises immediately
        s.optin.activate(&record.id);
        s.govgraph.update_trust("agent:plan", true);
        s.fabric.emit(FabricEvent::open(
            &record.id,
            Stage::Run,
            json!({ "event": "optin_aligned", "name": req.name, "actor": actor }),
        ));
        "approved"
    } else {
        s.fabric.emit(FabricEvent::closed(
            &record.id,
            Stage::Run,
            "alignment check failed",
        ));
        "rejected"
    };

    Json(json!({ "optin": record, "verdict": status }))
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ActorBody { actor_did: Option<String> }

async fn activate(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let ok = s.optin.activate(&id);
    Json(json!({ "id": id, "activated": ok }))
}

async fn withdraw(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ActorBody>,
) -> Json<Value> {
    let actor = body.actor_did.as_deref().unwrap_or("did:autonomyx:anonymous");
    let ok = s.optin.withdraw(&id, actor);
    Json(json!({ "id": id, "withdrawn": ok }))
}
