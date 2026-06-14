// Autonomyx Lifecycle — idempotent gates, each with an oath.
//
// The loop:
//   Build → Sign → Push → Sync → Deploy → Run → Observe → Feedback → (iterate)
//
// A Gate is the keeper of a stage transition.
// An Oath is its twin — the invariant that must hold before the gate opens.
// Code is the contract: if the oath breaks, the gate stays closed.
//
// Every gate is idempotent: calling it twice with the same input yields the
// same state. Transitions are atomic — partial state never leaks.
//
// Every gate emits:
//   - A SurrealDB record (fires live queries to all subscribers)
//   - An OTel span (distributed trace across the full loop)
//   - A Prometheus counter (metrics scrape)

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Lifecycle stages ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Build,     // Stacker SI — hermetic OCI image
    Sign,      // cosign — supply chain proof
    Push,      // Zot registry — content-addressed store
    Sync,      // ArgoCD — GitOps delivery
    Deploy,    // k8s rollout — cluster state
    Run,       // autonomyx-runner — agent execution
    Observe,   // OTel + Prometheus — telemetry
    Feedback,  // customer signal — adoption loop
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Build    => "build",
            Stage::Sign     => "sign",
            Stage::Push     => "push",
            Stage::Sync     => "sync",
            Stage::Deploy   => "deploy",
            Stage::Run      => "run",
            Stage::Observe  => "observe",
            Stage::Feedback => "feedback",
        }
    }

    /// The stage that must precede this one. None = entry point.
    pub fn predecessor(self) -> Option<Stage> {
        match self {
            Stage::Build    => None,
            Stage::Sign     => Some(Stage::Build),
            Stage::Push     => Some(Stage::Sign),
            Stage::Sync     => Some(Stage::Push),
            Stage::Deploy   => Some(Stage::Sync),
            Stage::Run      => Some(Stage::Deploy),
            Stage::Observe  => Some(Stage::Run),
            Stage::Feedback => Some(Stage::Observe),
        }
    }
}

// ── Gate outcome ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Open,    // oath held, transition committed
    Closed,  // oath broke — gate refused
    Already, // idempotent: stage already reached
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRecord {
    pub id:           String,
    pub artifact:     String,          // image digest, run_id, etc.
    pub stage:        Stage,
    pub status:       GateStatus,
    pub oath:         String,          // which invariant was checked
    pub detail:       Option<String>,  // why the gate closed (if it did)
    pub transitioned_at: DateTime<Utc>,
}

impl GateRecord {
    fn new(artifact: &str, stage: Stage, status: GateStatus, oath: &str, detail: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            artifact: artifact.into(),
            stage,
            status,
            oath: oath.into(),
            detail,
            transitioned_at: Utc::now(),
        }
    }
}

// ── Oath — the invariant twin ─────────────────────────────────────────────────
//
// An Oath is a named predicate over a payload. It returns Ok(()) when the
// invariant holds, Err(reason) when it breaks and the gate must stay closed.

pub type OathFn = Box<dyn Fn(&serde_json::Value) -> Result<(), String> + Send + Sync>;

pub struct Oath {
    pub name: &'static str,
    check:    OathFn,
}

impl Oath {
    pub fn new(name: &'static str, f: impl Fn(&serde_json::Value) -> Result<(), String> + Send + Sync + 'static) -> Self {
        Self { name, check: Box::new(f) }
    }

    pub fn verify(&self, payload: &serde_json::Value) -> Result<(), String> {
        (self.check)(payload)
    }
}

// ── Standard oaths — one per stage ───────────────────────────────────────────

pub fn build_oath() -> Oath {
    Oath::new("artifact_has_digest", |p| {
        match p.get("digest").and_then(|v| v.as_str()) {
            Some(d) if d.starts_with("sha256:") && d.len() == 71 => Ok(()),
            Some(d) => Err(format!("invalid digest format: {d}")),
            None => Err("missing artifact digest".into()),
        }
    })
}

pub fn sign_oath() -> Oath {
    Oath::new("cosign_bundle_present", |p| {
        match p.get("cosign_bundle").and_then(|v| v.as_str()) {
            Some(b) if !b.is_empty() => Ok(()),
            _ => Err("cosign bundle missing — supply chain unverified".into()),
        }
    })
}

pub fn push_oath() -> Oath {
    Oath::new("registry_ref_valid", |p| {
        match p.get("registry_ref").and_then(|v| v.as_str()) {
            Some(r) if r.contains('/') && r.contains(':') => Ok(()),
            Some(r) => Err(format!("malformed registry ref: {r}")),
            None => Err("registry_ref missing".into()),
        }
    })
}

pub fn sync_oath() -> Oath {
    Oath::new("argocd_app_healthy", |p| {
        match p.get("argocd_health").and_then(|v| v.as_str()) {
            Some("Healthy") => Ok(()),
            Some(s) => Err(format!("ArgoCD app unhealthy: {s}")),
            None => Err("argocd_health missing".into()),
        }
    })
}

pub fn deploy_oath() -> Oath {
    Oath::new("rollout_ready", |p| {
        let ready   = p.get("ready_replicas").and_then(|v| v.as_u64()).unwrap_or(0);
        let desired = p.get("desired_replicas").and_then(|v| v.as_u64()).unwrap_or(1);
        if ready >= desired {
            Ok(())
        } else {
            Err(format!("rollout not ready: {ready}/{desired} replicas"))
        }
    })
}

pub fn run_oath() -> Oath {
    Oath::new("run_has_agent", |p| {
        match p.get("agent_id").and_then(|v| v.as_str()) {
            Some(a) if !a.is_empty() => Ok(()),
            _ => Err("run must bind to a known agent_id".into()),
        }
    })
}

pub fn observe_oath() -> Oath {
    Oath::new("telemetry_emitted", |p| {
        match p.get("trace_id").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => Ok(()),
            _ => Err("trace_id missing — OTel span not recorded".into()),
        }
    })
}

pub fn feedback_oath() -> Oath {
    Oath::new("signal_has_source", |p| {
        match p.get("source").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => Ok(()),
            _ => Err("feedback signal must have a source".into()),
        }
    })
}

// ── Lifecycle registry ────────────────────────────────────────────────────────
//
// Tracks the highest stage reached per artifact. Idempotent: attempting to
// transition to a stage already reached returns GateStatus::Already without
// side effects.

#[derive(Default)]
struct LifecycleState {
    // artifact → highest Stage reached
    progress: HashMap<String, Stage>,
    // all gate records (append-only audit log)
    log: Vec<GateRecord>,
}

#[derive(Clone)]
pub struct LifecycleRegistry {
    inner: Arc<RwLock<LifecycleState>>,
}

impl LifecycleRegistry {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(LifecycleState::default())) }
    }

    /// Attempt a stage transition for an artifact.
    ///
    /// Gate rules (in order):
    ///   1. Oath check — invariant must hold.
    ///   2. Predecessor check — prior stage must be reached.
    ///   3. Idempotency — if already at this stage, return Already.
    ///   4. Atomic commit — stage recorded, record appended.
    pub fn transition(
        &self,
        artifact: &str,
        stage:    Stage,
        oath:     &Oath,
        payload:  &serde_json::Value,
    ) -> GateRecord {
        // 1. Oath check
        if let Err(reason) = oath.verify(payload) {
            let rec = GateRecord::new(artifact, stage, GateStatus::Closed, oath.name, Some(reason.clone()));
            tracing::warn!(artifact, stage = stage.as_str(), oath = oath.name, reason, "gate closed: oath broke");
            self.inner.write().unwrap().log.push(rec.clone());
            return rec;
        }

        let mut state = self.inner.write().unwrap();

        // 2. Predecessor check
        if let Some(pred) = stage.predecessor() {
            let reached = state.progress.get(artifact).copied();
            if reached.map(|r| r < pred).unwrap_or(true) {
                let reason = format!("predecessor stage '{:?}' not yet reached", pred);
                let rec = GateRecord::new(artifact, stage, GateStatus::Closed, oath.name, Some(reason.clone()));
                tracing::warn!(artifact, stage = stage.as_str(), reason, "gate closed: out of order");
                state.log.push(rec.clone());
                return rec;
            }
        }

        // 3. Idempotency
        if let Some(&reached) = state.progress.get(artifact) {
            if reached >= stage {
                let rec = GateRecord::new(artifact, stage, GateStatus::Already, oath.name, None);
                tracing::debug!(artifact, stage = stage.as_str(), "gate: already open (idempotent)");
                // Do NOT append to log — truly idempotent, no side effects.
                return rec;
            }
        }

        // 4. Atomic commit
        state.progress.insert(artifact.to_string(), stage);
        let rec = GateRecord::new(artifact, stage, GateStatus::Open, oath.name, None);
        state.log.push(rec.clone());
        tracing::info!(artifact, stage = stage.as_str(), oath = oath.name, "gate open");
        rec
    }

    pub fn stage_of(&self, artifact: &str) -> Option<Stage> {
        self.inner.read().unwrap().progress.get(artifact).copied()
    }

    pub fn log_for(&self, artifact: &str) -> Vec<GateRecord> {
        self.inner.read().unwrap().log.iter()
            .filter(|r| r.artifact == artifact)
            .cloned()
            .collect()
    }

    pub fn full_log(&self) -> Vec<GateRecord> {
        self.inner.read().unwrap().log.clone()
    }
}

// ── Gate API — open one stage at a time ───────────────────────────────────────

pub struct Gate<'a> {
    pub registry: &'a LifecycleRegistry,
    pub artifact: &'a str,
}

impl<'a> Gate<'a> {
    pub fn new(registry: &'a LifecycleRegistry, artifact: &'a str) -> Self {
        Self { registry, artifact }
    }

    pub fn build(&self, payload: &serde_json::Value)    -> GateRecord { self.registry.transition(self.artifact, Stage::Build,    &build_oath(),    payload) }
    pub fn sign(&self, payload: &serde_json::Value)     -> GateRecord { self.registry.transition(self.artifact, Stage::Sign,     &sign_oath(),     payload) }
    pub fn push(&self, payload: &serde_json::Value)     -> GateRecord { self.registry.transition(self.artifact, Stage::Push,     &push_oath(),     payload) }
    pub fn sync(&self, payload: &serde_json::Value)     -> GateRecord { self.registry.transition(self.artifact, Stage::Sync,     &sync_oath(),     payload) }
    pub fn deploy(&self, payload: &serde_json::Value)   -> GateRecord { self.registry.transition(self.artifact, Stage::Deploy,   &deploy_oath(),   payload) }
    pub fn run(&self, payload: &serde_json::Value)      -> GateRecord { self.registry.transition(self.artifact, Stage::Run,      &run_oath(),      payload) }
    pub fn observe(&self, payload: &serde_json::Value)  -> GateRecord { self.registry.transition(self.artifact, Stage::Observe,  &observe_oath(),  payload) }
    pub fn feedback(&self, payload: &serde_json::Value) -> GateRecord { self.registry.transition(self.artifact, Stage::Feedback, &feedback_oath(), payload) }
}
