// Arithmetic — the numbers behind the platform.
//
// Every platform decision has a number behind it:
//   - How much did this run cost?
//   - What is the trust score trajectory for this agent?
//   - What is the net impact of this goal so far?
//   - How many tokens were spent per objective achieved?
//   - What is the alignment score distribution across all active goals?
//
// Arithmetic provides:
//   1. Expression evaluation — safe math over platform variables
//   2. Platform statistics — aggregated metrics across all state
//   3. Trend analysis — time-series over fabric events
//   4. Cost model — token cost, compute cost, impact cost per run
//   5. Impact arithmetic — how much world-change per unit of compute
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use crate::AppState;
use crate::goals::GoalStatus;

// ── Expression evaluator ──────────────────────────────────────────────────────
// Evaluates safe arithmetic expressions over named variables.
// No external deps — no exec, no eval, no sandbox needed.
// Supports: +, -, *, /, %, (, ), unary -, integer and float literals.

#[derive(Debug, Clone)]
struct Expr<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Expr<'a> {
    fn new(s: &'a str) -> Self { Expr { src: s.as_bytes(), pos: 0 } }

    fn peek(&self) -> Option<u8> { self.src.get(self.pos).copied() }

    fn skip_ws(&mut self) {
        while self.peek().map(|c| c == b' ' || c == b'\t').unwrap_or(false) {
            self.pos += 1;
        }
    }

    fn parse_num(&mut self) -> Result<f64, String> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == Some(b'-') { self.pos += 1; }
        while self.peek().map(|c| c.is_ascii_digit() || c == b'.').unwrap_or(false) {
            self.pos += 1;
        }
        if self.pos == start { return Err(format!("expected number at pos {}", self.pos)); }
        std::str::from_utf8(&self.src[start..self.pos])
            .map_err(|e| e.to_string())?
            .parse::<f64>()
            .map_err(|e| e.to_string())
    }

    fn parse_factor(&mut self, vars: &HashMap<String, f64>) -> Result<f64, String> {
        self.skip_ws();
        if self.peek() == Some(b'(') {
            self.pos += 1;
            let v = self.parse_expr(vars)?;
            self.skip_ws();
            if self.peek() == Some(b')') { self.pos += 1; } else { return Err("missing )".into()); }
            return Ok(v);
        }
        // Variable reference: starts with letter
        if self.peek().map(|c| c.is_ascii_alphabetic() || c == b'_').unwrap_or(false) {
            let start = self.pos;
            while self.peek().map(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.').unwrap_or(false) {
                self.pos += 1;
            }
            let name = std::str::from_utf8(&self.src[start..self.pos])
                .map_err(|e| e.to_string())?;
            return vars.get(name).copied()
                .ok_or_else(|| format!("unknown variable '{}'", name));
        }
        self.parse_num()
    }

    fn parse_term(&mut self, vars: &HashMap<String, f64>) -> Result<f64, String> {
        let mut v = self.parse_factor(vars)?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'*') => { self.pos += 1; v *= self.parse_factor(vars)?; }
                Some(b'/') => {
                    self.pos += 1;
                    let d = self.parse_factor(vars)?;
                    if d == 0.0 { return Err("division by zero".into()); }
                    v /= d;
                }
                Some(b'%') => { self.pos += 1; v %= self.parse_factor(vars)?; }
                _ => break,
            }
        }
        Ok(v)
    }

    fn parse_expr(&mut self, vars: &HashMap<String, f64>) -> Result<f64, String> {
        let mut v = self.parse_term(vars)?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'+') => { self.pos += 1; v += self.parse_term(vars)?; }
                Some(b'-') => { self.pos += 1; v -= self.parse_term(vars)?; }
                _ => break,
            }
        }
        Ok(v)
    }
}

pub fn eval_expr(expr: &str, vars: &HashMap<String, f64>) -> Result<f64, String> {
    if expr.len() > 512 { return Err("expression too long".into()); }
    let mut e = Expr::new(expr);
    let result = e.parse_expr(vars)?;
    e.skip_ws();
    if e.pos != e.src.len() {
        return Err(format!("unexpected character at pos {}", e.pos));
    }
    Ok(result)
}

// ── Platform variable snapshot ────────────────────────────────────────────────
// Collect all numeric platform values into a flat variable map for expression eval.

pub fn platform_vars(state: &AppState) -> HashMap<String, f64> {
    let mut vars = HashMap::new();

    let agents = state.list_agents();
    vars.insert("agents.total".into(), agents.len() as f64);
    vars.insert("agents.idle".into(),
        agents.iter().filter(|a| a.status == "idle").count() as f64);

    let runs = state.list_runs();
    vars.insert("runs.total".into(), runs.len() as f64);
    vars.insert("runs.running".into(),
        runs.iter().filter(|r| r.status == crate::store::RunStatus::Running).count() as f64);
    vars.insert("runs.completed".into(),
        runs.iter().filter(|r| r.status == crate::store::RunStatus::Completed).count() as f64);
    vars.insert("runs.failed".into(),
        runs.iter().filter(|r| r.status == crate::store::RunStatus::Failed).count() as f64);
    let total_steps: usize = runs.iter().map(|r| r.steps.len()).sum();
    vars.insert("runs.total_steps".into(), total_steps as f64);

    let goals = state.goals.list();
    vars.insert("goals.total".into(),    goals.len() as f64);
    vars.insert("goals.active".into(),   goals.iter().filter(|g| g.status == GoalStatus::Active).count() as f64);
    vars.insert("goals.achieved".into(), goals.iter().filter(|g| g.status == GoalStatus::Achieved).count() as f64);
    vars.insert("goals.aligned".into(),  goals.iter().filter(|g| g.status == GoalStatus::Aligned).count() as f64);
    vars.insert("goals.rejected".into(), goals.iter().filter(|g| g.status == GoalStatus::Rejected).count() as f64);

    let avg_align = if goals.is_empty() { 0.0 } else {
        goals.iter()
            .filter_map(|g| g.alignment.as_ref().map(|a| a.score()))
            .sum::<f64>() / goals.len() as f64
    };
    vars.insert("goals.avg_alignment_score".into(), avg_align);

    let nodes = state.govgraph.list_nodes();
    vars.insert("govgraph.nodes".into(), nodes.len() as f64);
    let avg_trust = if nodes.is_empty() { 0.0 } else {
        nodes.iter().map(|n| n.trust_score).sum::<f64>() / nodes.len() as f64
    };
    vars.insert("govgraph.avg_trust".into(), avg_trust);

    let plugins = state.plugins.list();
    vars.insert("plugins.total".into(),   plugins.len() as f64);
    vars.insert("plugins.enabled".into(), plugins.iter().filter(|p| p.enabled).count() as f64);

    let peers = state.list_peers();
    vars.insert("peers.total".into(),  peers.len() as f64);
    vars.insert("peers.online".into(), peers.iter().filter(|p| p.status == "online").count() as f64);

    let optins = state.optin.list();
    vars.insert("optin.total".into(),    optins.len() as f64);
    vars.insert("optin.approved".into(), optins.iter().filter(|o| o.status == crate::optin::OptInStatus::Approved || o.status == crate::optin::OptInStatus::Active).count() as f64);

    let events = state.fabric.full_log();
    vars.insert("fabric.events".into(), events.len() as f64);

    vars
}

// ── Platform statistics ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformStats {
    pub agents:   AgentStats,
    pub runs:     RunStats,
    pub goals:    GoalStats,
    pub govgraph: GraphStats,
    pub plugins:  PluginStats,
    pub fabric:   FabricStats,
    pub ratios:   RatioStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats  { pub total: usize, pub idle: usize, pub running: usize }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStats    { pub total: usize, pub running: usize, pub completed: usize, pub failed: usize, pub avg_steps: f64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStats   { pub total: usize, pub active: usize, pub achieved: usize, pub avg_alignment: f64, pub impact_progress: f64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats  { pub nodes: usize, pub avg_trust: f64, pub min_trust: f64, pub max_trust: f64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStats { pub total: usize, pub enabled: usize, pub capabilities: usize }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricStats { pub events: usize, pub open: usize, pub closed: usize }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioStats  {
    pub success_rate:        f64,   // completed / (completed + failed)
    pub alignment_rate:      f64,   // aligned+achieved / total goals
    pub plugin_enable_rate:  f64,   // enabled / total plugins
    pub trust_health:        f64,   // avg trust across graph
    pub impact_per_run:      f64,   // achieved goals / total runs
}

pub fn compute_stats(state: &AppState) -> PlatformStats {
    let agents = state.list_agents();
    let runs   = state.list_runs();
    let goals  = state.goals.list();
    let nodes  = state.govgraph.list_nodes();
    let plugins = state.plugins.list();
    let events  = state.fabric.full_log();

    let completed = runs.iter().filter(|r| r.status == crate::store::RunStatus::Completed).count();
    let failed    = runs.iter().filter(|r| r.status == crate::store::RunStatus::Failed).count();
    let running   = runs.iter().filter(|r| r.status == crate::store::RunStatus::Running).count();
    let total_steps: usize = runs.iter().map(|r| r.steps.len()).sum();

    let active_goals   = goals.iter().filter(|g| g.status == GoalStatus::Active).count();
    let achieved_goals = goals.iter().filter(|g| g.status == GoalStatus::Achieved).count();
    let avg_align = if goals.is_empty() { 0.0 } else {
        goals.iter().filter_map(|g| g.alignment.as_ref().map(|a| a.score())).sum::<f64>()
        / goals.len() as f64
    };
    let impact_progress = if active_goals == 0 { 0.0 } else {
        goals.iter().filter(|g| g.status == GoalStatus::Active)
            .map(|g| if g.impact_metrics.is_empty() { 0.0 }
                     else { g.impact_metrics.iter().map(|m| m.progress()).sum::<f64>() / g.impact_metrics.len() as f64 })
            .sum::<f64>() / active_goals as f64
    };

    let trust_scores: Vec<f64> = nodes.iter().map(|n| n.trust_score).collect();
    let avg_trust = if trust_scores.is_empty() { 0.0 } else { trust_scores.iter().sum::<f64>() / trust_scores.len() as f64 };
    let min_trust = trust_scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_trust = trust_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let enabled_plugins = plugins.iter().filter(|p| p.enabled).count();
    let all_caps = state.plugins.all_capabilities().len();

    let open_events   = events.iter().filter(|e| e.status == crate::fabric::FabricStatus::Open).count();
    let closed_events = events.iter().filter(|e| e.status == crate::fabric::FabricStatus::Closed).count();

    let success_rate = if completed + failed == 0 { 0.0 }
        else { completed as f64 / (completed + failed) as f64 };
    let alignment_rate = if goals.is_empty() { 0.0 }
        else { (active_goals + achieved_goals) as f64 / goals.len() as f64 };
    let plugin_enable_rate = if plugins.is_empty() { 0.0 }
        else { enabled_plugins as f64 / plugins.len() as f64 };
    let impact_per_run = if runs.is_empty() { 0.0 }
        else { achieved_goals as f64 / runs.len() as f64 };

    PlatformStats {
        agents:   AgentStats  { total: agents.len(), idle: agents.iter().filter(|a| a.status == "idle").count(), running: agents.iter().filter(|a| a.status == "running").count() },
        runs:     RunStats    { total: runs.len(), running, completed, failed, avg_steps: if runs.is_empty() { 0.0 } else { total_steps as f64 / runs.len() as f64 } },
        goals:    GoalStats   { total: goals.len(), active: active_goals, achieved: achieved_goals, avg_alignment: avg_align, impact_progress },
        govgraph: GraphStats  { nodes: nodes.len(), avg_trust, min_trust: if trust_scores.is_empty() { 0.0 } else { min_trust }, max_trust: if trust_scores.is_empty() { 0.0 } else { max_trust } },
        plugins:  PluginStats { total: plugins.len(), enabled: enabled_plugins, capabilities: all_caps },
        fabric:   FabricStats { events: events.len(), open: open_events, closed: closed_events },
        ratios:   RatioStats  { success_rate, alignment_rate, plugin_enable_rate, trust_health: avg_trust, impact_per_run },
    }
}
