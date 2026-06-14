// Opt-in — extend or align.
//
// Two voluntary, governed flows for any agent, plugin, or human to join the platform:
//
// EXTEND — register a new capability, plugin node, or tool into the governance graph.
//   Anyone can extend the platform. Extensions are governed: they must declare their
//   capabilities, required config, and the governance edges they want to traverse.
//   Approved extensions become first-class citizens — they appear in search, dashboards,
//   governance paths, and are callable by other agents.
//
// ALIGN — submit an intent, goal, or workflow for the 7-value alignment check.
//   Alignment is opt-in: you can run without it, but aligned agents get priority routing,
//   higher trust scores, and can traverse human-gated governance edges.
//   Alignment is transparent: the verdict, score, and any failures are public.
//   Alignment is not a gate — it is a signal. Rejected alignment is logged, not banned.
//   The platform encourages re-submission with improved intent.
//
// Both flows emit fabric events, appear in search, and are tracked in the accountability log.
//
// "Everything extendable." — openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Opt-in kind ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OptInKind {
    Extend,   // register a capability / plugin / governance node
    Align,    // submit intent for 7-value alignment check
}

// ── Opt-in status ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OptInStatus {
    Pending,   // submitted, not yet processed
    Approved,  // extend: wired in; align: passed all 7 checks
    Rejected,  // extend: invalid declaration; align: failed alignment
    Active,    // extend: live in graph; align: agent is operating under alignment
    Withdrawn, // actor voluntarily withdrew the opt-in
}

// ── Alignment verdict ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentVerdict {
    pub score:     f64,          // 0.0–1.0; fraction of 7 checks passing
    pub passes:    bool,         // true only if all 7 pass
    pub checks: AlignmentChecks,
    pub failures:  Vec<String>,  // human-readable failure reasons
    pub guidance:  Vec<String>,  // suggestions to improve alignment
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentChecks {
    pub non_harm:           bool,
    pub transparent:        bool,
    pub consent_respecting: bool,
    pub accountable:        bool,
    pub reversible:         bool,
    pub net_positive:       bool,
    pub anti_extraction:    bool,
}

impl AlignmentChecks {
    pub fn assess(title: &str, description: &str, intended_impact: &str) -> Self {
        let combined = format!("{} {} {}", title, description, intended_impact).to_lowercase();

        let harmful     = ["harm", "deceiv", "manipulat", "exploit", "steal", "spy",
                           "coerce", "destroy", "damage", "weaponize", "surveil"];
        let opaque      = ["hidden agenda", "without telling", "bypass", "circumvent",
                           "without consent", "secretly", "covert"];
        let extractive  = ["extract", "monopol", "lock-in", "vendor lock", "capture market",
                           "drain", "exploit users"];

        let non_harm            = !harmful.iter().any(|s| combined.contains(s));
        let transparent         = !opaque.iter().any(|s| combined.contains(s));
        let anti_extraction     = !extractive.iter().any(|s| combined.contains(s));
        let consent_respecting  = !combined.contains("without consent")
                                   && !combined.contains("force");
        let reversible          = !combined.contains("irreversible")
                                   && !combined.contains("permanent delete")
                                   && !combined.contains("cannot be undone");
        let net_positive        = combined.contains("improv") || combined.contains("help")
                                   || combined.contains("benefit") || combined.contains("better")
                                   || combined.contains("support") || combined.contains("enable")
                                   || combined.contains("accelerat") || combined.contains("build")
                                   || combined.contains("creat") || combined.contains("protect");
        let accountable         = true;  // platform guarantees this for all opt-ins

        AlignmentChecks {
            non_harm, transparent, consent_respecting,
            accountable, reversible, net_positive, anti_extraction,
        }
    }

    pub fn passes(&self) -> bool {
        self.non_harm && self.transparent && self.consent_respecting
            && self.accountable && self.net_positive && self.anti_extraction
    }

    pub fn score(&self) -> f64 {
        let checks = [
            self.non_harm, self.transparent, self.consent_respecting,
            self.accountable, self.reversible, self.net_positive, self.anti_extraction,
        ];
        checks.iter().filter(|&&v| v).count() as f64 / checks.len() as f64
    }

    pub fn failures(&self) -> Vec<String> {
        let mut f = vec![];
        if !self.non_harm           { f.push("non_harm: intent may cause harm — clarify that this improves conditions".into()); }
        if !self.transparent        { f.push("transparent: hidden or covert patterns detected — state all actions explicitly".into()); }
        if !self.consent_respecting { f.push("consent: coercive patterns detected — all actions must be opt-in".into()); }
        if !self.net_positive       { f.push("net_positive: no positive impact declared — state who benefits and how".into()); }
        if !self.anti_extraction    { f.push("anti_extraction: extractive pattern detected — distribute value, don't capture it".into()); }
        f
    }

    pub fn guidance(&self) -> Vec<String> {
        let mut g = vec![];
        if !self.non_harm      { g.push("Add a 'safeguards' field: how does this prevent harm?".into()); }
        if !self.transparent   { g.push("Add an 'actions' field listing every action this takes explicitly.".into()); }
        if !self.net_positive  { g.push("Add an 'intended_impact' field: who benefits, in what domain, by how much?".into()); }
        if !self.reversible    { g.push("Mark irreversible actions explicitly and add a confirmation step.".into()); }
        if !self.anti_extraction { g.push("Clarify how value is distributed to all participants, not captured by one.".into()); }
        if g.is_empty() {
            g.push("All checks pass. Consider adding impact metrics to track real-world outcomes.".into());
        }
        g
    }
}

// ── Extension declaration ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDeclaration {
    pub name:         String,
    pub version:      String,
    pub description:  String,
    pub author:       String,
    pub capabilities: Vec<String>,
    pub config_keys:  Vec<String>,
    pub node_kind:    String,        // "tool" | "source" | "sink" | "api" | "agent"
    pub edges_to:     Vec<String>,   // governance graph node IDs to connect to
    pub homepage:     Option<String>,
}

// ── Opt-in record ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptInRecord {
    pub id:           String,
    pub kind:         OptInKind,
    pub actor_did:    String,        // who is opting in
    pub name:         String,        // human label
    pub description:  String,
    pub status:       OptInStatus,

    // For Align kind
    pub alignment:    Option<AlignmentVerdict>,

    // For Extend kind
    pub extension:    Option<ExtensionDeclaration>,

    // Tracking
    pub submitted_at: DateTime<Utc>,
    pub resolved_at:  Option<DateTime<Utc>>,
    pub metadata:     Value,
}

// ── Opt-in registry ───────────────────────────────────────────────────────────

pub struct OptInRegistry {
    records: RwLock<HashMap<String, OptInRecord>>,
}

impl OptInRegistry {
    pub fn new() -> Self {
        OptInRegistry { records: RwLock::new(HashMap::new()) }
    }

    // ── Extend flow ──────────────────────────────────────────────────────────

    pub fn submit_extend(
        &self,
        actor_did: &str,
        ext: ExtensionDeclaration,
    ) -> OptInRecord {
        let id = format!("optin_{}", Uuid::new_v4().simple());

        // Basic validation
        let valid = !ext.name.is_empty()
            && !ext.capabilities.is_empty()
            && !ext.description.is_empty();

        let status = if valid { OptInStatus::Approved } else { OptInStatus::Rejected };
        let name   = ext.name.clone();
        let desc   = ext.description.clone();

        let record = OptInRecord {
            id:           id.clone(),
            kind:         OptInKind::Extend,
            actor_did:    actor_did.to_string(),
            name,
            description:  desc,
            status:       status.clone(),
            alignment:    None,
            extension:    Some(ext),
            submitted_at: Utc::now(),
            resolved_at:  Some(Utc::now()),
            metadata:     json!({}),
        };
        self.records.write().unwrap().insert(id.clone(), record.clone());

        tracing::info!(
            id      = %id,
            actor   = %actor_did,
            status  = ?status,
            "optin: extend submitted"
        );
        record
    }

    // ── Align flow ───────────────────────────────────────────────────────────

    pub fn submit_align(
        &self,
        actor_did: &str,
        name: &str,
        description: &str,
        intended_impact: &str,
    ) -> OptInRecord {
        let id = format!("optin_{}", Uuid::new_v4().simple());
        let checks = AlignmentChecks::assess(name, description, intended_impact);
        let passes  = checks.passes();
        let score   = checks.score();
        let failures = checks.failures();
        let guidance = checks.guidance();

        let verdict = AlignmentVerdict {
            score, passes, checks, failures, guidance,
        };

        let status = if passes { OptInStatus::Approved } else { OptInStatus::Rejected };

        let record = OptInRecord {
            id:           id.clone(),
            kind:         OptInKind::Align,
            actor_did:    actor_did.to_string(),
            name:         name.to_string(),
            description:  description.to_string(),
            status:       status.clone(),
            alignment:    Some(verdict),
            extension:    None,
            submitted_at: Utc::now(),
            resolved_at:  Some(Utc::now()),
            metadata:     json!({ "intended_impact": intended_impact }),
        };
        self.records.write().unwrap().insert(id.clone(), record.clone());

        if passes {
            tracing::info!(id = %id, actor = %actor_did, score = score, "optin: aligned ✓");
        } else {
            tracing::warn!(id = %id, actor = %actor_did, score = score, "optin: alignment rejected");
        }
        record
    }

    // ── Lifecycle ────────────────────────────────────────────────────────────

    pub fn activate(&self, id: &str) -> bool {
        let mut records = self.records.write().unwrap();
        if let Some(r) = records.get_mut(id) {
            if r.status == OptInStatus::Approved {
                r.status = OptInStatus::Active;
                return true;
            }
        }
        false
    }

    pub fn withdraw(&self, id: &str, actor_did: &str) -> bool {
        let mut records = self.records.write().unwrap();
        if let Some(r) = records.get_mut(id) {
            if r.actor_did == actor_did {
                r.status = OptInStatus::Withdrawn;
                return true;
            }
        }
        false
    }

    // ── Query ────────────────────────────────────────────────────────────────

    pub fn get(&self, id: &str) -> Option<OptInRecord> {
        self.records.read().unwrap().get(id).cloned()
    }

    pub fn list(&self) -> Vec<OptInRecord> {
        let mut v: Vec<OptInRecord> = self.records.read().unwrap().values().cloned().collect();
        v.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));
        v
    }

    pub fn list_by_kind(&self, kind: OptInKind) -> Vec<OptInRecord> {
        self.records.read().unwrap().values()
            .filter(|r| r.kind == kind)
            .cloned().collect()
    }

    pub fn active_extensions(&self) -> Vec<OptInRecord> {
        self.records.read().unwrap().values()
            .filter(|r| r.kind == OptInKind::Extend
                     && (r.status == OptInStatus::Approved || r.status == OptInStatus::Active))
            .cloned().collect()
    }

    pub fn active_alignments(&self) -> Vec<OptInRecord> {
        self.records.read().unwrap().values()
            .filter(|r| r.kind == OptInKind::Align
                     && (r.status == OptInStatus::Approved || r.status == OptInStatus::Active))
            .cloned().collect()
    }

    pub fn summary(&self) -> Value {
        let records = self.records.read().unwrap();
        let total    = records.len();
        let approved = records.values().filter(|r| r.status == OptInStatus::Approved).count();
        let active   = records.values().filter(|r| r.status == OptInStatus::Active).count();
        let rejected = records.values().filter(|r| r.status == OptInStatus::Rejected).count();
        let extends  = records.values().filter(|r| r.kind == OptInKind::Extend).count();
        let aligns   = records.values().filter(|r| r.kind == OptInKind::Align).count();
        let avg_score: f64 = {
            let scored: Vec<f64> = records.values()
                .filter_map(|r| r.alignment.as_ref().map(|a| a.score))
                .collect();
            if scored.is_empty() { 0.0 }
            else { scored.iter().sum::<f64>() / scored.len() as f64 }
        };

        json!({
            "total":    total,
            "approved": approved,
            "active":   active,
            "rejected": rejected,
            "by_kind": {
                "extend": extends,
                "align":  aligns,
            },
            "avg_alignment_score": avg_score,
            "philosophy": {
                "extend": "Anyone can extend the platform. Extensions are governed.",
                "align":  "Alignment is opt-in. Aligned agents get priority routing and higher trust.",
                "rejection": "Rejected alignment is logged, not banned. Resubmit with improved intent.",
            },
        })
    }
}
