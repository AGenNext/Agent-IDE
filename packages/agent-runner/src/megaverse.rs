// Autonomyx Megaverse — the unified world model.
//
// Every entity in the platform is a node in the megaverse:
//   agents, runs, peers, goals, plugins, fabric events, governance nodes,
//   applications, opt-ins, dashboards, decisions, arithmetic formulas,
//   blockchain records, storage artifacts, compute jobs, cluster members.
//
// Every relationship is an edge:
//   agent→run, run→step, peer→bridge, goal→objective, plugin→node,
//   event→artifact, node→node (govgraph), peer→peer (federation mesh).
//
// The megaverse is:
//   Live      — updates via fabric event subscription in real time
//   Unified   — single graph across all surfaces and all connected nodes
//   Traversable — BFS/DFS path queries between any two entities
//   Queryable — filter by kind, status, trust, region, capability
//   Federated — aggregates across all peers in the cluster mesh
//
// "The megaverse is the platform's mind. Everything that exists is a node.
//  Everything that happens is an edge." — openautonomyx.com

use std::collections::HashMap;
use std::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Node ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Agent,
    Run,
    Peer,
    Goal,
    Plugin,
    Event,
    GovNode,
    Application,
    OptIn,
    Dashboard,
    ComputeJob,
    StorageArtifact,
    BlockchainRecord,
    Formula,
    Credential,
    ClusterMember,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Agent            => "agent",
            NodeKind::Run              => "run",
            NodeKind::Peer             => "peer",
            NodeKind::Goal             => "goal",
            NodeKind::Plugin           => "plugin",
            NodeKind::Event            => "event",
            NodeKind::GovNode          => "gov_node",
            NodeKind::Application      => "application",
            NodeKind::OptIn            => "optin",
            NodeKind::Dashboard        => "dashboard",
            NodeKind::ComputeJob       => "compute_job",
            NodeKind::StorageArtifact  => "storage_artifact",
            NodeKind::BlockchainRecord => "blockchain_record",
            NodeKind::Formula          => "formula",
            NodeKind::Credential       => "credential",
            NodeKind::ClusterMember    => "cluster_member",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MegaverseNode {
    pub id:         String,
    pub kind:       NodeKind,
    pub label:      String,
    pub status:     String,
    pub trust:      f64,
    pub region:     Option<String>,
    pub did:        Option<String>,
    pub tags:       Vec<String>,
    pub meta:       serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source:     String,    // "local" | peer URL for federated nodes
}

// ── Edge ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MegaverseEdge {
    pub id:     String,
    pub from:   String,
    pub to:     String,
    pub kind:   String,    // "owns", "runs", "bridges", "governs", "produces", etc.
    pub weight: f64,
    pub meta:   serde_json::Value,
}

// ── Megaverse graph ───────────────────────────────────────────────────────────

pub struct Megaverse {
    nodes: RwLock<HashMap<String, MegaverseNode>>,
    edges: RwLock<Vec<MegaverseEdge>>,
    // adjacency list: node_id → list of (edge_kind, target_id)
    adj:   RwLock<HashMap<String, Vec<(String, String)>>>,
}

impl Megaverse {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(Vec::new()),
            adj:   RwLock::new(HashMap::new()),
        }
    }

    pub fn upsert(&self, node: MegaverseNode) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&self, from: &str, to: &str, kind: &str, weight: f64) {
        let edge = MegaverseEdge {
            id:     Uuid::new_v4().to_string(),
            from:   from.to_string(),
            to:     to.to_string(),
            kind:   kind.to_string(),
            weight,
            meta:   serde_json::json!({}),
        };
        self.edges.write().unwrap().push(edge);
        self.adj.write().unwrap()
            .entry(from.to_string())
            .or_default()
            .push((kind.to_string(), to.to_string()));
    }

    pub fn get(&self, id: &str) -> Option<MegaverseNode> {
        self.nodes.read().unwrap().get(id).cloned()
    }

    pub fn all_nodes(&self) -> Vec<MegaverseNode> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    pub fn all_edges(&self) -> Vec<MegaverseEdge> {
        self.edges.read().unwrap().clone()
    }

    pub fn nodes_by_kind(&self, kind: &NodeKind) -> Vec<MegaverseNode> {
        self.nodes.read().unwrap().values()
            .filter(|n| &n.kind == kind)
            .cloned().collect()
    }

    pub fn neighbors(&self, id: &str) -> Vec<(String, MegaverseNode)> {
        let adj = self.adj.read().unwrap();
        let nodes = self.nodes.read().unwrap();
        adj.get(id).cloned().unwrap_or_default()
            .into_iter()
            .filter_map(|(kind, target)| {
                nodes.get(&target).cloned().map(|n| (kind, n))
            })
            .collect()
    }

    /// BFS path between two nodes — returns the node IDs in traversal order.
    pub fn path(&self, from: &str, to: &str) -> Option<Vec<String>> {
        use std::collections::{VecDeque, HashSet};
        let adj = self.adj.read().unwrap();
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<Vec<String>> = VecDeque::new();
        queue.push_back(vec![from.to_string()]);
        while let Some(path) = queue.pop_front() {
            let current = path.last().unwrap().clone();
            if current == to { return Some(path); }
            if visited.contains(&current) { continue; }
            visited.insert(current.clone());
            for (_, next) in adj.get(&current).cloned().unwrap_or_default() {
                if !visited.contains(&next) {
                    let mut new_path = path.clone();
                    new_path.push(next);
                    queue.push_back(new_path);
                }
            }
        }
        None
    }

    /// Stats summary for the megaverse.
    pub fn summary(&self) -> serde_json::Value {
        let nodes = self.nodes.read().unwrap();
        let edges = self.edges.read().unwrap();

        let mut by_kind: HashMap<&str, usize> = HashMap::new();
        let mut trust_sum = 0.0f64;
        let mut federated = 0usize;
        for n in nodes.values() {
            *by_kind.entry(n.kind.as_str()).or_insert(0) += 1;
            trust_sum += n.trust;
            if n.source != "local" { federated += 1; }
        }

        serde_json::json!({
            "nodes":      nodes.len(),
            "edges":      edges.len(),
            "federated":  federated,
            "local":      nodes.len() - federated,
            "avg_trust":  if nodes.is_empty() { 0.0 } else { trust_sum / nodes.len() as f64 },
            "by_kind":    by_kind,
        })
    }

    /// Query nodes by free-text label match + optional kind filter.
    pub fn query(&self, q: &str, kind: Option<&str>, limit: usize) -> Vec<MegaverseNode> {
        let q_low = q.to_lowercase();
        let nodes = self.nodes.read().unwrap();
        let mut results: Vec<MegaverseNode> = nodes.values()
            .filter(|n| {
                let kind_ok = kind.map(|k| n.kind.as_str() == k).unwrap_or(true);
                let text_ok = q.is_empty()
                    || n.label.to_lowercase().contains(&q_low)
                    || n.id.contains(&q_low)
                    || n.tags.iter().any(|t| t.to_lowercase().contains(&q_low));
                kind_ok && text_ok
            })
            .cloned()
            .collect();
        // Sort by trust descending
        results.sort_by(|a, b| b.trust.partial_cmp(&a.trust).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }
}

// ── Platform → Megaverse indexer ─────────────────────────────────────────────
// Call index() to synchronise all platform state into the megaverse graph.
// Called at startup and by the reconciler on cadence.

use std::sync::Arc;
use crate::store::AppState;
use crate::fabric::FabricEvent;

/// Start the live megaverse fabric listener.
/// Subscribes to the fabric broadcast channel.
/// On every fabric event, re-indexes the touched entities into the megaverse.
/// No polling — the megaverse updates the instant the fabric moves.
pub fn start_live(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut rx = state.fabric.subscribe();
        loop {
            match rx.recv().await {
                Ok(raw) => {
                    if let Ok(ev) = serde_json::from_str::<FabricEvent>(&raw) {
                        // Update the artifact node in the megaverse
                        on_fabric_event(&state.megaverse, &state, &ev);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "megaverse: fabric lagged — partial live update");
                    // Full reindex to catch up
                    index(&state.megaverse, &state);
                }
                Err(_) => break,
            }
        }
    });
}

/// Process a single fabric event — update affected megaverse nodes immediately.
fn on_fabric_event(mv: &Arc<Megaverse>, state: &Arc<AppState>, ev: &FabricEvent) {
    // Update the primary artifact node
    update_node_from_event(mv, state, &ev.artifact);

    // Update all tagged entities
    for entity_id in &ev.entities {
        update_node_from_event(mv, state, entity_id);
    }
}

/// Refresh a single node in the megaverse from current platform state.
fn update_node_from_event(mv: &Arc<Megaverse>, state: &Arc<AppState>, id: &str) {
    // Determine entity kind from ID prefix convention
    if id.starts_with("agent:") || id.starts_with("agent_") {
        let agent_id = id.trim_start_matches("agent:").trim_start_matches("agent_");
        if let Some(a) = state.agents.read().unwrap().get(agent_id).cloned() {
            mv.upsert(MegaverseNode {
                id:         format!("agent:{}", a.id),
                kind:       NodeKind::Agent,
                label:      a.name.clone(),
                status:     a.status.clone(),
                trust:      0.8,
                region:     None,
                did:        None,
                tags:       a.capabilities.clone(),
                meta:       serde_json::json!({ "model": a.model }),
                created_at: a.created_at,
                updated_at: Utc::now(),
                source:     "local".into(),
            });
        }
    } else if id.starts_with("run:") {
        let run_id = id.trim_start_matches("run:");
        if let Some(r) = state.runs.read().unwrap().get(run_id).cloned() {
            mv.upsert(MegaverseNode {
                id:         format!("run:{}", r.run_id),
                kind:       NodeKind::Run,
                label:      format!("{} / {}", r.agent_name, r.task.chars().take(40).collect::<String>()),
                status:     format!("{:?}", r.status).to_lowercase(),
                trust:      if matches!(r.status, crate::store::RunStatus::Completed) { 1.0 } else { 0.5 },
                region:     None,
                did:        None,
                tags:       vec![r.model.clone()],
                meta:       serde_json::json!({ "steps": r.steps.len() }),
                created_at: r.started_at,
                updated_at: Utc::now(),
                source:     "local".into(),
            });
        }
    } else if id.starts_with("peer:") {
        let peer_id = id.trim_start_matches("peer:");
        if let Some(p) = state.peers.read().unwrap().get(peer_id).cloned() {
            mv.upsert(MegaverseNode {
                id:         format!("peer:{}", p.id),
                kind:       NodeKind::Peer,
                label:      p.name.clone(),
                status:     p.status.clone(),
                trust:      if p.status == "online" { 0.9 } else { 0.2 },
                region:     p.region.clone(),
                did:        None,
                tags:       vec!["federation".into()],
                meta:       serde_json::json!({ "url": p.url }),
                created_at: p.last_seen.unwrap_or_else(Utc::now),
                updated_at: Utc::now(),
                source:     "local".into(),
            });
        }
    }
    // For other kinds (reconciler, plugin, goal etc.) the next full index() will catch them
}

pub fn index(mv: &Arc<Megaverse>, state: &Arc<AppState>) {
    // Agents
    for a in state.agents.read().unwrap().values() {
        mv.upsert(MegaverseNode {
            id:         format!("agent:{}", a.id),
            kind:       NodeKind::Agent,
            label:      a.name.clone(),
            status:     a.status.clone(),
            trust:      0.8,
            region:     None,
            did:        None,
            tags:       a.capabilities.clone(),
            meta:       serde_json::json!({ "model": a.model, "owner": a.owner_id }),
            created_at: a.created_at,
            updated_at: a.updated_at,
            source:     "local".into(),
        });
    }

    // Runs
    for r in state.runs.read().unwrap().values() {
        let run_node_id = format!("run:{}", r.run_id);
        let agent_node_id = format!("agent:{}", r.agent_id);
        mv.upsert(MegaverseNode {
            id:         run_node_id.clone(),
            kind:       NodeKind::Run,
            label:      format!("{} / {}", r.agent_name, r.task.chars().take(40).collect::<String>()),
            status:     format!("{:?}", r.status).to_lowercase(),
            trust:      if matches!(r.status, crate::store::RunStatus::Completed) { 1.0 } else { 0.5 },
            region:     None,
            did:        None,
            tags:       vec![r.model.clone()],
            meta:       serde_json::json!({ "steps": r.steps.len(), "model": r.model }),
            created_at: r.started_at,
            updated_at: r.completed_at.unwrap_or(r.started_at),
            source:     "local".into(),
        });
        mv.add_edge(&agent_node_id, &run_node_id, "runs", 1.0);
    }

    // Peers
    for p in state.peers.read().unwrap().values() {
        mv.upsert(MegaverseNode {
            id:         format!("peer:{}", p.id),
            kind:       NodeKind::Peer,
            label:      p.name.clone(),
            status:     p.status.clone(),
            trust:      if p.status == "online" { 0.9 } else { 0.2 },
            region:     p.region.clone(),
            did:        None,
            tags:       vec!["federation".into()],
            meta:       serde_json::json!({ "url": p.url }),
            created_at: p.last_seen.unwrap_or_else(Utc::now),
            updated_at: p.last_seen.unwrap_or_else(Utc::now),
            source:     "local".into(),
        });
    }

    // Goals
    for g in state.goals.list() {
        let goal_node_id = format!("goal:{}", g.id);
        let agent_node_id = format!("agent:{}", g.agent_id);
        let alignment_score = g.alignment.as_ref()
            .map(|a| {
                let checks = [a.non_harm, a.transparent, a.consent_respecting,
                              a.accountable, a.reversible, a.net_positive, a.anti_extraction];
                checks.iter().filter(|&&v| v).count() as f64 / checks.len() as f64
            })
            .unwrap_or(0.5);
        mv.upsert(MegaverseNode {
            id:         goal_node_id.clone(),
            kind:       NodeKind::Goal,
            label:      g.title.clone(),
            status:     format!("{:?}", g.status).to_lowercase(),
            trust:      alignment_score,
            region:     None,
            did:        None,
            tags:       g.tags.clone(),
            meta:       serde_json::json!({ "impact": g.intended_impact }),
            created_at: g.created_at,
            updated_at: g.updated_at,
            source:     "local".into(),
        });
        mv.add_edge(&agent_node_id, &goal_node_id, "pursues", alignment_score);
    }

    // Plugins
    for p in state.plugins.list() {
        mv.upsert(MegaverseNode {
            id:         format!("plugin:{}", p.id),
            kind:       NodeKind::Plugin,
            label:      p.name.clone(),
            status:     if p.enabled { "enabled" } else { "disabled" }.into(),
            trust:      if p.enabled { 0.85 } else { 0.0 },
            region:     None,
            did:        None,
            tags:       p.capabilities.clone(),
            meta:       serde_json::json!({ "kind": p.kind, "version": p.version }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source:     "local".into(),
        });
    }

    // Governance graph nodes
    for n in state.govgraph.list_nodes() {
        mv.upsert(MegaverseNode {
            id:         format!("gov:{}", n.id),
            kind:       NodeKind::GovNode,
            label:      n.label.clone(),
            status:     "active".into(),
            trust:      n.trust_score,
            region:     None,
            did:        n.did.clone(),
            tags:       n.capabilities.clone(),
            meta:       serde_json::json!({ "requires": n.requires }),
            created_at: n.created_at,
            updated_at: n.updated_at,
            source:     "local".into(),
        });
    }

    // Opt-ins
    for o in state.optin.list() {
        mv.upsert(MegaverseNode {
            id:         format!("optin:{}", o.id),
            kind:       NodeKind::OptIn,
            label:      o.name.clone(),
            status:     format!("{:?}", o.status).to_lowercase(),
            trust:      0.75,
            region:     None,
            did:        Some(o.actor_did.clone()),
            tags:       vec![format!("{:?}", o.kind).to_lowercase()],
            meta:       o.metadata.clone(),
            created_at: o.submitted_at,
            updated_at: o.resolved_at.unwrap_or(o.submitted_at),
            source:     "local".into(),
        });
    }

    // Applications
    for a in state.apps.read().unwrap().values() {
        mv.upsert(MegaverseNode {
            id:         format!("app:{}", a.id),
            kind:       NodeKind::Application,
            label:      a.name.clone(),
            status:     format!("{:?}", a.status).to_lowercase(),
            trust:      0.8,
            region:     None,
            did:        a.did.clone(),
            tags:       vec![a.version.clone()],
            meta:       serde_json::json!({ "agents": a.agents }),
            created_at: a.created_at,
            updated_at: a.updated_at,
            source:     "local".into(),
        });
        // Wire agents into their app
        for agent_id in &a.agents {
            mv.add_edge(&format!("app:{}", a.id), &format!("agent:{agent_id}"), "owns", 1.0);
        }
    }

    // Cluster members (self + all peers)
    let self_url = std::env::var("AUTONOMYX_PUBLIC_URL")
        .unwrap_or_else(|_| "http://localhost:3001".into());
    mv.upsert(MegaverseNode {
        id:         "cluster:self".into(),
        kind:       NodeKind::ClusterMember,
        label:      std::env::var("AUTONOMYX_NODE_NAME").unwrap_or_else(|_| "autonomyx-node".into()),
        status:     "online".into(),
        trust:      1.0,
        region:     std::env::var("AUTONOMYX_REGION").ok(),
        did:        None,
        tags:       vec!["self".into()],
        meta:       serde_json::json!({ "url": self_url }),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        source:     "local".into(),
    });
    for p in state.peers.read().unwrap().values() {
        let member_id = format!("cluster:peer:{}", p.id);
        mv.upsert(MegaverseNode {
            id:         member_id.clone(),
            kind:       NodeKind::ClusterMember,
            label:      p.name.clone(),
            status:     p.status.clone(),
            trust:      if p.status == "online" { 0.9 } else { 0.1 },
            region:     p.region.clone(),
            did:        None,
            tags:       vec!["peer".into()],
            meta:       serde_json::json!({ "url": p.url }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source:     p.url.clone(),
        });
        mv.add_edge("cluster:self", &member_id, "bridges", 0.9);
    }

    tracing::debug!(
        nodes = mv.all_nodes().len(),
        edges = mv.all_edges().len(),
        "megaverse: indexed"
    );
}
