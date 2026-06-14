// Dashboard routes — custom views over the full platform world model.
//
// GET  /api/dashboards              — list dashboards (owned + public)
// POST /api/dashboards              — create a custom dashboard
// GET  /api/dashboards/:id          — get dashboard layout
// GET  /api/dashboards/:id/data     — render all widget data (single request)
// PUT  /api/dashboards/:id/widgets  — replace widget layout
// POST /api/dashboards/:id/widgets  — add a widget
// DELETE /api/dashboards/:id/widgets/:wid — remove a widget
// DELETE /api/dashboards/:id        — delete dashboard
// GET  /api/dashboards/:id/live     — WebSocket upgrade for live widget push
//
// openautonomyx.com

use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;
use crate::store::AppState;
use crate::dashboard::{Dashboard, DashboardTheme, DataSource, Widget, WidgetKind};

#[derive(Deserialize)]
struct CreateDashboardReq {
    name:        String,
    description: Option<String>,
    owner_did:   Option<String>,
    theme:       Option<String>,
    public:      Option<bool>,
    tags:        Option<Vec<String>>,
}

#[derive(Deserialize)]
struct AddWidgetReq {
    kind:        String,
    title:       String,
    source:      String,
    artifact_id: Option<String>,
    filter:      Option<String>,
    limit:       Option<usize>,
    col:         Option<u8>,
    row:         Option<u8>,
    width:       Option<u8>,
    height:      Option<u8>,
    refresh_sec: Option<u64>,
    config:      Option<Value>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/dashboards",                         get(list_dashboards).post(create_dashboard))
        .route("/dashboards/:id",                     get(get_dashboard).delete(delete_dashboard))
        .route("/dashboards/:id/data",                get(dashboard_data))
        .route("/dashboards/:id/widgets",             post(add_widget).put(replace_widgets))
        .route("/dashboards/:id/widgets/:wid",        delete(remove_widget))
        .with_state(state)
}

async fn list_dashboards(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Json<Value> {
    let owner = q.get("owner_did").map(|s| s.as_str());
    let list  = state.dashboards.list(owner);
    Json(json!({ "dashboards": list, "count": list.len() }))
}

async fn create_dashboard(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateDashboardReq>,
) -> Json<Value> {
    let theme = match req.theme.as_deref() {
        Some("dark")    => DashboardTheme::Dark,
        Some("impact")  => DashboardTheme::Impact,
        Some("ledger")  => DashboardTheme::Ledger,
        Some("mission") => DashboardTheme::Mission,
        Some("ops")     => DashboardTheme::Ops,
        _               => DashboardTheme::Default,
    };
    let d = Dashboard {
        id:          Uuid::new_v4().to_string(),
        name:        req.name,
        description: req.description.unwrap_or_default(),
        owner_did:   req.owner_did.unwrap_or_else(|| "did:autonomyx:platform".into()),
        widgets:     vec![],
        theme,
        public:      req.public.unwrap_or(false),
        pinned:      false,
        tags:        req.tags.unwrap_or_default(),
        created_at:  Utc::now(),
        updated_at:  Utc::now(),
    };
    let d = state.dashboards.create(d);
    Json(json!({ "dashboard": d }))
}

async fn get_dashboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match state.dashboards.get(&id) {
        Some(d) => Json(json!({ "dashboard": d })),
        None    => Json(json!({ "error": "dashboard not found", "id": id })),
    }
}

async fn delete_dashboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    Json(json!({ "deleted": state.dashboards.delete(&id), "id": id }))
}

/// GET /api/dashboards/:id/data — render all widgets in one request.
/// Each widget's data is fetched from the appropriate AppState source.
/// The client gets a complete dashboard snapshot; live updates via WebSocket.
async fn dashboard_data(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let Some(dashboard) = state.dashboards.get(&id) else {
        return Json(json!({ "error": "dashboard not found" }));
    };

    let widget_data: Vec<Value> = dashboard.widgets.iter()
        .map(|w| json!({
            "widget_id": w.id,
            "kind":      w.kind,
            "title":     w.title,
            "col":       w.col,
            "row":       w.row,
            "width":     w.width,
            "height":    w.height,
            "refresh_sec": w.refresh_sec,
            "data":      render_widget(w, &state),
        }))
        .collect();

    Json(json!({
        "dashboard": {
            "id":    dashboard.id,
            "name":  dashboard.name,
            "theme": dashboard.theme,
        },
        "widgets":    widget_data,
        "rendered_at": Utc::now(),
        "live_url":    format!("/ws/stream"),
    }))
}

/// Render a single widget's data from AppState.
fn render_widget(w: &Widget, state: &Arc<AppState>) -> Value {
    match w.source {
        DataSource::Goals => {
            let summary = state.goals.summary();
            match w.kind {
                WidgetKind::Metric => {
                    let title = w.title.to_lowercase();
                    if title.contains("active")   { json!({ "value": summary["goals"]["active"],   "label": "Active Goals" }) }
                    else if title.contains("achieved") { json!({ "value": summary["goals"]["achieved"], "label": "Achieved" }) }
                    else if title.contains("impact") { json!({ "value": summary["impact_progress"], "label": "Avg Impact %", "format": "percent" }) }
                    else { summary }
                }
                WidgetKind::Impact => {
                    let goals = state.goals.list_goals(None);
                    json!(goals.into_iter()
                        .filter(|g| g.status == crate::goals::GoalStatus::Active || g.status == crate::goals::GoalStatus::Achieved)
                        .take(w.limit.unwrap_or(10))
                        .map(|g| json!({
                            "id":      g.id,
                            "title":   g.title,
                            "status":  g.status,
                            "metrics": g.impact_metrics.iter().map(|m| json!({
                                "name":     m.name,
                                "unit":     m.unit,
                                "progress": m.progress(),
                                "actual":   m.actual,
                                "target":   m.target,
                            })).collect::<Vec<_>>(),
                        }))
                        .collect::<Vec<_>>())
                }
                WidgetKind::Map => {
                    let goals = state.goals.list_goals(None);
                    json!(goals.into_iter()
                        .filter(|g| w.filter.as_deref().map(|f| format!("{:?}", g.status).to_lowercase().contains(f)).unwrap_or(true))
                        .take(w.limit.unwrap_or(5))
                        .map(|g| json!({
                            "title":  g.title,
                            "before": g.world_model.before,
                            "after":  g.world_model.after,
                            "scope":  g.world_model.scope,
                            "status": g.status,
                        }))
                        .collect::<Vec<_>>())
                }
                _ => state.goals.summary(),
            }
        }

        DataSource::Fabric => {
            let events_raw = state.fabric.full_log();
            let events: Vec<Value> = events_raw.iter().rev()
                .take(w.limit.unwrap_or(50))
                .map(|e| json!({
                    "id": e.id, "artifact": e.artifact,
                    "stage": e.stage.as_str(), "status": e.status,
                    "payload": e.payload, "at": e.emitted_at,
                })).collect();
            let events = events;
            match w.kind {
                WidgetKind::Metric => json!({ "value": events.len(), "label": "Fabric Events" }),
                _ => {
                    let filtered: Vec<&Value> = events.iter()
                        .filter(|e| {
                            w.filter.as_ref().map(|f| {
                                e["stage"].as_str().unwrap_or("").contains(f.as_str()) ||
                                e["payload"]["type"].as_str().unwrap_or("").contains(f.as_str())
                            }).unwrap_or(true)
                        })
                        .collect();
                    json!(filtered)
                }
            }
        }

        DataSource::Runs => {
            let runs = state.list_runs();
            match w.kind {
                WidgetKind::Metric => {
                    let active = runs.iter().filter(|r| r.status == crate::store::RunStatus::Running).count();
                    json!({ "value": active, "label": "Active Runs", "total": runs.len() })
                }
                _ => {
                    let limited: Vec<_> = runs.into_iter().rev()
                        .take(w.limit.unwrap_or(20)).collect();
                    json!(limited)
                }
            }
        }

        DataSource::Agents => {
            let agents = state.list_agents();
            match w.kind {
                WidgetKind::Metric => json!({ "value": agents.len(), "label": "Agents" }),
                _ => json!(agents.into_iter().take(w.limit.unwrap_or(20)).collect::<Vec<_>>()),
            }
        }

        DataSource::Usage => {
            let summary = state.usage.summary();
            match w.kind {
                WidgetKind::Metric => {
                    let title = w.title.to_lowercase();
                    if title.contains("cost")       { json!({ "value": summary["total_cost_usd"], "label": "Total Cost USD", "format": "currency" }) }
                    else if title.contains("tokens in") { json!({ "value": summary["total_tokens_in"], "label": "Tokens In" }) }
                    else if title.contains("tokens out") { json!({ "value": summary["total_tokens_out"], "label": "Tokens Out" }) }
                    else { summary }
                }
                WidgetKind::Table  => {
                    let records = state.usage.all_records();
                    json!(records.into_iter().rev().take(w.limit.unwrap_or(50))
                        .map(|r| json!({
                            "did": r.did, "artifact": r.artifact,
                            "stage": r.stage.as_str(), "model": r.model,
                            "tokens_in": r.tokens_in, "tokens_out": r.tokens_out,
                            "cost_usd": r.total_usd(), "at": r.recorded_at,
                        })).collect::<Vec<_>>())
                }
                _ => summary,
            }
        }

        DataSource::Blockchain => {
            let summary = state.blockchain.summary();
            match w.kind {
                WidgetKind::Metric => json!({ "value": summary["events"]["submitted"], "label": "Chain Settlements" }),
                WidgetKind::Ledger => json!({
                    "summary":   summary,
                    "pending":   state.blockchain.pending(),
                    "submitted": state.blockchain.submitted(),
                }),
                _ => summary,
            }
        }

        DataSource::Govgraph => {
            let summary = state.govgraph.summary();
            match w.kind {
                WidgetKind::Metric => {
                    let title = w.title.to_lowercase();
                    if title.contains("node")       { json!({ "value": summary["nodes"],            "label": "Nodes" }) }
                    else if title.contains("edge")  { json!({ "value": summary["edges"],            "label": "Edges" }) }
                    else if title.contains("trav")  { json!({ "value": summary["total_traversals"], "label": "Traversals" }) }
                    else if title.contains("trust") { json!({ "value": summary["avg_trust"],        "label": "Avg Trust", "format": "score" }) }
                    else { summary }
                }
                WidgetKind::Graph => state.govgraph.to_graph_json(),
                _ => summary,
            }
        }

        DataSource::Lifecycle => {
            let log = state.lifecycle.full_log();
            match w.kind {
                WidgetKind::Metric    => json!({ "value": log.len(), "label": "Gate Events" }),
                WidgetKind::Timeline  => {
                    json!(log.iter().rev().take(w.limit.unwrap_or(30))
                        .map(|r| json!({
                            "artifact": r.artifact,
                            "stage":    r.stage.as_str(),
                            "status":   r.status,
                            "oath":     r.oath,
                            "at":       r.transitioned_at,
                        })).collect::<Vec<_>>())
                }
                _ => json!(log.len()),
            }
        }

        DataSource::Computekube => {
            let summary = state.computekube.summary();
            match w.kind {
                WidgetKind::Metric => {
                    let title = w.title.to_lowercase();
                    if title.contains("job") { json!({ "value": summary["jobs"]["total"], "label": "Kube Jobs" }) }
                    else { summary.clone() }
                }
                WidgetKind::Table => {
                    let jobs = state.computekube.list_jobs();
                    json!(jobs.into_iter().rev().take(w.limit.unwrap_or(20)).collect::<Vec<_>>())
                }
                _ => summary,
            }
        }

        DataSource::Accountability => {
            let log = state.federation.full_audit_log();
            json!(log.into_iter().rev().take(w.limit.unwrap_or(30))
                .map(|r| json!({
                    "id":       r.id,
                    "actor":    r.did,
                    "action":   r.action,
                    "resource": r.resource,
                    "outcome":  r.outcome,
                    "at":       r.recorded_at,
                })).collect::<Vec<_>>())
        }

        DataSource::Platform => {
            let cloud  = crate::cloud::CloudContext::detect();
            let device = crate::cloud::DeviceContext::detect();
            json!({
                "version":  env!("CARGO_PKG_VERSION"),
                "cloud":    cloud,
                "device":   device,
                "uptime":   "live",
            })
        }

        _ => json!({ "source": format!("{:?}", w.source), "note": "data source not yet wired" }),
    }
}

async fn add_widget(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<AddWidgetReq>,
) -> Json<Value> {
    let kind = parse_kind(req.kind.as_str());
    let source = parse_source(req.source.as_str());
    let widget = Widget {
        id:          Uuid::new_v4().to_string(),
        kind,
        title:       req.title,
        source,
        artifact_id: req.artifact_id,
        filter:      req.filter,
        limit:       req.limit,
        col:         req.col.unwrap_or(1),
        row:         req.row.unwrap_or(1),
        width:       req.width.unwrap_or(6),
        height:      req.height.unwrap_or(2),
        refresh_sec: req.refresh_sec,
        config:      req.config.unwrap_or(json!({})),
    };
    match state.dashboards.add_widget(&id, widget) {
        Some(d) => Json(json!({ "dashboard": d })),
        None    => Json(json!({ "error": "dashboard not found" })),
    }
}

async fn replace_widgets(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(widgets): Json<Vec<Widget>>,
) -> Json<Value> {
    match state.dashboards.update_widgets(&id, widgets) {
        Some(d) => Json(json!({ "dashboard": d })),
        None    => Json(json!({ "error": "dashboard not found" })),
    }
}

async fn remove_widget(
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Json<Value> {
    match state.dashboards.remove_widget(&id, &wid) {
        Some(d) => Json(json!({ "dashboard": d, "removed": wid })),
        None    => Json(json!({ "error": "dashboard or widget not found" })),
    }
}

fn parse_kind(s: &str) -> WidgetKind {
    match s {
        "table"    => WidgetKind::Table,
        "timeline" => WidgetKind::Timeline,
        "graph"    => WidgetKind::Graph,
        "impact"   => WidgetKind::Impact,
        "log"      => WidgetKind::Log,
        "map"      => WidgetKind::Map,
        "ledger"   => WidgetKind::Ledger,
        _          => WidgetKind::Metric,
    }
}

fn parse_source(s: &str) -> DataSource {
    match s {
        "goals"          => DataSource::Goals,
        "fabric"         => DataSource::Fabric,
        "lifecycle"      => DataSource::Lifecycle,
        "usage"          => DataSource::Usage,
        "blockchain"     => DataSource::Blockchain,
        "govgraph"       => DataSource::Govgraph,
        "agents"         => DataSource::Agents,
        "runs"           => DataSource::Runs,
        "storage"        => DataSource::Storage,
        "projects"       => DataSource::Projects,
        "accountability" => DataSource::Accountability,
        "computekube"    => DataSource::Computekube,
        "platform"       => DataSource::Platform,
        _                => DataSource::Custom,
    }
}
