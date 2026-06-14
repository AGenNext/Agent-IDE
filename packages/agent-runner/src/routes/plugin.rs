use axum::{Router, routing::{get, post}, extract::{State, Path}, Json};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::AppState;
use crate::plugin::{PluginDescriptor, PluginKind, PluginNode};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/plugins",              get(list_plugins).post(register_plugin))
        .route("/plugins/summary",      get(plugin_summary))
        .route("/plugins/capabilities", get(list_capabilities))
        .route("/plugins/:id",          get(get_plugin))
        .route("/plugins/:id/enable",   post(enable_plugin))
        .route("/plugins/:id/disable",  post(disable_plugin))
        .route("/plugins/:id/nodes",    get(plugin_nodes))
        .with_state(state)
}

async fn list_plugins(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "plugins": s.plugins.list() }))
}

async fn plugin_summary(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(s.plugins.summary())
}

async fn list_capabilities(State(s): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "capabilities": s.plugins.all_capabilities() }))
}

async fn get_plugin(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match s.plugins.get(&id) {
        Some(p) => Json(json!({ "plugin": p })),
        None    => Json(json!({ "error": "plugin not found", "id": id })),
    }
}

#[derive(Deserialize)]
struct RegisterPluginReq {
    id:           String,
    name:         String,
    version:      String,
    description:  String,
    author:       String,
    kind:         String,
    capabilities: Vec<String>,
    config_keys:  Option<Vec<String>>,
    homepage:     Option<String>,
}

async fn register_plugin(
    State(s): State<Arc<AppState>>,
    Json(req): Json<RegisterPluginReq>,
) -> Json<Value> {
    let kind = match req.kind.as_str() {
        "compute_provider"  => PluginKind::ComputeProvider,
        "storage_backend"   => PluginKind::StorageBackend,
        "data_pipeline"     => PluginKind::DataPipeline,
        "connector"         => PluginKind::Connector,
        "chain_adapter"     => PluginKind::ChainAdapter,
        "governance_rule"   => PluginKind::GovernanceRule,
        "dashboard_source"  => PluginKind::DashboardSource,
        "tool"              => PluginKind::Tool,
        "observer"          => PluginKind::Observer,
        _                   => PluginKind::Tool,
    };

    let plugin = PluginDescriptor {
        id:           req.id,
        name:         req.name,
        version:      req.version,
        description:  req.description,
        author:       req.author,
        kind,
        capabilities: req.capabilities,
        config_keys:  req.config_keys.unwrap_or_default(),
        enabled:      true,
        loaded:       true,
        homepage:     req.homepage,
        nodes:        vec![],
        data_sources: vec![],
    };

    let registered = s.plugins.register(plugin);
    Json(json!({ "plugin": registered, "status": "registered" }))
}

async fn enable_plugin(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match s.plugins.set_enabled(&id, true) {
        true  => Json(json!({ "id": id, "enabled": true })),
        false => Json(json!({ "error": "plugin not found", "id": id })),
    }
}

async fn disable_plugin(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match s.plugins.set_enabled(&id, false) {
        true  => Json(json!({ "id": id, "enabled": false })),
        false => Json(json!({ "error": "plugin not found", "id": id })),
    }
}

async fn plugin_nodes(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match s.plugins.get(&id) {
        Some(p) => Json(json!({ "id": id, "nodes": p.nodes })),
        None    => Json(json!({ "error": "plugin not found", "id": id })),
    }
}
