// Dashboards — custom views over the full platform world model.
//
// A dashboard is a named, owned, real-time window into what the platform knows.
// Every data source in Autonomyx is a widget source:
//   goals, fabric events, lifecycle gates, usage, blockchain, govgraph,
//   agents, runs, storage, projects, milestones, accountability log.
//
// Dashboards are custom — you define what you see.
// Widgets are live — backed by WebSocket fabric subscription.
// Data is governed — every widget checks the viewer's access policy.
//
// Widget types:
//   Metric    — single number with trend (goals achieved, cost today, trust score)
//   Table     — rows from any data source (runs, agents, jobs, objectives)
//   Timeline  — lifecycle gate history for an artifact
//   Graph     — governance graph node/edge view
//   Impact    — goal impact progress bars across metrics
//   Log       — live fabric event stream (filtered by type, artifact, stage)
//   Map       — world model delta: before/after/scope
//   Ledger    — usage + cost + blockchain settlements
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Widget ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    Metric,    // single KPI with trend
    Table,     // rows from a data source
    Timeline,  // lifecycle gate history
    Graph,     // governance graph
    Impact,    // goal impact progress
    Log,       // live fabric event stream
    Map,       // world model before/after
    Ledger,    // usage + cost + chain settlements
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    Goals,
    GoalDetail,        // requires artifact_id
    Fabric,
    Lifecycle,
    LifecycleArtifact, // requires artifact_id
    Usage,
    Blockchain,
    Govgraph,
    Agents,
    Runs,
    Storage,
    Projects,
    Accountability,
    Computekube,
    Platform,
    Custom,            // raw JSON-path query over AppState
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Widget {
    pub id:          String,
    pub kind:        WidgetKind,
    pub title:       String,
    pub source:      DataSource,
    pub artifact_id: Option<String>,  // filter by artifact / agent / goal
    pub filter:      Option<String>,  // event type, stage name, agent_id, etc.
    pub limit:       Option<usize>,
    pub col:         u8,              // grid column (1-12)
    pub row:         u8,              // grid row
    pub width:       u8,              // column span
    pub height:      u8,              // row span
    pub refresh_sec: Option<u64>,     // 0 = live via WebSocket
    pub config:      Value,           // widget-specific config
}

impl Widget {
    pub fn metric(title: &str, source: DataSource, col: u8, row: u8) -> Self {
        Widget {
            id: Uuid::new_v4().to_string(), kind: WidgetKind::Metric,
            title: title.into(), source, artifact_id: None, filter: None,
            limit: Some(1), col, row, width: 3, height: 1,
            refresh_sec: Some(30), config: json!({}),
        }
    }

    pub fn table(title: &str, source: DataSource, col: u8, row: u8, limit: usize) -> Self {
        Widget {
            id: Uuid::new_v4().to_string(), kind: WidgetKind::Table,
            title: title.into(), source, artifact_id: None, filter: None,
            limit: Some(limit), col, row, width: 6, height: 3,
            refresh_sec: Some(10), config: json!({}),
        }
    }

    pub fn log(title: &str, filter: Option<String>, col: u8, row: u8) -> Self {
        Widget {
            id: Uuid::new_v4().to_string(), kind: WidgetKind::Log,
            title: title.into(), source: DataSource::Fabric,
            artifact_id: None, filter,
            limit: Some(50), col, row, width: 12, height: 4,
            refresh_sec: Some(0), config: json!({}),  // 0 = live WebSocket
        }
    }
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub id:          String,
    pub name:        String,
    pub description: String,
    pub owner_did:   String,
    pub widgets:     Vec<Widget>,
    pub theme:       DashboardTheme,
    pub public:      bool,
    pub pinned:      bool,
    pub tags:        Vec<String>,
    pub created_at:  DateTime<Utc>,
    pub updated_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DashboardTheme {
    Default,
    Dark,
    Impact,    // green-focused, world-change narrative
    Ledger,    // cost/usage financial view
    Mission,   // goal/mission/impact view
    Ops,       // ops: runs, jobs, kube, fabric events
}

// ── Registry ──────────────────────────────────────────────────────────────────

pub struct DashboardRegistry {
    dashboards: RwLock<HashMap<String, Dashboard>>,
}

impl DashboardRegistry {
    pub fn new() -> Self {
        let reg = DashboardRegistry {
            dashboards: RwLock::new(HashMap::new()),
        };
        reg.seed_defaults();
        reg
    }

    /// Seed built-in dashboards — useful day-one, fully customisable.
    fn seed_defaults(&self) {
        // 1. Mission Control — the world view
        let mission = Dashboard {
            id:          "dashboard_mission".into(),
            name:        "Mission Control".into(),
            description: "Goals, alignment, impact. Is the world getting better?".into(),
            owner_did:   "did:autonomyx:platform".into(),
            theme:       DashboardTheme::Mission,
            public:      true,
            pinned:      true,
            tags:        vec!["default".into(), "goals".into()],
            created_at:  Utc::now(),
            updated_at:  Utc::now(),
            widgets: vec![
                Widget::metric("Goals Active",   DataSource::Goals,    1, 1),
                Widget::metric("Goals Achieved", DataSource::Goals,    4, 1),
                Widget::metric("Avg Impact %",   DataSource::Goals,    7, 1),
                Widget::metric("Agents Running", DataSource::Agents,  10, 1),
                Widget { id: Uuid::new_v4().to_string(), kind: WidgetKind::Impact,
                    title: "Impact Progress".into(), source: DataSource::Goals,
                    artifact_id: None, filter: None, limit: Some(10),
                    col: 1, row: 2, width: 8, height: 3,
                    refresh_sec: Some(30), config: json!({}) },
                Widget { id: Uuid::new_v4().to_string(), kind: WidgetKind::Map,
                    title: "World Model".into(), source: DataSource::Goals,
                    artifact_id: None, filter: Some("achieved".into()), limit: Some(5),
                    col: 9, row: 2, width: 4, height: 3,
                    refresh_sec: Some(60), config: json!({}) },
                Widget::log("Live Events", None, 1, 5),
            ],
        };

        // 2. Platform Ops
        let ops = Dashboard {
            id:          "dashboard_ops".into(),
            name:        "Platform Ops".into(),
            description: "Runs, jobs, gates, fabric — is the platform healthy?".into(),
            owner_did:   "did:autonomyx:platform".into(),
            theme:       DashboardTheme::Ops,
            public:      true,
            pinned:      true,
            tags:        vec!["default".into(), "ops".into()],
            created_at:  Utc::now(),
            updated_at:  Utc::now(),
            widgets: vec![
                Widget::metric("Active Runs",    DataSource::Runs,       1, 1),
                Widget::metric("Kube Jobs",      DataSource::Computekube, 4, 1),
                Widget::metric("Gate Events",    DataSource::Fabric,     7, 1),
                Widget::metric("Total Cost USD", DataSource::Usage,     10, 1),
                Widget::table("Recent Runs",     DataSource::Runs,       1, 2, 20),
                Widget::table("Kube Jobs",       DataSource::Computekube, 7, 2, 20),
                Widget { id: Uuid::new_v4().to_string(), kind: WidgetKind::Timeline,
                    title: "Lifecycle Gates".into(), source: DataSource::Lifecycle,
                    artifact_id: None, filter: None, limit: Some(30),
                    col: 1, row: 5, width: 12, height: 3,
                    refresh_sec: Some(5), config: json!({}) },
                Widget::log("Fabric Stream", Some("gate".into()), 1, 8),
            ],
        };

        // 3. Ledger — cost, usage, blockchain
        let ledger = Dashboard {
            id:          "dashboard_ledger".into(),
            name:        "Ledger".into(),
            description: "Usage, cost, settlements. Freedom not free — transparently.".into(),
            owner_did:   "did:autonomyx:platform".into(),
            theme:       DashboardTheme::Ledger,
            public:      true,
            pinned:      false,
            tags:        vec!["default".into(), "cost".into()],
            created_at:  Utc::now(),
            updated_at:  Utc::now(),
            widgets: vec![
                Widget::metric("Total Cost USD",   DataSource::Usage,      1, 1),
                Widget::metric("Tokens In",        DataSource::Usage,      4, 1),
                Widget::metric("Tokens Out",       DataSource::Usage,      7, 1),
                Widget::metric("Chain Settlements",DataSource::Blockchain, 10, 1),
                Widget::table("Usage Records",     DataSource::Usage,      1, 2, 50),
                Widget { id: Uuid::new_v4().to_string(), kind: WidgetKind::Ledger,
                    title: "Chain Ledger".into(), source: DataSource::Blockchain,
                    artifact_id: None, filter: None, limit: Some(20),
                    col: 7, row: 2, width: 6, height: 3,
                    refresh_sec: Some(30), config: json!({}) },
            ],
        };

        // 4. Governance — graph, trust, accountability
        let gov = Dashboard {
            id:          "dashboard_gov".into(),
            name:        "Governance".into(),
            description: "Graph, trust scores, accountability. Who did what?".into(),
            owner_did:   "did:autonomyx:platform".into(),
            theme:       DashboardTheme::Default,
            public:      true,
            pinned:      false,
            tags:        vec!["default".into(), "governance".into()],
            created_at:  Utc::now(),
            updated_at:  Utc::now(),
            widgets: vec![
                Widget::metric("Graph Nodes",     DataSource::Govgraph,      1, 1),
                Widget::metric("Graph Edges",     DataSource::Govgraph,      4, 1),
                Widget::metric("Traversals",      DataSource::Govgraph,      7, 1),
                Widget::metric("Avg Trust Score", DataSource::Govgraph,     10, 1),
                Widget { id: Uuid::new_v4().to_string(), kind: WidgetKind::Graph,
                    title: "Governance Graph".into(), source: DataSource::Govgraph,
                    artifact_id: None, filter: None, limit: None,
                    col: 1, row: 2, width: 8, height: 5,
                    refresh_sec: Some(10), config: json!({}) },
                Widget::table("Accountability Log", DataSource::Accountability, 9, 2, 30),
            ],
        };

        let mut db = self.dashboards.write().unwrap();
        db.insert(mission.id.clone(), mission);
        db.insert(ops.id.clone(), ops);
        db.insert(ledger.id.clone(), ledger);
        db.insert(gov.id.clone(), gov);
    }

    pub fn create(&self, dashboard: Dashboard) -> Dashboard {
        let mut db = self.dashboards.write().unwrap();
        db.insert(dashboard.id.clone(), dashboard.clone());
        dashboard
    }

    pub fn get(&self, id: &str) -> Option<Dashboard> {
        self.dashboards.read().unwrap().get(id).cloned()
    }

    pub fn list(&self, owner_did: Option<&str>) -> Vec<Dashboard> {
        self.dashboards.read().unwrap().values()
            .filter(|d| {
                d.public || owner_did.map(|o| d.owner_did == o).unwrap_or(false)
            })
            .cloned().collect()
    }

    pub fn update_widgets(&self, id: &str, widgets: Vec<Widget>) -> Option<Dashboard> {
        let mut db = self.dashboards.write().unwrap();
        let d = db.get_mut(id)?;
        d.widgets    = widgets;
        d.updated_at = Utc::now();
        Some(d.clone())
    }

    pub fn add_widget(&self, dashboard_id: &str, widget: Widget) -> Option<Dashboard> {
        let mut db = self.dashboards.write().unwrap();
        let d = db.get_mut(dashboard_id)?;
        d.widgets.push(widget);
        d.updated_at = Utc::now();
        Some(d.clone())
    }

    pub fn remove_widget(&self, dashboard_id: &str, widget_id: &str) -> Option<Dashboard> {
        let mut db = self.dashboards.write().unwrap();
        let d = db.get_mut(dashboard_id)?;
        d.widgets.retain(|w| w.id != widget_id);
        d.updated_at = Utc::now();
        Some(d.clone())
    }

    pub fn delete(&self, id: &str) -> bool {
        self.dashboards.write().unwrap().remove(id).is_some()
    }
}
