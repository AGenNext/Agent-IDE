// Universal open search — one query across the entire platform world model.
//
// Searches: agents, goals, objectives, runs, artifacts, fabric events,
//           accountability records, governance nodes, plugins, peers, apps.
//
// Ranking: exact id match > name prefix > capability/tag match > description substring.
// All searches are in-memory; no external index required.
//
// "Universal open search" — openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub kind:    String,
    pub id:      String,
    pub title:   String,
    pub excerpt: String,
    pub score:   f64,
    pub source:  String,
    pub meta:    Value,
}

pub struct SearchQuery {
    pub q:      String,
    pub limit:  usize,
    pub kinds:  Option<Vec<String>>,
}

impl SearchQuery {
    pub fn wants(&self, kind: &str) -> bool {
        match &self.kinds {
            None       => true,
            Some(list) => list.iter().any(|k| k == kind),
        }
    }
}

// Compute relevance score for a candidate against the query terms.
// Returns 0.0 if no match, higher = better.
fn score(q: &str, id: &str, title: &str, body: &str, tags: &[&str]) -> f64 {
    let ql = q.to_lowercase();
    let terms: Vec<&str> = ql.split_whitespace().collect();
    if terms.is_empty() { return 0.0; }

    let id_l    = id.to_lowercase();
    let title_l = title.to_lowercase();
    let body_l  = body.to_lowercase();

    let mut s: f64 = 0.0;
    for t in &terms {
        if id_l == *t                  { s += 3.0; }
        else if id_l.contains(t)       { s += 2.0; }
        if title_l.starts_with(t)      { s += 2.5; }
        else if title_l.contains(t)    { s += 1.5; }
        if body_l.contains(t)          { s += 0.5; }
        for tag in tags {
            if tag.to_lowercase().contains(t) { s += 1.0; }
        }
    }
    // Normalise by number of terms so multi-term queries don't unfairly beat single
    s / (terms.len() as f64).max(1.0)
}

pub async fn search(q: SearchQuery, state: Arc<AppState>) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = Vec::new();

    // ── Agents ────────────────────────────────────────────────────────────────
    if q.wants("agent") {
        for a in state.list_agents() {
            let s = score(&q.q, &a.id, &a.name,
                          &format!("{} {} {}", a.description, a.model, a.status),
                          &a.capabilities.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "agent".into(),
                    id:      a.id.clone(),
                    title:   a.name.clone(),
                    excerpt: a.description.clone(),
                    score:   s,
                    source:  "agents".into(),
                    meta: json!({ "model": a.model, "status": a.status, "capabilities": a.capabilities }),
                });
            }
        }
    }

    // ── Runs ──────────────────────────────────────────────────────────────────
    if q.wants("run") {
        for r in state.list_runs() {
            let s = score(&q.q, &r.run_id, &r.agent_name,
                          &format!("{} {:?}", r.task, r.status),
                          &[r.model.as_str()]);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "run".into(),
                    id:      r.run_id.clone(),
                    title:   format!("{} — {}", r.agent_name, r.task),
                    excerpt: format!("{:?} • {} steps", r.status, r.steps.len()),
                    score:   s,
                    source:  "runs".into(),
                    meta: json!({ "agent_id": r.agent_id, "model": r.model, "status": r.status }),
                });
            }
        }
    }

    // ── Apps ──────────────────────────────────────────────────────────────────
    if q.wants("app") {
        for a in state.list_apps() {
            let s = score(&q.q, &a.id, &a.name,
                          &format!("{} {:?} {}", a.description, a.status, a.version),
                          &[]);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "app".into(),
                    id:      a.id.clone(),
                    title:   a.name.clone(),
                    excerpt: a.description.clone(),
                    score:   s,
                    source:  "apps".into(),
                    meta: json!({ "version": a.version, "status": a.status, "did": a.did }),
                });
            }
        }
    }

    // ── Peers ─────────────────────────────────────────────────────────────────
    if q.wants("peer") {
        for p in state.list_peers() {
            let s = score(&q.q, &p.id, &p.name,
                          &format!("{} {:?}", p.url, p.region),
                          &[p.status.as_str()]);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "peer".into(),
                    id:      p.id.clone(),
                    title:   p.name.clone(),
                    excerpt: p.url.clone(),
                    score:   s,
                    source:  "peers".into(),
                    meta: json!({ "url": p.url, "status": p.status, "region": p.region }),
                });
            }
        }
    }

    // ── Goals ─────────────────────────────────────────────────────────────────
    if q.wants("goal") {
        for g in state.goals.list() {
            let tags: Vec<&str> = g.tags.iter().map(|s| s.as_str()).collect();
            let s = score(&q.q, &g.id, &g.title,
                          &format!("{} {} {:?}", g.description, g.intended_impact, g.status),
                          &tags);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "goal".into(),
                    id:      g.id.clone(),
                    title:   g.title.clone(),
                    excerpt: g.description.clone(),
                    score:   s,
                    source:  "goals".into(),
                    meta: json!({ "status": g.status, "agent_id": g.agent_id, "tags": g.tags }),
                });
            }
        }
    }

    // ── Governance graph nodes ─────────────────────────────────────────────────
    if q.wants("node") {
        for n in state.govgraph.list_nodes() {
            let tags: Vec<&str> = n.capabilities.iter().map(|s| s.as_str()).collect();
            let s = score(&q.q, &n.id, &n.label,
                          &format!("{:?} {}", n.kind, n.requires.join(" ")),
                          &tags);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "node".into(),
                    id:      n.id.clone(),
                    title:   n.label.clone(),
                    excerpt: format!("{:?} • trust {:.2}", n.kind, n.trust_score),
                    score:   s,
                    source:  "govgraph".into(),
                    meta: json!({ "kind": n.kind, "trust_score": n.trust_score, "did": n.did }),
                });
            }
        }
    }

    // ── Plugins ────────────────────────────────────────────────────────────────
    if q.wants("plugin") {
        for p in state.plugins.list() {
            let tags: Vec<&str> = p.capabilities.iter().map(|s| s.as_str()).collect();
            let s = score(&q.q, &p.id, &p.name,
                          &format!("{} {:?} {}", p.description, p.kind, p.author),
                          &tags);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "plugin".into(),
                    id:      p.id.clone(),
                    title:   p.name.clone(),
                    excerpt: p.description.clone(),
                    score:   s,
                    source:  "plugins".into(),
                    meta: json!({ "kind": p.kind, "enabled": p.enabled, "version": p.version, "homepage": p.homepage }),
                });
            }
        }
    }

    // ── Fabric events (recent 200) ─────────────────────────────────────────────
    if q.wants("event") {
        for e in state.fabric.full_log().into_iter().rev().take(200) {
            let s = score(&q.q, &e.id, &e.artifact,
                          &format!("{:?} {:?} {:?}", e.stage, e.status, e.payload),
                          &[]);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "event".into(),
                    id:      e.id.clone(),
                    title:   format!("{:?} → {:?}", e.stage, e.status),
                    excerpt: format!("artifact: {}", e.artifact),
                    score:   s,
                    source:  "fabric".into(),
                    meta: json!({ "stage": e.stage, "status": e.status, "artifact": e.artifact }),
                });
            }
        }
    }

    // ── Accountability records (recent 200) ──────────────────────────────────
    if q.wants("record") {
        for r in state.federation.full_audit_log().into_iter().rev().take(200) {
            let s = score(&q.q, &r.id, &r.action,
                          &format!("{} {} {:?}", r.did, r.resource, r.outcome),
                          &[]);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "record".into(),
                    id:      r.id.clone(),
                    title:   format!("{} on {}", r.action, r.resource),
                    excerpt: format!("actor: {} • {:?}", r.did, r.outcome),
                    score:   s,
                    source:  "federation".into(),
                    meta: json!({ "did": r.did, "action": r.action, "outcome": r.outcome }),
                });
            }
        }
    }

    // ── Dashboards ────────────────────────────────────────────────────────────
    if q.wants("dashboard") {
        for d in state.dashboards.list(None) {
            let widget_text: String = d.widgets.iter()
                .map(|w| format!("{} {:?}", w.title, w.kind))
                .collect::<Vec<_>>().join(" ");
            let s = score(&q.q, &d.id, &d.name,
                          &format!("{} {}", d.description, widget_text),
                          &[]);
            if s > 0.0 {
                results.push(SearchResult {
                    kind:    "dashboard".into(),
                    id:      d.id.clone(),
                    title:   d.name.clone(),
                    excerpt: d.description.clone(),
                    score:   s,
                    source:  "dashboards".into(),
                    meta: json!({ "widget_count": d.widgets.len(), "owner": d.owner_did }),
                });
            }
        }
    }

    // Sort: highest score first; for ties, prefer shorter id (more specific)
    results.sort_by(|a, b| b.score.partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(a.id.len().cmp(&b.id.len())));

    results.truncate(q.limit);
    results
}
