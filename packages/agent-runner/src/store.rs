use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::lifecycle::LifecycleRegistry;
use crate::fabric::Fabric;
use crate::federation::FederationRegistry;
use crate::usage::UsageMeter;
use crate::onboarding::OnboardingRegistry;
use crate::blockchain::BlockchainBridge;
use crate::storage::StorageRegistry;
use crate::govgraph::GovernanceGraph;
use crate::goals::GoalRegistry;
use crate::dashboard::DashboardRegistry;
use crate::computekube::ComputeKube;
use crate::plugin::PluginRegistry;
use crate::optin::OptInRegistry;
use crate::authmatic::AuthMatic;
use crate::megaverse::Megaverse;
use crate::teams::TeamRegistry;

// ── Agent identity ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub id:           String,
    pub owner_id:     String,
    pub name:         String,
    pub description:  String,
    pub model:        String,
    pub status:       String,
    pub capabilities: Vec<String>,
    pub created_at:   DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
}

// ── Run ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus { Running, Completed, Failed, Cancelled }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStep {
    pub step:      usize,
    pub r#type:    String,
    pub content:   String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub run_id:      String,
    pub agent_id:    String,
    pub agent_name:  String,
    pub model:       String,
    pub task:        String,
    pub status:      RunStatus,
    pub steps:       Vec<RunStep>,
    pub started_at:  DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ── Peer ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id:        String,
    pub name:      String,
    pub url:       String,
    pub status:    String,
    pub last_seen: Option<DateTime<Utc>>,
    pub region:    Option<String>,
}

// ── Application ───────────────────────────────────────────────────────────────
// "Application is the product" — the declared .ayx artifact the platform makes real.
// An app is a named, versioned, governed collection of agents + workflows.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
    pub id:          String,
    pub owner_id:    String,
    pub name:        String,
    pub description: String,
    pub version:     String,
    pub did:         Option<String>,   // did:autonomyx:<pubkey> — assigned at build gate
    pub status:      AppStatus,
    pub agents:      Vec<String>,      // agent IDs bound to this app
    pub ayx_source:  Option<String>,   // .ayx declaration source (theory)
    pub created_at:  DateTime<Utc>,
    pub updated_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    Draft,     // declared, not yet built
    Building,  // build gate open
    Live,      // deployed and running
    Paused,    // suspended
    Retired,   // end-of-life
}

// ── AppState — Autonomyx Runtime Core ────────────────────────────────────────
// Single shared state; native everywhere (home server, edge, k8s, embedded).
// ConfigDB (SurrealDB) is the live config store — all other state is ephemeral.

pub struct AppState {
    pub agents:  RwLock<HashMap<String, AgentIdentity>>,
    pub runs:    RwLock<HashMap<String, AgentRun>>,
    pub peers:   RwLock<HashMap<String, Peer>>,
    pub apps:    RwLock<HashMap<String, Application>>,
    // WebSocket event sinks: run_id → tokio broadcast senders
    pub ws_sinks: RwLock<HashMap<String, Vec<tokio::sync::broadcast::Sender<String>>>>,
    // ConfigDB handle — SurrealDB (embedded or remote)
    pub config:   std::sync::Arc<crate::configdb::ConfigDB>,
    // Lifecycle gate registry — idempotent ACID stage transitions
    pub lifecycle: LifecycleRegistry,
    // Fabric — framework that fills the gaps between gates
    pub fabric: Arc<Fabric>,
    // Federation — real, unique, identifiable, governed, autonomous, federal, accountable, intelligent
    pub federation: FederationRegistry,
    // Usage meter — usage-based billing: fair, transparent, freedom not free
    pub usage: UsageMeter,
    // Onboarding — chat-based configuration; the chat IS the platform interface
    pub onboarding: OnboardingRegistry,
    // Blockchain bridge — on-chain DID, accountability, usage settlement, agent NFTs
    pub blockchain: Arc<BlockchainBridge>,
    // Distributed storage — policy-driven, milestone-bound, fine-grained ACL
    pub storage: StorageRegistry,
    // Governance graph — compute core wired to governance at every edge
    pub govgraph: GovernanceGraph,
    // ComputeKube — Kubernetes execution backend for governance graph nodes
    pub computekube: ComputeKube,
    // Goals — purpose-driven agents; mission → goal → objective → impact → loop
    pub goals: GoalRegistry,
    // Dashboards — custom views; 4 built-in + unlimited custom
    pub dashboards: DashboardRegistry,
    // Plugins — everything extendable; built-in + custom; contribute nodes/sources
    pub plugins: PluginRegistry,
    // Opt-in — voluntary governed flows: extend (add capability) or align (7-value check)
    pub optin: OptInRegistry,
    // Auth-matic — automatic auth: JIT keys, enrollment tokens, peer credentials
    pub authmatic: AuthMatic,
    // Megaverse — unified world model: every entity, every relationship, every surface
    pub megaverse: Arc<Megaverse>,
    // Teams — institutional agent teams: cross-sector, unicode-native, federation-aware
    pub teams: TeamRegistry,
}

impl AppState {
    pub fn new() -> Self {
        let mut agents = HashMap::new();
        let demo = AgentIdentity {
            id: "agent_demo".into(), owner_id: "user_demo".into(),
            name: "Demo Agent".into(), description: "Built-in demo agent".into(),
            model: "gpt-4o".into(), status: "idle".into(),
            capabilities: vec!["research".into(), "code".into()],
            created_at: Utc::now(), updated_at: Utc::now(),
        };
        agents.insert(demo.id.clone(), demo);

        // ConfigDB: start with in-memory stub; connect() upgrades async on startup
        let config = std::sync::Arc::new(crate::configdb::ConfigDB::new_sync());

        let fabric = Arc::new(Fabric::new());

        Self {
            agents:     RwLock::new(agents),
            runs:       RwLock::new(HashMap::new()),
            peers:      RwLock::new(HashMap::new()),
            apps:       RwLock::new(HashMap::new()),
            ws_sinks:   RwLock::new(HashMap::new()),
            config,
            lifecycle:  LifecycleRegistry::new(),
            fabric,
            federation:  FederationRegistry::new(),
            usage:       UsageMeter::new(),
            onboarding:  OnboardingRegistry::new(),
            blockchain:  BlockchainBridge::new(),
            storage:     StorageRegistry::new(),
            govgraph:    GovernanceGraph::new(),
            computekube: ComputeKube::new(),
            goals:       GoalRegistry::new(),
            dashboards:  DashboardRegistry::new(),
            plugins:     PluginRegistry::new(),
            optin:       OptInRegistry::new(),
            authmatic:   AuthMatic::new(),
            megaverse:   Arc::new(Megaverse::new()),
            teams:       TeamRegistry::new(),
        }
    }

    pub fn create_agent(&self, owner_id: &str, name: &str, description: &str, model: &str) -> AgentIdentity {
        let id = format!("agent_{}", Uuid::new_v4().simple());
        let agent = AgentIdentity {
            id: id.clone(), owner_id: owner_id.into(),
            name: name.into(), description: description.into(),
            model: model.into(), status: "idle".into(),
            capabilities: vec![],
            created_at: Utc::now(), updated_at: Utc::now(),
        };
        self.agents.write().unwrap().insert(id, agent.clone());
        // Thread: emit fabric event so the megaverse live-updates and thread is woven
        self.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("agent:{}", agent.id),
                crate::lifecycle::Stage::Build,
                serde_json::json!({ "action": "created", "name": name, "owner": owner_id }),
            ).with_entities([format!("agent:{}", agent.id), format!("owner:{owner_id}")])
        );
        agent
    }

    pub fn get_agent(&self, id: &str) -> Option<AgentIdentity> {
        self.agents.read().unwrap().get(id).cloned()
    }

    pub fn list_agents(&self) -> Vec<AgentIdentity> {
        self.agents.read().unwrap().values().cloned().collect()
    }

    pub fn create_run(&self, agent_id: &str, agent_name: &str, model: &str, task: &str) -> AgentRun {
        let run_id = format!("run_{}", Uuid::new_v4().simple());
        let run = AgentRun {
            run_id: run_id.clone(), agent_id: agent_id.into(),
            agent_name: agent_name.into(), model: model.into(),
            task: task.into(), status: RunStatus::Running,
            steps: vec![], started_at: Utc::now(), completed_at: None,
        };
        self.runs.write().unwrap().insert(run_id, run.clone());
        // Thread: weave run into fabric so megaverse and subscribers see it immediately
        self.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("run:{}", run.run_id),
                crate::lifecycle::Stage::Run,
                serde_json::json!({ "action": "started", "agent": agent_id, "model": model }),
            ).with_entities([
                format!("run:{}", run.run_id),
                format!("agent:{agent_id}"),
            ])
        );
        run
    }

    pub fn get_run(&self, id: &str) -> Option<AgentRun> {
        self.runs.read().unwrap().get(id).cloned()
    }

    pub fn list_runs(&self) -> Vec<AgentRun> {
        self.runs.read().unwrap().values().cloned().collect()
    }

    pub fn add_run_step(&self, run_id: &str, step_type: &str, content: &str) -> Option<RunStep> {
        let mut runs = self.runs.write().unwrap();
        let run = runs.get_mut(run_id)?;
        let step = RunStep {
            step: run.steps.len() + 1,
            r#type: step_type.into(),
            content: content.into(),
            timestamp: Utc::now(),
        };
        run.steps.push(step.clone());
        Some(step)
    }

    pub fn finish_run(&self, run_id: &str, status: RunStatus) {
        let agent_id = {
            let mut runs = self.runs.write().unwrap();
            if let Some(run) = runs.get_mut(run_id) {
                run.status = status.clone();
                run.completed_at = Some(Utc::now());
                run.agent_id.clone()
            } else { return; }
        };
        // Thread: weave completion into fabric
        let stage = if status == RunStatus::Completed { crate::lifecycle::Stage::Feedback }
                    else { crate::lifecycle::Stage::Observe };
        self.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("run:{run_id}"),
                stage,
                serde_json::json!({ "action": "finished", "status": format!("{:?}", status) }),
            ).with_entities([format!("run:{run_id}"), format!("agent:{agent_id}")])
        );
    }

    pub fn register_ws_sink(&self, run_id: &str, tx: tokio::sync::broadcast::Sender<String>) {
        self.ws_sinks.write().unwrap()
            .entry(run_id.to_string()).or_default().push(tx);
    }

    pub fn broadcast_to_run(&self, run_id: &str, msg: &str) {
        if let Some(senders) = self.ws_sinks.read().unwrap().get(run_id) {
            for tx in senders { let _ = tx.send(msg.to_string()); }
        }
    }

    pub fn create_peer(&self, name: &str, url: &str, region: Option<&str>) -> Peer {
        let id = Uuid::new_v4().to_string();
        let peer = Peer {
            id: id.clone(), name: name.into(),
            url: url.trim_end_matches('/').into(),
            status: "unknown".into(), last_seen: None,
            region: region.map(|r| r.into()),
        };
        self.peers.write().unwrap().insert(id, peer.clone());
        self.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("peer:{}", peer.id),
                crate::lifecycle::Stage::Build,
                serde_json::json!({ "action": "registered", "name": name, "url": url, "region": region }),
            ).with_entities([format!("peer:{}", peer.id)])
        );
        peer
    }

    pub fn get_peer(&self, id: &str) -> Option<Peer> {
        self.peers.read().unwrap().get(id).cloned()
    }

    pub fn list_peers(&self) -> Vec<Peer> {
        self.peers.read().unwrap().values().cloned().collect()
    }

    pub fn remove_peer(&self, id: &str) -> bool {
        let removed = self.peers.write().unwrap().remove(id).is_some();
        if removed {
            self.fabric.emit(
                crate::fabric::FabricEvent::open(
                    &format!("peer:{id}"),
                    crate::lifecycle::Stage::Observe,
                    serde_json::json!({ "action": "removed" }),
                ).with_entities([format!("peer:{id}")])
            );
        }
        removed
    }

    pub fn set_peer_status(&self, id: &str, status: &str) {
        let found = {
            let mut peers = self.peers.write().unwrap();
            if let Some(p) = peers.get_mut(id) {
                p.status = status.into();
                if status == "online" { p.last_seen = Some(Utc::now()); }
                true
            } else { false }
        };
        if found {
            self.fabric.emit(
                crate::fabric::FabricEvent::open(
                    &format!("peer:{id}"),
                    crate::lifecycle::Stage::Observe,
                    serde_json::json!({ "action": "status_changed", "status": status }),
                ).with_entities([format!("peer:{id}")])
            );
        }
    }

    pub fn create_app(&self, owner_id: &str, name: &str, description: &str, version: &str, ayx_source: Option<&str>) -> Application {
        let id = format!("app_{}", Uuid::new_v4().simple());
        let app = Application {
            id: id.clone(), owner_id: owner_id.into(),
            name: name.into(), description: description.into(),
            version: version.into(), did: None,
            status: AppStatus::Draft,
            agents: vec![],
            ayx_source: ayx_source.map(|s| s.into()),
            created_at: Utc::now(), updated_at: Utc::now(),
        };
        self.apps.write().unwrap().insert(id, app.clone());
        self.fabric.emit(
            crate::fabric::FabricEvent::open(
                &format!("app:{}", app.id),
                crate::lifecycle::Stage::Build,
                serde_json::json!({ "action": "created", "name": name, "owner": owner_id, "version": version }),
            ).with_entities([format!("app:{}", app.id), format!("owner:{owner_id}")])
        );
        app
    }

    pub fn get_app(&self, id: &str) -> Option<Application> {
        self.apps.read().unwrap().get(id).cloned()
    }

    pub fn list_apps(&self) -> Vec<Application> {
        self.apps.read().unwrap().values().cloned().collect()
    }

    pub fn activate_app(&self, id: &str, did: &str) {
        let found = {
            let mut apps = self.apps.write().unwrap();
            if let Some(app) = apps.get_mut(id) {
                app.did = Some(did.into());
                app.status = AppStatus::Live;
                app.updated_at = Utc::now();
                true
            } else { false }
        };
        if found {
            self.fabric.emit(
                crate::fabric::FabricEvent::open(
                    &format!("app:{id}"),
                    crate::lifecycle::Stage::Deploy,
                    serde_json::json!({ "action": "activated", "did": did }),
                ).with_entities([format!("app:{id}")])
            );
        }
    }

    pub fn bind_agent_to_app(&self, app_id: &str, agent_id: &str) {
        let found = {
            let mut apps = self.apps.write().unwrap();
            if let Some(app) = apps.get_mut(app_id) {
                if !app.agents.contains(&agent_id.to_string()) {
                    app.agents.push(agent_id.into());
                    app.updated_at = Utc::now();
                }
                true
            } else { false }
        };
        if found {
            self.fabric.emit(
                crate::fabric::FabricEvent::open(
                    &format!("app:{app_id}"),
                    crate::lifecycle::Stage::Build,
                    serde_json::json!({ "action": "agent_bound", "agent_id": agent_id }),
                ).with_entities([format!("app:{app_id}"), format!("agent:{agent_id}")])
            );
        }
    }
}
