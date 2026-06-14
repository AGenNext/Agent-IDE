// Goals — agents with purpose.
//
// "Agents with a goal to make the world better."
//
// An agent without a goal is just a program.
// A goal without alignment is just ambition.
// A goal with alignment, accountability, and measurable impact — that changes the world.
//
// Goal hierarchy:
//   Mission     — why the agent exists (values-rooted, enduring)
//   Goal        — what the agent is trying to achieve (specific, measurable, time-bound)
//   Objective   — concrete milestone toward the goal
//   Task        — action the agent takes to advance an objective
//   Impact      — measured change in the world (not just technical output)
//
// Alignment check — before any goal is accepted, the platform checks:
//   ✓ Does it improve, not harm?             (non-harm principle)
//   ✓ Is the intent transparent?             (no hidden agendas)
//   ✓ Does it respect consent?               (no coercive actions)
//   ✓ Are outcomes verifiable and auditable? (accountability)
//   ✓ Is it reversible where possible?       (prefer undoable over permanent)
//   ✓ Does it benefit more than it costs?    (net positive)
//   ✓ Does it distribute value, not extract? (anti-extraction)
//
// The governance graph enforces alignment structurally.
// Every edge the agent traverses was allowed by policy.
// Every action is an accountable fact in the ledger.
// The feedback gate closes the loop: did the goal move the world forward?
//
// "Everything is possible" — when goals are aligned, agents are governed,
// and the feedback loop runs. The platform exists to make impossible goals reachable.
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Alignment ─────────────────────────────────────────────────────────────────

/// The seven alignment checks every goal must pass.
/// Not rules imposed from outside — values built into the protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentCheck {
    pub non_harm:        bool,  // improves the world, does not harm
    pub transparent:     bool,  // intent is stated and visible
    pub consent_respecting: bool, // no coercion of humans or agents
    pub accountable:     bool,  // outcomes verifiable in the accountability log
    pub reversible:      bool,  // prefers undoable actions (or clearly flags irreversible)
    pub net_positive:    bool,  // benefits > costs (social + economic + environmental)
    pub anti_extraction: bool,  // distributes value, does not extract from others
}

impl AlignmentCheck {
    pub fn assess(goal: &Goal) -> Self {
        let desc = goal.description.to_lowercase();
        let impact = goal.intended_impact.to_lowercase();
        let combined = format!("{} {}", desc, impact);

        // Heuristic alignment from declared intent — production: LLM-assessed + human review
        let harmful_signals   = ["harm", "deceiv", "manipulat", "exploit", "steal", "spy", "coerce", "destroy"];
        let opaque_signals    = ["hidden", "secret agenda", "without telling", "unknown intent"];
        let extract_signals   = ["extract", "monopol", "lock-in", "vendor lock", "capture"];

        let harm_free     = !harmful_signals.iter().any(|s| combined.contains(s));
        let transparent   = !opaque_signals.iter().any(|s| combined.contains(s));
        let anti_extract  = !extract_signals.iter().any(|s| combined.contains(s));

        AlignmentCheck {
            non_harm:           harm_free,
            transparent,
            consent_respecting: goal.requires_consent,
            accountable:        true,    // platform guarantees this for all goals
            reversible:         goal.prefer_reversible,
            net_positive:       goal.net_positive_declared,
            anti_extraction:    anti_extract,
        }
    }

    pub fn passes(&self) -> bool {
        self.non_harm
            && self.transparent
            && self.consent_respecting
            && self.accountable
            && self.net_positive
            && self.anti_extraction
    }

    pub fn score(&self) -> f64 {
        let checks = [
            self.non_harm, self.transparent, self.consent_respecting,
            self.accountable, self.reversible, self.net_positive, self.anti_extraction,
        ];
        checks.iter().filter(|&&v| v).count() as f64 / checks.len() as f64
    }

    pub fn failures(&self) -> Vec<&'static str> {
        let mut f = vec![];
        if !self.non_harm           { f.push("non_harm: goal may cause harm"); }
        if !self.transparent        { f.push("transparent: intent is not fully stated"); }
        if !self.consent_respecting { f.push("consent: coercive actions detected"); }
        if !self.accountable        { f.push("accountable: outcomes not verifiable"); }
        if !self.net_positive       { f.push("net_positive: benefits unclear"); }
        if !self.anti_extraction    { f.push("anti_extraction: extractive pattern detected"); }
        f
    }
}

// ── Impact metric ─────────────────────────────────────────────────────────────

/// A measurable change in the world.
/// Technical success (job ran, code compiled) ≠ world impact.
/// Impact is the real test of whether the goal was achieved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactMetric {
    pub id:          String,
    pub name:        String,
    pub description: String,
    pub unit:        String,         // "people helped", "hours saved", "CO2 kg", "USD distributed"
    pub baseline:    f64,
    pub target:      f64,
    pub actual:      Option<f64>,
    pub measured_at: Option<DateTime<Utc>>,
    pub domain:      ImpactDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImpactDomain {
    Social,        // people, communities, access, equity
    Economic,      // income, jobs, cost reduction, value creation
    Environmental, // CO2, energy, waste, biodiversity
    Educational,   // knowledge, skills, literacy, access to information
    Health,        // physical, mental, preventive care
    Civic,         // governance, transparency, participation
    Technological, // infrastructure, capability, access
    Cultural,      // arts, heritage, diversity, expression
}

impl ImpactMetric {
    pub fn progress(&self) -> f64 {
        if self.target == self.baseline { return 1.0; }
        let actual = self.actual.unwrap_or(self.baseline);
        ((actual - self.baseline) / (self.target - self.baseline)).clamp(0.0, 1.0)
    }

    pub fn achieved(&self) -> bool {
        self.actual.map(|a| a >= self.target).unwrap_or(false)
    }
}

// ── Objective ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub id:          String,
    pub goal_id:     String,
    pub title:       String,
    pub description: String,
    pub status:      ObjectiveStatus,
    pub assigned_to: Option<String>,     // agent DID responsible
    pub node_path:   Option<(String, String)>, // (from_node, to_node) in governance graph
    pub due_at:      Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub impact_contribution: f64,        // 0.0–1.0 fraction of goal impact this delivers
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
    Skipped,
}

// ── Mission ───────────────────────────────────────────────────────────────────

/// Why the agent exists. Enduring. Values-rooted. Not a task — a reason for being.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id:          String,
    pub agent_id:    String,
    pub statement:   String,
    pub values:      Vec<String>,        // e.g. "transparency", "equity", "sustainability"
    pub beneficiaries: Vec<String>,      // who benefits: "students", "smallholder farmers", "global"
    pub domain:      ImpactDomain,
    pub created_at:  DateTime<Utc>,
}

// ── Goal ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id:                   String,
    pub agent_id:             String,
    pub mission_id:           Option<String>,
    pub title:                String,
    pub description:          String,
    pub intended_impact:      String,    // plain language: "Reduce food waste in Lagos by 20%"
    pub status:               GoalStatus,
    pub alignment:            Option<AlignmentCheck>,
    pub impact_metrics:       Vec<ImpactMetric>,
    pub objectives:           Vec<String>, // objective IDs
    pub requires_consent:     bool,
    pub prefer_reversible:    bool,
    pub net_positive_declared: bool,
    pub due_at:               Option<DateTime<Utc>>,
    pub created_at:           DateTime<Utc>,
    pub updated_at:           DateTime<Utc>,
    pub completed_at:         Option<DateTime<Utc>>,
    pub world_model:          WorldModelDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Draft,        // being defined, not yet aligned-checked
    Aligned,      // passed all alignment checks — ready to pursue
    Active,       // agents working toward it
    Achieved,     // all impact metrics hit
    Abandoned,    // no longer pursued (reason recorded)
    Rejected,     // failed alignment check (reason recorded)
}

/// The change in the world the goal expects to produce.
/// Expressed as a before/after state — falsifiable, measurable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldModelDelta {
    pub before: String,      // current state of the world (the problem)
    pub after:  String,      // expected state after the goal is achieved
    pub scope:  String,      // geographic / demographic / systemic scope
    pub timeline_days: Option<u32>,
}

impl WorldModelDelta {
    pub fn empty() -> Self {
        WorldModelDelta {
            before:        String::new(),
            after:         String::new(),
            scope:         "local".into(),
            timeline_days: None,
        }
    }
}

// ── Goal registry ─────────────────────────────────────────────────────────────

pub struct GoalRegistry {
    goals:      RwLock<HashMap<String, Goal>>,
    objectives: RwLock<HashMap<String, Objective>>,
    missions:   RwLock<HashMap<String, Mission>>,
}

impl GoalRegistry {
    pub fn new() -> Self {
        GoalRegistry {
            goals:      RwLock::new(HashMap::new()),
            objectives: RwLock::new(HashMap::new()),
            missions:   RwLock::new(HashMap::new()),
        }
    }

    // ── Mission ──────────────────────────────────────────────────────────────

    pub fn declare_mission(
        &self,
        agent_id: &str,
        statement: &str,
        values: Vec<String>,
        beneficiaries: Vec<String>,
        domain: ImpactDomain,
    ) -> Mission {
        let m = Mission {
            id:            Uuid::new_v4().to_string(),
            agent_id:      agent_id.to_string(),
            statement:     statement.to_string(),
            values,
            beneficiaries,
            domain,
            created_at:    Utc::now(),
        };
        self.missions.write().unwrap().insert(m.id.clone(), m.clone());
        tracing::info!(agent = %agent_id, mission = %statement, "goals: mission declared");
        m
    }

    pub fn get_mission(&self, id: &str) -> Option<Mission> {
        self.missions.read().unwrap().get(id).cloned()
    }

    pub fn missions_for(&self, agent_id: &str) -> Vec<Mission> {
        self.missions.read().unwrap().values()
            .filter(|m| m.agent_id == agent_id)
            .cloned().collect()
    }

    // ── Goal ─────────────────────────────────────────────────────────────────

    pub fn create_goal(
        &self,
        agent_id: &str,
        title: &str,
        description: &str,
        intended_impact: &str,
        mission_id: Option<String>,
        world_model: WorldModelDelta,
        impact_metrics: Vec<ImpactMetric>,
    ) -> Goal {
        let id = Uuid::new_v4().to_string();
        let goal = Goal {
            id:                   id.clone(),
            agent_id:             agent_id.to_string(),
            mission_id,
            title:                title.to_string(),
            description:          description.to_string(),
            intended_impact:      intended_impact.to_string(),
            status:               GoalStatus::Draft,
            alignment:            None,
            impact_metrics,
            objectives:           vec![],
            requires_consent:     true,    // default: always ask consent
            prefer_reversible:    true,    // default: prefer reversible actions
            net_positive_declared: true,
            due_at:               None,
            created_at:           Utc::now(),
            updated_at:           Utc::now(),
            completed_at:         None,
            world_model,
        };
        self.goals.write().unwrap().insert(id, goal.clone());
        goal
    }

    /// Run alignment check and update goal status.
    /// Aligned goals can be pursued. Rejected goals are blocked.
    pub fn align(&self, goal_id: &str) -> Result<AlignmentCheck, String> {
        let mut goals = self.goals.write().unwrap();
        let goal = goals.get_mut(goal_id)
            .ok_or_else(|| format!("goal '{}' not found", goal_id))?;

        let check = AlignmentCheck::assess(goal);
        let passes = check.passes();
        let failures = check.failures();

        goal.alignment = Some(check.clone());
        goal.status = if passes {
            GoalStatus::Aligned
        } else {
            GoalStatus::Rejected
        };
        goal.updated_at = Utc::now();

        if passes {
            tracing::info!(goal = %goal_id, score = check.score(), "goals: aligned ✓");
        } else {
            tracing::warn!(
                goal     = %goal_id,
                failures = ?failures,
                "goals: alignment rejected"
            );
        }

        if passes { Ok(check) } else {
            Err(format!("Alignment failed: {}", failures.join("; ")))
        }
    }

    pub fn activate(&self, goal_id: &str) -> Result<Goal, String> {
        let mut goals = self.goals.write().unwrap();
        let goal = goals.get_mut(goal_id)
            .ok_or_else(|| format!("goal '{}' not found", goal_id))?;
        if goal.status != GoalStatus::Aligned {
            return Err(format!("Goal must be aligned before activation (status: {:?})", goal.status));
        }
        goal.status     = GoalStatus::Active;
        goal.updated_at = Utc::now();
        Ok(goal.clone())
    }

    pub fn record_impact(&self, goal_id: &str, metric_name: &str, actual: f64) -> Result<Goal, String> {
        let mut goals = self.goals.write().unwrap();
        let goal = goals.get_mut(goal_id)
            .ok_or_else(|| format!("goal '{}' not found", goal_id))?;

        for metric in &mut goal.impact_metrics {
            if metric.name == metric_name {
                metric.actual      = Some(actual);
                metric.measured_at = Some(Utc::now());
                break;
            }
        }

        // Check if all metrics achieved → mark goal complete
        let all_achieved = goal.impact_metrics.iter().all(|m| m.achieved());
        if all_achieved && goal.status == GoalStatus::Active {
            goal.status       = GoalStatus::Achieved;
            goal.completed_at = Some(Utc::now());
            tracing::info!(goal = %goal_id, "goals: achieved — the world is better");
        }

        goal.updated_at = Utc::now();
        Ok(goal.clone())
    }

    pub fn get_goal(&self, id: &str) -> Option<Goal> {
        self.goals.read().unwrap().get(id).cloned()
    }

    pub fn list_goals(&self, agent_id: Option<&str>) -> Vec<Goal> {
        self.goals.read().unwrap().values()
            .filter(|g| agent_id.map(|id| g.agent_id == id).unwrap_or(true))
            .cloned().collect()
    }

    // ── Objective ────────────────────────────────────────────────────────────

    pub fn add_objective(
        &self,
        goal_id: &str,
        title: &str,
        description: &str,
        assigned_to: Option<String>,
        node_path: Option<(String, String)>,
    ) -> Result<Objective, String> {
        {
            let goals = self.goals.read().unwrap();
            goals.get(goal_id).ok_or_else(|| format!("goal '{}' not found", goal_id))?;
        }
        let obj = Objective {
            id:                   Uuid::new_v4().to_string(),
            goal_id:              goal_id.to_string(),
            title:                title.to_string(),
            description:          description.to_string(),
            status:               ObjectiveStatus::Pending,
            assigned_to,
            node_path,
            due_at:               None,
            completed_at:         None,
            impact_contribution:  0.0,
        };
        let obj_id = obj.id.clone();
        self.objectives.write().unwrap().insert(obj_id.clone(), obj.clone());
        self.goals.write().unwrap()
            .get_mut(goal_id).map(|g| g.objectives.push(obj_id));
        Ok(obj)
    }

    pub fn complete_objective(&self, obj_id: &str) -> Result<Objective, String> {
        let mut objectives = self.objectives.write().unwrap();
        let obj = objectives.get_mut(obj_id)
            .ok_or_else(|| format!("objective '{}' not found", obj_id))?;
        obj.status       = ObjectiveStatus::Completed;
        obj.completed_at = Some(Utc::now());
        Ok(obj.clone())
    }

    pub fn objectives_for(&self, goal_id: &str) -> Vec<Objective> {
        self.objectives.read().unwrap().values()
            .filter(|o| o.goal_id == goal_id)
            .cloned().collect()
    }

    // ── Summary ──────────────────────────────────────────────────────────────

    pub fn summary(&self) -> Value {
        let goals = self.goals.read().unwrap();
        let total     = goals.len();
        let aligned   = goals.values().filter(|g| g.status == GoalStatus::Aligned).count();
        let active    = goals.values().filter(|g| g.status == GoalStatus::Active).count();
        let achieved  = goals.values().filter(|g| g.status == GoalStatus::Achieved).count();
        let rejected  = goals.values().filter(|g| g.status == GoalStatus::Rejected).count();
        let missions  = self.missions.read().unwrap().len();
        let objectives = self.objectives.read().unwrap().len();

        // Average impact progress across all active goals
        let impact_progress: f64 = if active == 0 { 0.0 } else {
            goals.values()
                .filter(|g| g.status == GoalStatus::Active)
                .map(|g| {
                    if g.impact_metrics.is_empty() { 0.0 }
                    else { g.impact_metrics.iter().map(|m| m.progress()).sum::<f64>()
                           / g.impact_metrics.len() as f64 }
                })
                .sum::<f64>() / active as f64
        };

        json!({
            "missions":       missions,
            "goals": {
                "total":    total,
                "aligned":  aligned,
                "active":   active,
                "achieved": achieved,
                "rejected": rejected,
            },
            "objectives":     objectives,
            "impact_progress": impact_progress,
            "philosophy": {
                "purpose":     "Agents with goals to make the world better",
                "alignment":   "Every goal checked against 7 values before activation",
                "measurement": "Impact measured in the world, not just in the system",
                "feedback":    "Feedback gate closes the loop — did the goal move the world forward?",
                "possible":    "Everything is possible when goals are aligned and agents are governed",
            },
        })
    }
}
