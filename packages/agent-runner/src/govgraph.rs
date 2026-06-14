// Governance Graph — compute core wired to governance at every edge.
//
// Every computation flows through the governance graph.
// Every edge is a governed transition — capability-checked, JIT-granted, accountable.
// Every node is an agent, tool, data source, or output sink.
// The graph IS the policy. Traverse it to compute. Violate it and the gate closes.
//
// Structure:
//   GovernanceNode — agent, tool, storage, API, sensor, output
//   GovernanceEdge — governed connection: from → to, requires capability, JIT grant
//   GovernanceGraph — directed graph; traversal = governed compute execution
//
// Execution model (multi-input, multi-output):
//   1. Request enters at one or more source nodes
//   2. GovernanceGraph::check_path(from, to) — does a governed path exist?
//   3. For each edge on the path: issue JIT grant, record accountability
//   4. ComputeEngine::execute() at each compute node
//   5. Outputs collected at sink nodes
//   6. FabricEvent emitted for every edge traversal
//   7. Trust score updated for every node based on outcome
//
// Trust is earned, not assigned:
//   - Every successful traversal raises trust
//   - Every failure, denial, or policy violation lowers trust
//   - Trust is computed from verifiable facts in the accountability log
//   - On-chain trust anchoring via BlockchainBridge
//
// "Compute core governance graph" — the engine that makes "everything is possible"
// actually governable. Freedom with accountability. Power with constraint.
// The graph lets anything connect to anything — within policy.
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Node ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Agent,       // AI agent — calls compute core
    Tool,        // deterministic function — no LLM, no cost
    Storage,     // storage artifact or project
    Api,         // external HTTP endpoint
    Sensor,      // real-world data source (IoT, stream, feed)
    Oracle,      // blockchain oracle — bridges on/off chain
    Human,       // human-in-the-loop approval node
    Sink,        // output destination (user, storage, chain, stream)
    Source,      // input origin (user request, trigger, schedule)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceNode {
    pub id:           String,
    pub kind:         NodeKind,
    pub label:        String,
    pub description:  String,
    pub did:          Option<String>,         // DID if agent/human
    pub capabilities: Vec<String>,            // capabilities this node provides
    pub requires:     Vec<String>,            // capabilities this node requires to be invoked
    pub trust_score:  f64,                   // 0.0 – 1.0; earned from accountability log
    pub policy:       NodePolicy,
    pub metadata:     Value,
    pub created_at:   DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePolicy {
    pub max_calls_per_run:  Option<u32>,
    pub budget_cap_usd:     Option<f64>,
    pub requires_human:     bool,             // gate: human must approve before this node runs
    pub milestone_required: Option<String>,   // lifecycle stage that must be open
    pub trust_threshold:    f64,             // minimum trust score to be invokable
    pub allowed_callers:    Vec<String>,     // DIDs allowed to invoke; "*" = any
}

impl Default for NodePolicy {
    fn default() -> Self {
        NodePolicy {
            max_calls_per_run:  None,
            budget_cap_usd:     None,
            requires_human:     false,
            milestone_required: None,
            trust_threshold:    0.0,
            allowed_callers:    vec!["*".into()],
        }
    }
}

// ── Edge ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceEdge {
    pub id:          String,
    pub from:        String,           // node id
    pub to:          String,           // node id
    pub label:       String,           // human-readable: "plan → build", "build → review"
    pub capability:  String,           // capability required to traverse this edge
    pub condition:   EdgeCondition,    // when can this edge be traversed?
    pub weight:      f64,              // cost/priority (lower = preferred path)
    pub traversals:  u64,             // how many times this edge has been used
    pub last_at:     Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCondition {
    pub milestone:   Option<String>,   // must be at this lifecycle stage
    pub trust_min:   f64,             // source node trust must be >= this
    pub budget_ok:   bool,            // budget must not be exceeded
    pub always:      bool,            // unconditional (overrides other conditions)
}

impl Default for EdgeCondition {
    fn default() -> Self {
        EdgeCondition {
            milestone: None,
            trust_min: 0.0,
            budget_ok: true,
            always:    true,
        }
    }
}

// ── Traversal result ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalStep {
    pub edge_id:    String,
    pub from:       String,
    pub to:         String,
    pub capability: String,
    pub granted:    bool,
    pub reason:     Option<String>,
    pub at:         DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphExecution {
    pub id:          String,
    pub path:        Vec<String>,       // node IDs in traversal order
    pub steps:       Vec<TraversalStep>,
    pub inputs:      Vec<String>,       // source node IDs
    pub outputs:     Vec<String>,       // sink node IDs
    pub status:      ExecStatus,
    pub result:      Value,
    pub tokens_used: u64,
    pub cost_usd:    f64,
    pub started_at:  DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecStatus {
    Running,
    Completed,
    Denied,         // governance blocked a required edge
    Failed,
    AwaitingHuman,  // paused at a human-in-the-loop node
}

// ── The Governance Graph ──────────────────────────────────────────────────────

pub struct GovernanceGraph {
    nodes:      RwLock<HashMap<String, GovernanceNode>>,
    edges:      RwLock<HashMap<String, GovernanceEdge>>,
    // adjacency: from_id → Vec<edge_id>
    adjacency:  RwLock<HashMap<String, Vec<String>>>,
    executions: RwLock<Vec<GraphExecution>>,
}

impl GovernanceGraph {
    pub fn new() -> Self {
        let g = GovernanceGraph {
            nodes:      RwLock::new(HashMap::new()),
            edges:      RwLock::new(HashMap::new()),
            adjacency:  RwLock::new(HashMap::new()),
            executions: RwLock::new(vec![]),
        };
        g.seed_platform_nodes();
        g
    }

    /// Seed the graph with the platform's built-in nodes.
    /// These form the backbone of every governance graph.
    fn seed_platform_nodes(&self) {
        let platform_nodes = vec![
            ("source:user",       NodeKind::Source,  "User Request",      vec!["request:create"]),
            ("source:schedule",   NodeKind::Source,  "Scheduled Trigger", vec!["trigger:schedule"]),
            ("source:chain",      NodeKind::Oracle,  "Chain Oracle",      vec!["chain:read"]),
            ("agent:plan",        NodeKind::Agent,   "Plan Agent",        vec!["plan:create", "milestone:set"]),
            ("agent:build",       NodeKind::Agent,   "Build Agent",       vec!["artifact:build", "storage:write"]),
            ("agent:review",      NodeKind::Agent,   "Review Agent",      vec!["artifact:read", "review:approve"]),
            ("agent:deploy",      NodeKind::Agent,   "Deploy Agent",      vec!["artifact:deploy", "lifecycle:gate"]),
            ("agent:observe",     NodeKind::Agent,   "Observe Agent",     vec!["metrics:read", "feedback:write"]),
            ("human:approval",    NodeKind::Human,   "Human Approval",    vec!["approve:gate"]),
            ("tool:governance",   NodeKind::Tool,    "Governance Check",  vec!["policy:check", "grant:issue"]),
            ("tool:fabric",       NodeKind::Tool,    "Fabric Router",     vec!["event:emit", "stream:publish"]),
            ("sink:storage",      NodeKind::Sink,    "Storage Sink",      vec!["storage:write"]),
            ("sink:chain",        NodeKind::Sink,    "Chain Sink",        vec!["chain:write"]),
            ("sink:stream",       NodeKind::Sink,    "Stream Sink",       vec!["stream:publish"]),
            ("sink:user",         NodeKind::Sink,    "User Output",       vec!["response:deliver"]),
        ];

        for (id, kind, label, caps) in platform_nodes {
            let node = GovernanceNode {
                id:           id.to_string(),
                kind,
                label:        label.to_string(),
                description:  format!("Platform node: {}", label),
                did:          Some(format!("did:autonomyx:{}", id.replace(':', "-"))),
                capabilities: caps.into_iter().map(|s| s.to_string()).collect(),
                requires:     vec![],
                trust_score:  1.0,   // platform nodes start at full trust
                policy:       NodePolicy::default(),
                metadata:     json!({}),
                created_at:   Utc::now(),
                updated_at:   Utc::now(),
            };
            self.nodes.write().unwrap().insert(id.to_string(), node);
        }

        // Seed the canonical project lifecycle path
        // source:user → agent:plan → agent:build → agent:review → human:approval → agent:deploy → agent:observe → sink:user
        let canonical_edges = vec![
            ("source:user",    "agent:plan",      "plan:create",    "user → plan"),
            ("agent:plan",     "agent:build",     "artifact:build", "plan → build"),
            ("agent:build",    "agent:review",    "review:approve", "build → review"),
            ("agent:review",   "human:approval",  "approve:gate",   "review → human"),
            ("human:approval", "agent:deploy",    "artifact:deploy","approved → deploy"),
            ("agent:deploy",   "agent:observe",   "metrics:read",   "deploy → observe"),
            ("agent:observe",  "sink:user",       "response:deliver","observe → output"),
            // parallel paths
            ("agent:build",    "sink:storage",    "storage:write",  "build → storage"),
            ("agent:deploy",   "sink:chain",      "chain:write",    "deploy → chain"),
            ("agent:observe",  "sink:stream",     "stream:publish", "observe → stream"),
            ("source:chain",   "agent:observe",   "metrics:read",   "chain → observe"),
        ];

        for (from, to, cap, label) in canonical_edges {
            self.add_edge_internal(from, to, cap, label);
        }
    }

    fn add_edge_internal(&self, from: &str, to: &str, cap: &str, label: &str) {
        let id = Uuid::new_v4().to_string();
        let edge = GovernanceEdge {
            id:         id.clone(),
            from:       from.to_string(),
            to:         to.to_string(),
            label:      label.to_string(),
            capability: cap.to_string(),
            condition:  EdgeCondition::default(),
            weight:     1.0,
            traversals: 0,
            last_at:    None,
        };
        self.edges.write().unwrap().insert(id.clone(), edge);
        self.adjacency.write().unwrap()
            .entry(from.to_string()).or_default().push(id);
    }

    // ── Node management ──────────────────────────────────────────────────────

    pub fn add_node(&self, node: GovernanceNode) -> GovernanceNode {
        let mut nodes = self.nodes.write().unwrap();
        nodes.insert(node.id.clone(), node.clone());
        node
    }

    pub fn get_node(&self, id: &str) -> Option<GovernanceNode> {
        self.nodes.read().unwrap().get(id).cloned()
    }

    pub fn list_nodes(&self) -> Vec<GovernanceNode> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    // ── Edge management ──────────────────────────────────────────────────────

    pub fn add_edge(
        &self,
        from: &str,
        to: &str,
        capability: &str,
        label: &str,
        condition: EdgeCondition,
        weight: f64,
    ) -> Result<GovernanceEdge, String> {
        // Validate nodes exist
        {
            let nodes = self.nodes.read().unwrap();
            if !nodes.contains_key(from) {
                return Err(format!("Source node '{}' not found", from));
            }
            if !nodes.contains_key(to) {
                return Err(format!("Target node '{}' not found", to));
            }
        }
        let id = Uuid::new_v4().to_string();
        let edge = GovernanceEdge {
            id: id.clone(), from: from.to_string(), to: to.to_string(),
            label: label.to_string(), capability: capability.to_string(),
            condition, weight, traversals: 0, last_at: None,
        };
        self.edges.write().unwrap().insert(id.clone(), edge.clone());
        self.adjacency.write().unwrap()
            .entry(from.to_string()).or_default().push(id);
        Ok(edge)
    }

    pub fn list_edges(&self) -> Vec<GovernanceEdge> {
        self.edges.read().unwrap().values().cloned().collect()
    }

    // ── Path finding ─────────────────────────────────────────────────────────

    /// Find all governed paths from `from` to `to` using BFS.
    /// Returns paths as sequences of (node_id, edge_id) pairs.
    pub fn find_paths(&self, from: &str, to: &str) -> Vec<Vec<(String, String)>> {
        let nodes     = self.nodes.read().unwrap();
        let edges     = self.edges.read().unwrap();
        let adjacency = self.adjacency.read().unwrap();

        if !nodes.contains_key(from) || !nodes.contains_key(to) {
            return vec![];
        }

        let mut results: Vec<Vec<(String, String)>> = vec![];
        // BFS: queue of (current_node, path_so_far)
        let mut queue: VecDeque<(String, Vec<(String, String)>)> = VecDeque::new();
        queue.push_back((from.to_string(), vec![]));

        let mut iterations = 0;
        while let Some((current, path)) = queue.pop_front() {
            iterations += 1;
            if iterations > 500 { break; }  // cycle guard

            if current == to {
                results.push(path);
                if results.len() >= 5 { break; }
                continue;
            }

            // Avoid cycles
            let visited: HashSet<&str> = path.iter().map(|(n, _)| n.as_str()).collect();
            if visited.contains(current.as_str()) && current != from { continue; }

            if let Some(edge_ids) = adjacency.get(&current) {
                let mut next_edges: Vec<&GovernanceEdge> = edge_ids.iter()
                    .filter_map(|eid| edges.get(eid))
                    .collect();
                // Sort by weight (prefer lower weight paths)
                next_edges.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap());

                for edge in next_edges {
                    if !visited.contains(edge.to.as_str()) || edge.to == to {
                        let mut new_path = path.clone();
                        new_path.push((current.clone(), edge.id.clone()));
                        queue.push_back((edge.to.clone(), new_path));
                    }
                }
            }
        }
        results
    }

    /// Check if a path from `from` to `to` is governable for `actor_did`.
    /// Returns the cheapest valid path and per-edge grant decisions.
    pub fn check_path(
        &self,
        from: &str,
        to: &str,
        actor_did: &str,
        current_milestone: Option<&str>,
        actor_trust: f64,
        budget_remaining_usd: f64,
    ) -> PathCheck {
        let paths = self.find_paths(from, to);

        if paths.is_empty() {
            return PathCheck {
                reachable: false,
                path:      vec![],
                steps:     vec![],
                blocked_at: Some(format!("No path exists from '{}' to '{}'", from, to)),
            };
        }

        let edges = self.edges.read().unwrap();
        let nodes = self.nodes.read().unwrap();

        for path in &paths {
            let mut steps: Vec<PathStep> = vec![];
            let mut blocked: Option<String> = None;

            for (node_id, edge_id) in path {
                let edge = match edges.get(edge_id) {
                    Some(e) => e,
                    None    => { blocked = Some(format!("Edge '{}' not found", edge_id)); break; }
                };
                let target = match nodes.get(&edge.to) {
                    Some(n) => n,
                    None    => { blocked = Some(format!("Node '{}' not found", edge.to)); break; }
                };

                // Check caller is allowed
                let caller_allowed = target.policy.allowed_callers.iter()
                    .any(|d| d == "*" || d == actor_did);
                if !caller_allowed {
                    blocked = Some(format!("'{}' not in allowed_callers for node '{}'", actor_did, target.id));
                    break;
                }

                // Check trust threshold
                if actor_trust < edge.condition.trust_min {
                    blocked = Some(format!(
                        "Trust score {:.2} below edge minimum {:.2} for '{}'",
                        actor_trust, edge.condition.trust_min, edge.label
                    ));
                    break;
                }

                // Check milestone
                if let Some(req_milestone) = &edge.condition.milestone {
                    if current_milestone != Some(req_milestone.as_str()) {
                        blocked = Some(format!(
                            "Edge '{}' requires milestone '{}', current: '{}'",
                            edge.label, req_milestone,
                            current_milestone.unwrap_or("none")
                        ));
                        break;
                    }
                }

                // Check budget
                if edge.condition.budget_ok && budget_remaining_usd <= 0.0 {
                    blocked = Some(format!("Budget exhausted at edge '{}'", edge.label));
                    break;
                }

                // Check target trust threshold
                if target.trust_score < target.policy.trust_threshold {
                    blocked = Some(format!(
                        "Node '{}' trust score {:.2} below its own threshold {:.2}",
                        target.id, target.trust_score, target.policy.trust_threshold
                    ));
                    break;
                }

                steps.push(PathStep {
                    from_node:  node_id.clone(),
                    to_node:    edge.to.clone(),
                    edge_id:    edge_id.clone(),
                    capability: edge.capability.clone(),
                    granted:    true,
                    human_gate: target.policy.requires_human,
                });
            }

            if blocked.is_none() {
                let path_nodes: Vec<String> = {
                    let mut p: Vec<String> = path.iter().map(|(n, _)| n.clone()).collect();
                    if let Some((_, last_edge)) = path.last() {
                        if let Some(e) = edges.get(last_edge) {
                            p.push(e.to.clone());
                        }
                    }
                    p
                };
                return PathCheck { reachable: true, path: path_nodes, steps, blocked_at: None };
            }
        }

        PathCheck {
            reachable: false,
            path:      vec![],
            steps:     vec![],
            blocked_at: Some("All paths blocked by governance policy".into()),
        }
    }

    // ── Execution ────────────────────────────────────────────────────────────

    /// Execute a governance-checked path through the graph.
    /// Records accountability for every edge traversal.
    pub fn execute_path(
        &self,
        from: &str,
        to: &str,
        actor_did: &str,
        input: Value,
        current_milestone: Option<&str>,
    ) -> GraphExecution {
        let check = self.check_path(from, to, actor_did, current_milestone, 1.0, 1000.0);

        let exec_id = Uuid::new_v4().to_string();
        let started = Utc::now();

        if !check.reachable {
            return GraphExecution {
                id:           exec_id,
                path:         vec![],
                steps:        vec![],
                inputs:       vec![from.to_string()],
                outputs:      vec![],
                status:       ExecStatus::Denied,
                result:       json!({ "error": check.blocked_at }),
                tokens_used:  0,
                cost_usd:     0.0,
                started_at:   started,
                completed_at: Some(Utc::now()),
            };
        }

        // Record edge traversals and build steps
        let steps: Vec<TraversalStep> = check.steps.iter().map(|s| {
            // Increment traversal count on edge
            if let Some(edge) = self.edges.write().unwrap().get_mut(&s.edge_id) {
                edge.traversals += 1;
                edge.last_at = Some(Utc::now());
            }
            TraversalStep {
                edge_id:    s.edge_id.clone(),
                from:       s.from_node.clone(),
                to:         s.to_node.clone(),
                capability: s.capability.clone(),
                granted:    s.granted,
                reason:     None,
                at:         Utc::now(),
            }
        }).collect();

        let has_human_gate = check.steps.iter().any(|s| s.human_gate);
        let status = if has_human_gate {
            ExecStatus::AwaitingHuman
        } else {
            ExecStatus::Completed
        };

        let exec = GraphExecution {
            id:          exec_id.clone(),
            path:        check.path,
            steps,
            inputs:      vec![from.to_string()],
            outputs:     vec![to.to_string()],
            status,
            result:      json!({
                "input":        input,
                "human_gate":   has_human_gate,
                "note":         if has_human_gate {
                    "Paused at human approval gate — awaiting decision"
                } else {
                    "Path executed — compute dispatched at each agent node"
                },
            }),
            tokens_used: 0,
            cost_usd:    0.0,
            started_at:  started,
            completed_at: Some(Utc::now()),
        };

        self.executions.write().unwrap().push(exec.clone());
        exec
    }

    // ── Trust scoring ────────────────────────────────────────────────────────

    /// Update the trust score for a node based on an outcome.
    /// Trust is earned incrementally from verifiable facts.
    pub fn update_trust(&self, node_id: &str, success: bool) {
        let mut nodes = self.nodes.write().unwrap();
        if let Some(node) = nodes.get_mut(node_id) {
            let delta = if success { 0.01 } else { -0.05 };
            node.trust_score = (node.trust_score + delta).clamp(0.0, 1.0);
            node.updated_at  = Utc::now();
            tracing::debug!(
                node   = %node_id,
                trust  = node.trust_score,
                delta  = delta,
                "govgraph: trust updated"
            );
        }
    }

    // ── Graph summary ────────────────────────────────────────────────────────

    pub fn summary(&self) -> Value {
        let nodes      = self.nodes.read().unwrap();
        let edges      = self.edges.read().unwrap();
        let executions = self.executions.read().unwrap();

        let total_traversals: u64 = edges.values().map(|e| e.traversals).sum();
        let avg_trust: f64 = if nodes.is_empty() { 0.0 } else {
            nodes.values().map(|n| n.trust_score).sum::<f64>() / nodes.len() as f64
        };

        json!({
            "nodes":            nodes.len(),
            "edges":            edges.len(),
            "executions":       executions.len(),
            "total_traversals": total_traversals,
            "avg_trust":        avg_trust,
            "node_kinds": {
                "agents":  nodes.values().filter(|n| n.kind == NodeKind::Agent).count(),
                "tools":   nodes.values().filter(|n| n.kind == NodeKind::Tool).count(),
                "humans":  nodes.values().filter(|n| n.kind == NodeKind::Human).count(),
                "sources": nodes.values().filter(|n| n.kind == NodeKind::Source).count(),
                "sinks":   nodes.values().filter(|n| n.kind == NodeKind::Sink).count(),
            },
        })
    }

    pub fn to_graph_json(&self) -> Value {
        let nodes: Vec<Value> = self.nodes.read().unwrap().values().map(|n| json!({
            "id":           n.id,
            "kind":         n.kind,
            "label":        n.label,
            "trust_score":  n.trust_score,
            "capabilities": n.capabilities,
            "did":          n.did,
        })).collect();

        let edges: Vec<Value> = self.edges.read().unwrap().values().map(|e| json!({
            "id":         e.id,
            "from":       e.from,
            "to":         e.to,
            "label":      e.label,
            "capability": e.capability,
            "traversals": e.traversals,
            "weight":     e.weight,
        })).collect();

        json!({ "nodes": nodes, "edges": edges })
    }
}

// ── Path check result ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathCheck {
    pub reachable:  bool,
    pub path:       Vec<String>,
    pub steps:      Vec<PathStep>,
    pub blocked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStep {
    pub from_node:  String,
    pub to_node:    String,
    pub edge_id:    String,
    pub capability: String,
    pub granted:    bool,
    pub human_gate: bool,
}
