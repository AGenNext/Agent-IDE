// Autonomyx Fabric — a framework, not just a bus.
//
// The fabric fills the gaps between gates.
// Any system can register a handler at any stage.
// The fabric routes; handlers act.
//
// Gate opens → fabric emits event → all registered handlers for that stage fire.
// Gate closes → event goes to dead-letter → ops handlers fire.
//
// Built-in handlers (registered by default):
//   LogHandler      — tracing::info every open event
//   MetricHandler   — Prometheus counter increment (via label)
//   SurrealHandler  — write to ConfigDB (triggers live queries)
//
// External systems plug in by implementing FabricHandler:
//   ArgoCD      — watch sync gate, trigger app refresh
//   k8s         — watch deploy gate, poll rollout
//   OTel        — watch run/observe, close the span
//   Product     — watch feedback, route signal to backlog
//   Scheduler   — watch deploy gate, wake agent runner
//
// The framework guarantees:
//   - All handlers called in registration order
//   - Handler panics are caught — one bad handler can't break the fabric
//   - Idempotent: Already-status events never reach handlers
//   - Dead-letter: Closed-status events reach dead-letter handlers only

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::lifecycle::{GateRecord, GateStatus, Stage};

// ── Fabric event ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricEvent {
    pub id:         String,
    pub artifact:   String,
    pub stage:      Stage,
    pub status:     FabricStatus,
    pub payload:    serde_json::Value,
    pub emitted_at: DateTime<Utc>,
    // Thread tags: every entity this event touches.
    // Pull the thread for any entity with fabric.thread(entity_id).
    #[serde(default)]
    pub entities:   Vec<String>,
    // Source node: which cluster member emitted this event (empty = local)
    #[serde(default)]
    pub source:     String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FabricStatus {
    Open,    // gate opened — route to stage handlers
    Closed,  // gate refused — route to dead-letter handlers
    Already, // idempotent replay — drop silently, no handlers called
}

impl FabricEvent {
    pub fn open(artifact: &str, stage: Stage, payload: serde_json::Value) -> Self {
        Self { id: Uuid::new_v4().to_string(), artifact: artifact.into(), stage,
               status: FabricStatus::Open, payload, emitted_at: Utc::now(),
               entities: vec![], source: String::new() }
    }

    pub fn closed(artifact: &str, stage: Stage, reason: &str) -> Self {
        Self { id: Uuid::new_v4().to_string(), artifact: artifact.into(), stage,
               status: FabricStatus::Closed,
               payload: serde_json::json!({ "reason": reason }),
               emitted_at: Utc::now(), entities: vec![], source: String::new() }
    }

    /// Tag this event with entity IDs — these entities are part of this thread.
    pub fn with_entities(mut self, entities: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.entities = entities.into_iter().map(Into::into).collect();
        self
    }

    /// Tag with the emitting source node URL.
    pub fn from_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Stage to trigger next, if this event is Open.
    pub fn next_stage(&self) -> Option<Stage> {
        if self.status != FabricStatus::Open { return None; }
        match self.stage {
            Stage::Build    => Some(Stage::Sign),
            Stage::Sign     => Some(Stage::Push),
            Stage::Push     => Some(Stage::Sync),
            Stage::Sync     => Some(Stage::Deploy),
            Stage::Deploy   => Some(Stage::Run),
            Stage::Run      => Some(Stage::Observe),
            Stage::Observe  => Some(Stage::Feedback),
            Stage::Feedback => None,
        }
    }
}

// ── Handler trait — the extension point ──────────────────────────────────────
//
// Implement FabricHandler to plug any system into the fabric.
// Handlers are sync (called from the emit path) and must not block.
// For async work, spawn a task inside handle() and return immediately.

pub trait FabricHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn handle(&self, event: &FabricEvent);
}

pub trait DeadLetterHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn handle_dead(&self, event: &FabricEvent, reason: &str);
}

// ── Built-in handlers ─────────────────────────────────────────────────────────

/// Logs every open gate event.
pub struct LogHandler;
impl FabricHandler for LogHandler {
    fn name(&self) -> &'static str { "log" }
    fn handle(&self, ev: &FabricEvent) {
        tracing::info!(
            artifact = %ev.artifact,
            stage    = ev.stage.as_str(),
            event_id = %ev.id,
            "fabric: gate open"
        );
    }
}

/// Increments a Prometheus-style counter via tracing (scrape picks it up).
pub struct MetricHandler;
impl FabricHandler for MetricHandler {
    fn name(&self) -> &'static str { "metric" }
    fn handle(&self, ev: &FabricEvent) {
        tracing::info!(
            target   = "autonomyx_metrics",
            metric   = "autonomyx_gate_open_total",
            stage    = ev.stage.as_str(),
            artifact = %ev.artifact,
            "counter"
        );
    }
}

/// Writes a gate-open event to ConfigDB (fires SurrealDB live queries).
pub struct SurrealHandler {
    pub config: Arc<crate::configdb::ConfigDB>,
}
impl FabricHandler for SurrealHandler {
    fn name(&self) -> &'static str { "surreal" }
    fn handle(&self, ev: &FabricEvent) {
        let config = self.config.clone();
        let artifact = ev.artifact.clone();
        let stage    = ev.stage.as_str().to_string();
        let event_id = ev.id.clone();
        let payload  = ev.payload.clone();
        // spawn async write — non-blocking from emit path
        tokio::spawn(async move {
            let name = format!("{artifact}/{stage}/{event_id}");
            let data = serde_json::json!({
                "artifact": artifact,
                "stage":    stage,
                "event_id": event_id,
                "payload":  payload,
            });
            if let Err(e) = config.put("gate_event", &name, data, "fabric").await {
                tracing::warn!(error = %e, "fabric: surreal write failed");
            }
        });
    }
}

/// Logs dead-letter events so ops can act.
pub struct DeadLetterLogger;
impl DeadLetterHandler for DeadLetterLogger {
    fn name(&self) -> &'static str { "dead_letter_log" }
    fn handle_dead(&self, ev: &FabricEvent, reason: &str) {
        tracing::warn!(
            artifact = %ev.artifact,
            stage    = ev.stage.as_str(),
            reason,
            "fabric: gate closed → dead-letter"
        );
    }
}

// ── Fabric registry ───────────────────────────────────────────────────────────

struct FabricInner {
    // per-stage handlers
    handlers:    HashMap<Option<Stage>, Vec<Box<dyn FabricHandler>>>,
    // dead-letter handlers
    dead:        Vec<Box<dyn DeadLetterHandler>>,
    // append-only event log (capped ring)
    log:         Vec<FabricEvent>,
    dead_log:    Vec<FabricEvent>,
}

// ── Fabric — the framework ────────────────────────────────────────────────────

pub struct Fabric {
    inner:   Arc<RwLock<FabricInner>>,
    // broadcast channel for async subscribers (WS, SSE, etc.)
    tx:      broadcast::Sender<String>,
}

impl Fabric {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        let fab = Self {
            inner: Arc::new(RwLock::new(FabricInner {
                handlers: HashMap::new(),
                dead:     Vec::new(),
                log:      Vec::new(),
                dead_log: Vec::new(),
            })),
            tx,
        };
        // Register built-in handlers (all stages)
        fab.on(None, LogHandler);
        fab.on(None, MetricHandler);
        fab
    }

    /// Register a handler for a specific stage (None = all stages).
    pub fn on(&self, stage: Option<Stage>, handler: impl FabricHandler + 'static) {
        self.inner.write().unwrap()
            .handlers.entry(stage).or_default()
            .push(Box::new(handler));
    }

    /// Register a dead-letter handler.
    pub fn on_dead(&self, handler: impl DeadLetterHandler + 'static) {
        self.inner.write().unwrap().dead.push(Box::new(handler));
    }

    /// Wire the SurrealDB handler (called after ConfigDB is available).
    pub fn wire_surreal(&self, config: Arc<crate::configdb::ConfigDB>) {
        self.on(None, SurrealHandler { config });
        self.on_dead(DeadLetterLogger);
    }

    /// Subscribe to raw JSON events (for WebSocket / SSE broadcast).
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    // ── Core emit path ────────────────────────────────────────────────────────

    pub fn emit(&self, ev: FabricEvent) {
        // Idempotent: Already events never reach handlers
        if ev.status == FabricStatus::Already {
            tracing::debug!(artifact = %ev.artifact, stage = ?ev.stage, "fabric: idempotent replay — dropped");
            return;
        }

        let is_closed = ev.status == FabricStatus::Closed;
        let reason = if is_closed {
            ev.payload.get("reason").and_then(|v| v.as_str()).unwrap_or("oath broke").to_string()
        } else {
            String::new()
        };

        // Broadcast JSON to async subscribers
        let _ = self.tx.send(ev.to_json_string());

        let inner = self.inner.read().unwrap();

        if is_closed {
            // Route to dead-letter handlers only
            inner.dead.iter().for_each(|h| {
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.handle_dead(&ev, &reason))) {
                    tracing::error!(handler = h.name(), "dead-letter handler panicked: {:?}", e);
                }
            });
            drop(inner);
            self.inner.write().unwrap().dead_log.push(ev);
            return;
        }

        // Route to stage-specific handlers, then global handlers
        let stage = ev.stage;
        for key in [Some(stage), None] {
            if let Some(hs) = inner.handlers.get(&key) {
                for h in hs {
                    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| h.handle(&ev))) {
                        tracing::error!(handler = h.name(), "fabric handler panicked: {:?}", e);
                    }
                }
            }
        }
        drop(inner);

        let mut w = self.inner.write().unwrap();
        w.log.push(ev);
        if w.log.len() > 10_000 { w.log.drain(0..1_000); }
    }

    /// Bridge from a gate record directly into the fabric.
    pub fn emit_gate(&self, rec: &GateRecord, payload: serde_json::Value) {
        let ev = match rec.status {
            GateStatus::Open    => FabricEvent::open(&rec.artifact, rec.stage, payload),
            GateStatus::Closed  => FabricEvent::closed(&rec.artifact, rec.stage,
                                     rec.detail.as_deref().unwrap_or("oath broke")),
            GateStatus::Already => {
                tracing::debug!(artifact = %rec.artifact, stage = ?rec.stage, "fabric: already (gate bridge)");
                return;
            }
        };
        self.emit(ev);
    }

    // ── Query ─────────────────────────────────────────────────────────────────

    pub fn log_for(&self, artifact: &str) -> Vec<FabricEvent> {
        self.inner.read().unwrap().log.iter()
            .filter(|e| e.artifact == artifact)
            .cloned()
            .collect()
    }

    /// Pull the full thread for any entity — all events that touch it.
    /// Matches on artifact, entity tags, and payload fields containing the id.
    pub fn thread(&self, entity_id: &str) -> Vec<FabricEvent> {
        self.inner.read().unwrap().log.iter()
            .filter(|e| {
                e.artifact == entity_id
                    || e.entities.iter().any(|t| t == entity_id)
                    || e.payload.to_string().contains(entity_id)
            })
            .cloned()
            .collect()
    }

    /// Latest N events across all artifacts — the live fabric stream snapshot.
    pub fn recent(&self, n: usize) -> Vec<FabricEvent> {
        let log = self.inner.read().unwrap();
        let total = log.log.len();
        log.log[total.saturating_sub(n)..].to_vec()
    }

    pub fn full_log(&self) -> Vec<FabricEvent> {
        self.inner.read().unwrap().log.clone()
    }

    pub fn dead_log(&self) -> Vec<FabricEvent> {
        self.inner.read().unwrap().dead_log.clone()
    }

    pub fn stats(&self) -> serde_json::Value {
        let inner = self.inner.read().unwrap();
        let open   = inner.log.iter().filter(|e| e.status == FabricStatus::Open).count();
        let closed = inner.dead_log.len();
        let by_stage: std::collections::HashMap<&str, usize> = {
            let mut m = std::collections::HashMap::new();
            for e in &inner.log { *m.entry(e.stage.as_str()).or_insert(0) += 1; }
            m
        };
        serde_json::json!({
            "total_events":  inner.log.len(),
            "open_events":   open,
            "dead_letters":  closed,
            "by_stage":      by_stage,
            "log_capacity":  10_000,
        })
    }
}

// ── Thread handler — auto-tags events with related entities ───────────────────
// Registers as a fabric handler. Inspects every event payload for known entity
// ID patterns and adds them to the entities thread list.

pub struct ThreadHandler;
impl FabricHandler for ThreadHandler {
    fn name(&self) -> &'static str { "thread" }
    fn handle(&self, _ev: &FabricEvent) {
        // Threading happens at emit time via with_entities() on the caller side.
        // This handler is a no-op hook point for future enrichment.
    }
}

impl FabricEvent {
    fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

impl Default for Fabric {
    fn default() -> Self { Self::new() }
}

impl Default for FabricEvent {
    fn default() -> Self {
        Self {
            id:         String::new(),
            artifact:   String::new(),
            stage:      crate::lifecycle::Stage::Build,
            status:     FabricStatus::Open,
            payload:    serde_json::Value::Null,
            emitted_at: Utc::now(),
            entities:   vec![],
            source:     String::new(),
        }
    }
}
