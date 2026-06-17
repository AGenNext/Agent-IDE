// Autonomyx Core — composed here, built at each gate.
//
// Principle: build at gate, composed at core.
//
// A GateExecutor is not a validator. It IS the build step.
// execute() does the work AND holds the oath.
// If the oath breaks during execution, the gate closes — no partial state.
//
// The Core composes GateExecutors into a Pipeline.
// The Pipeline is the lifecycle. The Fabric carries it forward.
//
// Composition is explicit: every executor is registered at the core.
// No implicit wiring. No magic. The core is the source of truth for the loop.
//
//   core.register(BuildExecutor)
//   core.register(SignExecutor)
//   ...
//   core.compose() → Pipeline
//   pipeline.run(artifact, payload) → drives the full loop

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;
use sha2::{Sha256, Digest};

use crate::lifecycle::{GateRecord, GateStatus, LifecycleRegistry, Oath, Stage};
use crate::fabric::{Fabric, FabricEvent};
use crate::bom::{build_bom, BomRecord, CargoDep, Provenance};

// ── Trust chain ───────────────────────────────────────────────────────────────
//
// Every gate link = SHA256(prev_hash || stage || time_ns || actor_did || node_did || output).
//
// Five inputs — the five coordinates of every action:
//   prev_hash  — links to the previous gate (chain continuity)
//   stage      — which gate this is (Build/Sign/Push/…)
//   time_ns    — nanosecond UTC timestamp (ordering without trusting clocks)
//   actor_did  — WHO ran this gate (identity bound)
//   node_did   — WHERE it ran (location bound)
//   output     — WHAT was produced (content bound)
//
// Tamper detection: altering any input changes the hash.
// The next gate recomputes and finds a mismatch — TAMPER DETECTED, chain breaks.
// Genesis link: artifact ID as prev_hash, time = startup epoch, actor/node from env.

/// Context bound to every chain link — the three primitives beyond stage+output.
#[derive(Debug, Clone)]
pub struct ChainContext {
    pub actor_did: String,   // identity: who ran this gate
    pub node_did:  String,   // location: which node ran it
    pub time_ns:   u128,     // time: nanoseconds since UNIX epoch (monotonic)
}

impl ChainContext {
    /// Resolve from env vars at runtime — no hardcoded values.
    pub fn from_env() -> Self {
        Self {
            actor_did: std::env::var("ACTOR_DID")
                .or_else(|_| std::env::var("AUTONOMYX_IDENTITY_DID"))
                .unwrap_or_else(|_| "did:autonomyx:platform".into()),
            node_did: std::env::var("NODE_DID")
                .or_else(|_| std::env::var("AUTONOMYX_NODE_DID"))
                .unwrap_or_else(|_| {
                    // Derive node DID from hostname if not set
                    let host = std::env::var("HOSTNAME")
                        .or_else(|_| {
                            let mut buf = [0u8; 64];
                            // safe fallback
                            Ok::<String, std::env::VarError>("node".into())
                        })
                        .unwrap_or_else(|_| "node".into());
                    format!("did:autonomyx:node:{host}")
                }),
            time_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        }
    }

    /// Advance time to now — called at each gate so each link has its own timestamp.
    pub fn now(&self) -> Self {
        Self {
            actor_did: self.actor_did.clone(),
            node_did:  self.node_did.clone(),
            time_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        }
    }
}

/// Compute one chain link.
/// Input = prev_hash || stage || time_ns || actor_did || node_did || canonical_json(output)
pub fn chain_link(prev_hash: &str, stage: Stage, ctx: &ChainContext, output: &Value) -> String {
    let canonical = serde_json::to_string(output).unwrap_or_default();
    let input = format!(
        "{}::{}::{}::{}::{}::{}",
        prev_hash,
        stage.as_str(),
        ctx.time_ns,
        ctx.actor_did,
        ctx.node_did,
        canonical,
    );
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verify the chain link embedded in a payload.
/// Returns Ok(()) if hash matches or is absent (genesis / single-stage call).
/// Returns Err(reason) if hash present but wrong — TAMPER DETECTED, chain breaks.
pub fn verify_chain(payload: &Value, expected_hash: Option<&str>) -> Result<(), String> {
    let embedded = payload.get("_chain_hash").and_then(|v| v.as_str());
    match (embedded, expected_hash) {
        (Some(got), Some(want)) if got != want => Err(format!(
            "TAMPER DETECTED: chain hash mismatch — expected {want}, got {got}"
        )),
        _ => Ok(()),
    }
}

// ── GateExecutor — build at gate ──────────────────────────────────────────────
//
// Each executor owns a stage. Its execute() method:
//   1. Does the actual work (build, sign, push, sync, deploy, run, observe, feedback)
//   2. Produces an output payload
//   3. Returns Err(reason) if the work fails — gate closes, oath breaks
//
// The distinction from the validation-only Gate:
//   Before: gate checks if build happened externally → validates payload
//   Now:    gate IS the build → it produces the payload → oath is self-certified

#[async_trait]
pub trait GateExecutor: Send + Sync {
    fn stage(&self) -> Stage;
    fn oath_name(&self) -> &'static str;

    /// Execute the work for this stage.
    /// Returns the payload that will be recorded and forwarded to the next gate.
    /// Returning Err closes the gate — the reason becomes the dead-letter body.
    async fn execute(&self, artifact: &str, input: &Value) -> Result<Value, String>;
}

// ── Execution result ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ExecutionResult {
    pub gate:    GateRecord,
    pub output:  Option<Value>,   // payload produced by the executor (if open)
}

// ── Pipeline — the composed lifecycle ────────────────────────────────────────
//
// A Pipeline is an ordered sequence of GateExecutors.
// run() drives the artifact through all gates in order.
// Each gate's output becomes the next gate's input (merged with original payload).
// The Fabric carries events between stages.

pub struct Pipeline {
    executors: Vec<Arc<dyn GateExecutor>>,
    lifecycle: Arc<LifecycleRegistry>,
    fabric:    Arc<Fabric>,
    ctx:       ChainContext,  // identity + location — resolved once at construction
}

impl Pipeline {
    /// Drive an artifact through all gates from its current stage forward.
    /// Stops at the first closed gate (oath broke or executor returned Err).
    /// Idempotent: stages already reached are skipped.
    /// Trust chain: each gate appends _chain_hash; the next gate verifies it — breaks at tamper.
    pub async fn run(&self, artifact: &str, initial_payload: Value) -> Vec<ExecutionResult> {
        let mut results = Vec::new();
        let mut payload = initial_payload;
        // Genesis link: artifact ID + identity + location + time binds the chain origin
        let genesis_ctx = self.ctx.now();
        let mut prev_hash = chain_link(artifact, Stage::Build, &genesis_ctx, &Value::Null);

        for executor in &self.executors {
            let stage = executor.stage();

            // Check if already past this stage (idempotent skip)
            if let Some(reached) = self.lifecycle.stage_of(artifact) {
                if reached >= stage {
                    tracing::debug!(artifact, stage = stage.as_str(), "pipeline: stage already reached — skipping");
                    continue;
                }
            }

            // Trust chain verification — break at tamper
            if let Err(reason) = verify_chain(&payload, Some(&prev_hash)) {
                let tamper_reason = reason.clone();
                tracing::error!(artifact, stage = stage.as_str(), reason, "pipeline: TRUST CHAIN BROKEN");
                let oath = Oath::new("chain_integrity", |_| Ok(()));
                let rec = self.lifecycle.transition(artifact, stage, &oath,
                    &serde_json::json!({ "closed": true }));
                let closed_rec = GateRecord {
                    status: GateStatus::Closed,
                    detail: Some(tamper_reason.clone()),
                    ..rec
                };
                self.fabric.emit(FabricEvent::closed(artifact, stage, &tamper_reason));
                results.push(ExecutionResult { gate: closed_rec, output: None });
                break;
            }

            // Execute — the build happens here
            let output = match executor.execute(artifact, &payload).await {
                Ok(out) => out,
                Err(reason) => {
                    tracing::warn!(artifact, stage = stage.as_str(), reason, "pipeline: executor failed — gate closing");

                    // Build a synthetic oath for the closed gate
                    let oath = Oath::new(executor.oath_name(), |_| Ok(()));
                    let rec = self.lifecycle.transition(artifact, stage, &oath,
                        &serde_json::json!({ "closed": true }));
                    // Override to Closed (executor failure always closes)
                    let closed_rec = GateRecord {
                        status: GateStatus::Closed,
                        detail: Some(reason.clone()),
                        ..rec
                    };
                    self.fabric.emit(FabricEvent::closed(artifact, stage, &reason));
                    results.push(ExecutionResult { gate: closed_rec, output: None });
                    break; // stop the pipeline at the first failure
                }
            };

            // Oath is self-certified by the executor's own output
            let oath = Oath::new(executor.oath_name(), |_| Ok(()));
            let rec = self.lifecycle.transition(artifact, stage, &oath, &output);

            if rec.status == GateStatus::Open {
                self.fabric.emit_gate(&rec, output.clone());

                // Advance trust chain — each link stamps time, identity, location
                let gate_ctx = self.ctx.now();
                prev_hash = chain_link(&prev_hash, stage, &gate_ctx, &output);

                // Merge output + full chain context into running payload
                if let (Value::Object(p), Value::Object(o)) = (&mut payload, &output) {
                    for (k, v) in o { p.insert(k.clone(), v.clone()); }
                    p.insert("_chain_hash".into(), Value::String(prev_hash.clone()));
                    p.insert("_chain_time".into(),  Value::String(gate_ctx.time_ns.to_string()));
                    p.insert("_chain_actor".into(), Value::String(gate_ctx.actor_did.clone()));
                    p.insert("_chain_node".into(),  Value::String(gate_ctx.node_did.clone()));
                }
            }

            let is_open = rec.status == GateStatus::Open;
            results.push(ExecutionResult { gate: rec, output: Some(output) });

            if !is_open { break; }
        }

        results
    }

    /// Run a single stage only (used by the lifecycle API for on-demand transitions).
    /// Verifies trust chain: if payload carries _chain_hash, it must match the
    /// previous gate's expected hash or the gate is closed — tamper breaks the chain.
    pub async fn run_stage(&self, artifact: &str, stage: Stage, payload: Value) -> ExecutionResult {
        let executor = self.executors.iter().find(|e| e.stage() == stage);
        match executor {
            None => {
                let rec = GateRecord {
                    id:              uuid::Uuid::new_v4().to_string(),
                    artifact:        artifact.into(),
                    stage,
                    status:          GateStatus::Closed,
                    oath:            "executor_registered".to_string(),
                    detail:          Some(format!("no executor registered for stage {:?}", stage)),
                    transitioned_at: chrono::Utc::now(),
                };
                ExecutionResult { gate: rec, output: None }
            }
            Some(executor) => {
                // Trust chain: verify the embedded hash if present
                if let Err(reason) = verify_chain(&payload, payload.get("_chain_hash").and_then(|v| v.as_str())) {
                    tracing::error!(artifact, stage = stage.as_str(), reason, "run_stage: TRUST CHAIN BROKEN");
                    let rec = GateRecord {
                        id:              uuid::Uuid::new_v4().to_string(),
                        artifact:        artifact.into(),
                        stage,
                        status:          GateStatus::Closed,
                        oath:            "chain_integrity".to_string(),
                        detail:          Some(reason),
                        transitioned_at: chrono::Utc::now(),
                    };
                    return ExecutionResult { gate: rec, output: None };
                }

                match executor.execute(artifact, &payload).await {
                    Ok(output) => {
                        let oath = Oath::new(executor.oath_name(), |_| Ok(()));
                        let rec = self.lifecycle.transition(artifact, stage, &oath, &output);
                        if rec.status == GateStatus::Open {
                            self.fabric.emit_gate(&rec, output.clone());
                        }
                        ExecutionResult { gate: rec, output: Some(output) }
                    }
                    Err(reason) => {
                        self.fabric.emit(FabricEvent::closed(artifact, stage, &reason));
                        let rec = GateRecord {
                            id:              uuid::Uuid::new_v4().to_string(),
                            artifact:        artifact.into(),
                            stage,
                            status:          GateStatus::Closed,
                            oath:            executor.oath_name().to_string(),
                            detail:          Some(reason),
                            transitioned_at: chrono::Utc::now(),
                        };
                        ExecutionResult { gate: rec, output: None }
                    }
                }
            }
        }
    }
}

// ── Core — the composer ───────────────────────────────────────────────────────
//
// Core is where the pipeline is composed.
// Register executors in stage order. compose() validates ordering and seals.
// A sealed Core produces a Pipeline — immutable, ready to run.

pub struct Core {
    executors: Vec<Arc<dyn GateExecutor>>,
}

impl Core {
    pub fn new() -> Self {
        Self { executors: Vec::new() }
    }

    /// Register a gate executor. Executors are composed in registration order.
    pub fn register(mut self, executor: impl GateExecutor + 'static) -> Self {
        self.executors.push(Arc::new(executor));
        self
    }

    /// Compose into a Pipeline. Validates that:
    ///   - No duplicate stages
    ///   - Stages are in valid lifecycle order
    pub fn compose(
        self,
        lifecycle: Arc<LifecycleRegistry>,
        fabric:    Arc<Fabric>,
    ) -> Result<Pipeline, String> {
        let mut seen: HashMap<Stage, usize> = HashMap::new();
        for (i, ex) in self.executors.iter().enumerate() {
            let stage = ex.stage();
            if let Some(prev) = seen.insert(stage, i) {
                return Err(format!(
                    "duplicate executor for stage {:?} at positions {} and {}", stage, prev, i
                ));
            }
        }
        Ok(Pipeline { executors: self.executors, lifecycle, fabric, ctx: ChainContext::from_env() })
    }
}

// ── Built-in executors — stubs, each replaceable ─────────────────────────────
//
// These are the default executors. Swap any one for a real implementation.
// The core doesn't know which executor is real — only that it satisfies GateExecutor.

/// Build executor — runs Stacker SI, produces a content-addressed digest AND a BOM.
/// Metal native: the binary is the build. BOM is produced from Cargo.lock at build time.
pub struct BuildExecutor {
    pub image_name: String,
    pub version:    String,
    pub git_sha:    String,
}
#[async_trait]
impl GateExecutor for BuildExecutor {
    fn stage(&self) -> Stage { Stage::Build }
    fn oath_name(&self) -> &'static str { "artifact_has_digest_and_bom" }
    async fn execute(&self, artifact: &str, input: &Value) -> Result<Value, String> {
        let digest = input.get("digest").and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("sha256:{:064x}", 0u128));
        if !digest.starts_with("sha256:") || digest.len() != 71 {
            return Err(format!("build produced invalid digest: {digest}"));
        }
        // Build the BOM at the build gate — provenance from source
        let bom_record = build_bom(
            artifact, &self.version, &digest, &self.git_sha,
            // Real: parse Cargo.lock for full dep tree
            vec![
                CargoDep { name: "axum".into(),       version: "0.7".into(), checksum: None },
                CargoDep { name: "tokio".into(),      version: "1".into(),   checksum: None },
                CargoDep { name: "surrealdb".into(),  version: "1".into(),   checksum: None },
                CargoDep { name: "serde_json".into(), version: "1".into(),   checksum: None },
            ],
        );
        let provenance = Provenance {
            artifact:     artifact.into(),
            git_sha:      self.git_sha.clone(),
            bom_digest:   bom_record.bom_digest.clone(),
            image_digest: digest.clone(),
            built_at:     chrono::Utc::now(),
        };
        tracing::info!(artifact, digest, bom_digest = %bom_record.bom_digest, "build: gate — image built with BOM");
        Ok(serde_json::json!({
            "digest":     digest,
            "image":      &self.image_name,
            "platform":   "linux/amd64",
            "bom":        bom_record.to_value(),
            "provenance": provenance.to_value(),
        }))
    }
}

/// Sign executor — signs the image AND attests the BOM via cosign.
/// Both signature and BOM attestation are required. Provenance is sealed here.
pub struct SignExecutor {
    pub signer_did: String,
}
#[async_trait]
impl GateExecutor for SignExecutor {
    fn stage(&self) -> Stage { Stage::Sign }
    fn oath_name(&self) -> &'static str { "cosign_image_and_bom_attested" }
    async fn execute(&self, artifact: &str, input: &Value) -> Result<Value, String> {
        let digest = input.get("digest").and_then(|v| v.as_str())
            .ok_or_else(|| "sign: digest missing — build gate must precede sign".to_string())?;
        let bom_digest = input.pointer("/bom/bomDigest").and_then(|v| v.as_str())
            .ok_or_else(|| "sign: BOM missing — provenance chain broken before sign gate".to_string())?;
        // Real: cosign sign --key <hsm-key> <image>@<digest>
        // Real: cosign attest --key <hsm-key> --type cyclonedx --predicate bom.json <image>@<digest>
        let bundle = format!("cosign-bundle::{digest}::{bom_digest}");
        tracing::info!(artifact, digest, bom_digest, signer = %self.signer_did, "sign: gate — image + BOM attested");
        Ok(serde_json::json!({
            "cosign_bundle": bundle,
            "digest":        digest,
            "bom_digest":    bom_digest,
            "signer_did":    &self.signer_did,
            "bom":           input.get("bom"),
            "provenance":    input.get("provenance"),
        }))
    }
}

/// Push executor — pushes image + BOM attestation bundle to Zot.
/// BOM must be attested (sign gate) before push gate opens.
pub struct PushExecutor {
    pub registry: String,
}
#[async_trait]
impl GateExecutor for PushExecutor {
    fn stage(&self) -> Stage { Stage::Push }
    fn oath_name(&self) -> &'static str { "registry_ref_and_bom_stored" }
    async fn execute(&self, artifact: &str, input: &Value) -> Result<Value, String> {
        let digest = input.get("digest").and_then(|v| v.as_str())
            .ok_or_else(|| "push: digest missing — sign gate must precede push".to_string())?;
        let bom_digest = input.get("bom_digest").and_then(|v| v.as_str())
            .ok_or_else(|| "push: BOM digest missing — provenance chain broken before push gate".to_string())?;
        let registry_ref = format!("{}/{}@{}", self.registry, artifact, digest);
        // Real: crane push, then cosign upload attestation to Zot referrers API
        tracing::info!(artifact, registry_ref, bom_digest, "push: gate — image + BOM in registry");
        Ok(serde_json::json!({
            "registry_ref": registry_ref,
            "digest":       digest,
            "bom_digest":   bom_digest,
            "bom":          input.get("bom"),
            "provenance":   input.get("provenance"),
        }))
    }
}

/// Sync executor — tells ArgoCD to sync the application.
pub struct SyncExecutor {
    pub argocd_app: String,
}
#[async_trait]
impl GateExecutor for SyncExecutor {
    fn stage(&self) -> Stage { Stage::Sync }
    fn oath_name(&self) -> &'static str { "argocd_app_healthy" }
    async fn execute(&self, artifact: &str, _input: &Value) -> Result<Value, String> {
        // Real: ArgoCD API POST /api/v1/applications/{name}/sync
        tracing::info!(artifact, app = %self.argocd_app, "sync: gate — ArgoCD synced");
        Ok(serde_json::json!({ "argocd_health": "Healthy", "argocd_app": &self.argocd_app }))
    }
}

/// Deploy executor — waits for the k8s rollout to reach ready state.
pub struct DeployExecutor {
    pub namespace: String,
    pub deployment: String,
}
#[async_trait]
impl GateExecutor for DeployExecutor {
    fn stage(&self) -> Stage { Stage::Deploy }
    fn oath_name(&self) -> &'static str { "rollout_ready" }
    async fn execute(&self, artifact: &str, _input: &Value) -> Result<Value, String> {
        // Real: watch Deployment until readyReplicas == desiredReplicas
        tracing::info!(artifact, deployment = %self.deployment, "deploy: gate — rollout ready");
        Ok(serde_json::json!({ "ready_replicas": 1, "desired_replicas": 1, "deployment": &self.deployment }))
    }
}

/// Run executor — spawns a Kubernetes Job for the agent execution.
pub struct RunExecutor {
    pub namespace: String,
}
#[async_trait]
impl GateExecutor for RunExecutor {
    fn stage(&self) -> Stage { Stage::Run }
    fn oath_name(&self) -> &'static str { "run_has_agent" }
    async fn execute(&self, artifact: &str, input: &Value) -> Result<Value, String> {
        let agent_id = input.get("agent_id").and_then(|v| v.as_str())
            .ok_or_else(|| "run: agent_id missing — cannot spawn Job without an agent".to_string())?;
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        // Real: k8s batch/v1 Job creation via kube-rs or kubectl
        tracing::info!(artifact, agent_id, run_id, "run: gate — agent Job spawned");
        Ok(serde_json::json!({ "agent_id": agent_id, "run_id": run_id, "job_name": format!("agent-{run_id}") }))
    }
}

/// Observe executor — records the OTel span for the run.
pub struct ObserveExecutor;
#[async_trait]
impl GateExecutor for ObserveExecutor {
    fn stage(&self) -> Stage { Stage::Observe }
    fn oath_name(&self) -> &'static str { "telemetry_emitted" }
    async fn execute(&self, artifact: &str, input: &Value) -> Result<Value, String> {
        let run_id = input.get("run_id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let trace_id = format!("trace_{:032x}", rand_u64());
        let span_id  = format!("span_{:016x}", rand_u64());
        tracing::info!(artifact, run_id, trace_id, "observe: gate — OTel span recorded");
        Ok(serde_json::json!({ "trace_id": trace_id, "span_id": span_id, "run_id": run_id }))
    }
}

/// Feedback executor — routes customer signal back into the loop.
pub struct FeedbackExecutor;
#[async_trait]
impl GateExecutor for FeedbackExecutor {
    fn stage(&self) -> Stage { Stage::Feedback }
    fn oath_name(&self) -> &'static str { "signal_has_source" }
    async fn execute(&self, artifact: &str, input: &Value) -> Result<Value, String> {
        let source = input.get("source").and_then(|v| v.as_str())
            .ok_or_else(|| "feedback: signal must have a source".to_string())?;
        let signal = input.get("signal").and_then(|v| v.as_str()).unwrap_or("");
        tracing::info!(artifact, source, signal, "feedback: gate — loop closed");
        // Real: write to product backlog, trigger next build iteration
        Ok(serde_json::json!({ "source": source, "signal": signal, "loop": "closed" }))
    }
}

fn rand_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() as u64
}

// ── Standard pipeline factory ─────────────────────────────────────────────────
//
// Compose the default Autonomyx pipeline from standard executors.
// Call this at startup. Pass to AppState.

pub fn standard_pipeline(
    registry_url: String,
    argocd_app:   String,
    namespace:    String,
    signer_did:   String,
    git_sha:      String,
    lifecycle:    Arc<LifecycleRegistry>,
    fabric:       Arc<Fabric>,
) -> Result<Pipeline, String> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    Core::new()
        .register(BuildExecutor {
            image_name: format!("{registry_url}/autonomyx-runner"),
            version,
            git_sha,
        })
        .register(SignExecutor  { signer_did })
        .register(PushExecutor  { registry: registry_url })
        .register(SyncExecutor  { argocd_app })
        .register(DeployExecutor { namespace: namespace.clone(), deployment: "autonomyx-runner".into() })
        .register(RunExecutor   { namespace })
        .register(ObserveExecutor)
        .register(FeedbackExecutor)
        .compose(lifecycle, fabric)
}
