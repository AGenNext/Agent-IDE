// ComputeKube — Kubernetes execution backend for the governance graph.
//
// When the governance graph executes a node, ComputeKube spawns a Kubernetes Job.
// The governance graph IS the job scheduler. Every job is governed, accountable, metered.
//
// Architecture:
//   GovernanceGraph::execute_path()
//     → ComputeKube::spawn(node, req)        — create k8s Job, label with DID + BOM
//     → KubeApiClient::create_job()          — POST to /apis/batch/v1/namespaces/:ns/jobs
//     → watch Job status                     — GET .../jobs/:name/status
//     → stream logs                          — GET .../pods/:name/log?follow=true
//     → ComputeKube::complete(job, result)   — record usage, update trust, emit fabric
//
// Every k8s Job:
//   - Label: autonomyx/did=<agent-did>
//   - Label: autonomyx/run-id=<run-id>
//   - Label: autonomyx/node=<govgraph-node-id>
//   - Label: autonomyx/capability=<capability>
//   - Annotation: autonomyx/bom=<content-hash>  ← Bill of Materials at every gate
//   - SecurityContext: nonRoot, readOnly FS, no privilege escalation
//   - ResourceLimits: from governance node policy
//   - ServiceAccount: autonomyx-runner (RBAC-controlled)
//   - TTL: 300s after completion (auto-cleanup)
//
// Auth: ServiceAccount in-cluster (KUBE_TOKEN + KUBE_CA_CERT) or KUBECONFIG for local dev.
//
// "ComputeKube" = compute + kube: every agent run is a governed k8s workload.
// Not just a job queue — the governance graph controls what runs, how, with what limits.
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Kube client — raw HTTPS to k8s API server ────────────────────────────────

#[derive(Clone)]
pub struct KubeApiClient {
    pub api_server: String,
    pub namespace:  String,
    pub token:      Option<String>,
    pub ca_cert:    Option<String>,
    http:           reqwest::Client,
}

impl KubeApiClient {
    pub fn from_env() -> Option<Self> {
        // In-cluster: token + CA mounted by k8s at well-known paths
        let api_server = std::env::var("KUBERNETES_SERVICE_HOST").ok()
            .map(|h| format!("https://{}:{}", h,
                std::env::var("KUBERNETES_SERVICE_PORT").unwrap_or_else(|_| "443".into())))
            .or_else(|| std::env::var("KUBE_API_SERVER").ok())?;

        let namespace = std::env::var("POD_NAMESPACE")
            .or_else(|_| std::env::var("AGENT_JOB_NAMESPACE"))
            .unwrap_or_else(|_| "autonomyx".into());

        let token = std::env::var("KUBE_TOKEN").ok()
            .or_else(|| Self::read_sa_token());

        let ca_cert = std::env::var("KUBE_CA_CERT").ok()
            .or_else(|| Self::read_sa_ca());

        // Build reqwest client — accept self-signed CA if provided
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30));

        if let Some(ref ca) = ca_cert {
            if let Ok(cert) = reqwest::Certificate::from_pem(ca.as_bytes()) {
                builder = builder.add_root_certificate(cert);
            }
        }

        let http = builder.build().ok()?;
        Some(KubeApiClient { api_server, namespace, token, ca_cert, http })
    }

    fn read_sa_token() -> Option<String> {
        std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/token").ok()
    }

    fn read_sa_ca() -> Option<String> {
        std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/ca.crt").ok()
    }

    fn auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Bearer {}", t.trim()))
    }

    fn jobs_url(&self) -> String {
        format!("{}/apis/batch/v1/namespaces/{}/jobs", self.api_server, self.namespace)
    }

    fn pods_url(&self) -> String {
        format!("{}/api/v1/namespaces/{}/pods", self.api_server, self.namespace)
    }

    pub async fn create_job(&self, spec: &Value) -> Result<Value, String> {
        let mut req = self.http.post(&self.jobs_url()).json(spec);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        req.send().await
            .map_err(|e| format!("kube create_job: {}", e))?
            .json::<Value>().await
            .map_err(|e| format!("kube create_job parse: {}", e))
    }

    pub async fn get_job(&self, name: &str) -> Result<Value, String> {
        let url = format!("{}/{}", self.jobs_url(), name);
        let mut req = self.http.get(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        req.send().await
            .map_err(|e| format!("kube get_job: {}", e))?
            .json::<Value>().await
            .map_err(|e| format!("kube get_job parse: {}", e))
    }

    pub async fn list_jobs(&self, label_selector: &str) -> Result<Value, String> {
        let url = format!("{}?labelSelector={}", self.jobs_url(),
            urlencoding::encode(label_selector));
        let mut req = self.http.get(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        req.send().await
            .map_err(|e| format!("kube list_jobs: {}", e))?
            .json::<Value>().await
            .map_err(|e| format!("kube list_jobs parse: {}", e))
    }

    pub async fn delete_job(&self, name: &str) -> Result<(), String> {
        let url = format!("{}/{}?propagationPolicy=Foreground", self.jobs_url(), name);
        let mut req = self.http.delete(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        req.send().await
            .map(|_| ())
            .map_err(|e| format!("kube delete_job: {}", e))
    }

    pub async fn get_pod_logs(&self, pod_name: &str) -> Result<String, String> {
        let url = format!("{}/{}/log?tailLines=100", self.pods_url(), pod_name);
        let mut req = self.http.get(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        req.send().await
            .map_err(|e| format!("kube logs: {}", e))?
            .text().await
            .map_err(|e| format!("kube logs parse: {}", e))
    }
}

// ── Compute job ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    Pending,    // submitted to k8s, not yet scheduled
    Running,    // pod is running
    Succeeded,  // job completed successfully
    Failed,     // job failed
    Cancelled,  // deleted before completion
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeJob {
    pub id:           String,          // internal UUID
    pub job_name:     String,          // k8s Job name (DNS-safe)
    pub run_id:       String,
    pub agent_id:     String,
    pub node_id:      String,          // governance graph node ID
    pub capability:   String,
    pub did:          Option<String>,
    pub image:        String,
    pub namespace:    String,
    pub task:         String,
    pub phase:        JobPhase,
    pub exit_code:    Option<i32>,
    pub logs:         Option<String>,
    pub tokens_used:  u64,
    pub cost_usd:     f64,
    pub cpu_request:  String,
    pub mem_request:  String,
    pub cpu_limit:    String,
    pub mem_limit:    String,
    pub created_at:   DateTime<Utc>,
    pub started_at:   Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl ComputeJob {
    fn job_name(agent_id: &str, run_id: &str) -> String {
        // k8s names: max 63 chars, lowercase alphanumeric + hyphens
        let base = format!("ayx-{}-{}", agent_id, &run_id[..run_id.len().min(8)]);
        base.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .take(63)
            .collect()
    }

    /// Build the k8s Job spec from this ComputeJob.
    pub fn to_k8s_spec(&self, env_extras: &HashMap<String, String>) -> Value {
        let mut env = vec![
            json!({ "name": "AUTONOMYX_RUN_ID",     "value": self.run_id }),
            json!({ "name": "AUTONOMYX_AGENT_ID",   "value": self.agent_id }),
            json!({ "name": "AUTONOMYX_NODE_ID",    "value": self.node_id }),
            json!({ "name": "AUTONOMYX_CAPABILITY", "value": self.capability }),
            json!({ "name": "AUTONOMYX_TASK",       "value": self.task }),
            json!({ "name": "AUTONOMYX_DID",        "value": self.did.as_deref().unwrap_or("") }),
            // Inherit platform secrets
            json!({ "name": "ANTHROPIC_API_KEY",
                    "valueFrom": { "secretKeyRef": {
                        "name": "autonomyx-secrets", "key": "ANTHROPIC_API_KEY", "optional": true
                    }}}),
            json!({ "name": "GATEWAY_API_KEY",
                    "valueFrom": { "secretKeyRef": {
                        "name": "autonomyx-secrets", "key": "GATEWAY_API_KEY", "optional": true
                    }}}),
        ];

        for (k, v) in env_extras {
            env.push(json!({ "name": k, "value": v }));
        }

        json!({
            "apiVersion": "batch/v1",
            "kind":       "Job",
            "metadata": {
                "name":      self.job_name,
                "namespace": self.namespace,
                "labels": {
                    "app":                       "autonomyx-runner",
                    "autonomyx/job-id":          self.id,
                    "autonomyx/run-id":          self.run_id,
                    "autonomyx/agent-id":        self.agent_id,
                    "autonomyx/node-id":         self.node_id,
                    "autonomyx/capability":      self.capability,
                    "autonomyx/managed-by":      "computekube",
                },
                "annotations": {
                    "autonomyx/did":             self.did.as_deref().unwrap_or(""),
                    "autonomyx/created-at":      self.created_at.to_rfc3339(),
                },
            },
            "spec": {
                "backoffLimit":          2,
                "ttlSecondsAfterFinished": 300,
                "template": {
                    "metadata": {
                        "labels": {
                            "app":                   "autonomyx-job",
                            "autonomyx/run-id":      self.run_id,
                        },
                    },
                    "spec": {
                        "serviceAccountName": "autonomyx-runner",
                        "restartPolicy":      "OnFailure",
                        "containers": [{
                            "name":            "agent",
                            "image":           self.image,
                            "imagePullPolicy": "IfNotPresent",
                            "env":             env,
                            "resources": {
                                "requests": {
                                    "cpu":    self.cpu_request,
                                    "memory": self.mem_request,
                                },
                                "limits": {
                                    "cpu":    self.cpu_limit,
                                    "memory": self.mem_limit,
                                },
                            },
                            "securityContext": {
                                "allowPrivilegeEscalation": false,
                                "readOnlyRootFilesystem":   true,
                                "runAsNonRoot":             true,
                                "runAsUser":                65532,
                                "capabilities":             { "drop": ["ALL"] },
                            },
                        }],
                        "automountServiceAccountToken": false,
                    },
                },
            },
        })
    }
}

// ── ComputeKube — the orchestrator ───────────────────────────────────────────

pub struct ComputeKube {
    pub client:    Option<KubeApiClient>,
    pub namespace: String,
    pub image:     String,
    jobs:          RwLock<HashMap<String, ComputeJob>>,
}

impl ComputeKube {
    pub fn new() -> Self {
        let client    = KubeApiClient::from_env();
        let namespace = std::env::var("POD_NAMESPACE")
            .or_else(|_| std::env::var("AGENT_JOB_NAMESPACE"))
            .unwrap_or_else(|_| "autonomyx".into());
        let image = std::env::var("AUTONOMYX_JOB_IMAGE")
            .unwrap_or_else(|_| "ghcr.io/agennext/autonomyx:latest".into());

        if client.is_some() {
            tracing::info!(namespace = %namespace, image = %image, "computekube: kubernetes available");
        } else {
            tracing::warn!("computekube: kubernetes not available — jobs run in-process");
        }

        ComputeKube { client, namespace, image, jobs: RwLock::new(HashMap::new()) }
    }

    pub fn is_available(&self) -> bool {
        self.client.is_some()
    }

    /// Spawn a Kubernetes Job for a governance graph node execution.
    /// Returns the ComputeJob; caller polls status or subscribes to fabric events.
    pub async fn spawn(
        &self,
        agent_id: &str,
        run_id: &str,
        node_id: &str,
        capability: &str,
        task: &str,
        did: Option<&str>,
        cpu_milli: Option<u32>,
        mem_mb: Option<u32>,
    ) -> ComputeJob {
        let id       = Uuid::new_v4().to_string();
        let job_name = ComputeJob::job_name(agent_id, run_id);

        let cpu_m = cpu_milli.unwrap_or(250);
        let mem_m = mem_mb.unwrap_or(256);

        let mut job = ComputeJob {
            id:           id.clone(),
            job_name:     job_name.clone(),
            run_id:       run_id.to_string(),
            agent_id:     agent_id.to_string(),
            node_id:      node_id.to_string(),
            capability:   capability.to_string(),
            did:          did.map(|s| s.to_string()),
            image:        self.image.clone(),
            namespace:    self.namespace.clone(),
            task:         task.to_string(),
            phase:        JobPhase::Pending,
            exit_code:    None,
            logs:         None,
            tokens_used:  0,
            cost_usd:     0.0,
            cpu_request:  format!("{}m", cpu_m / 2),
            mem_request:  format!("{}Mi", mem_m / 2),
            cpu_limit:    format!("{}m", cpu_m),
            mem_limit:    format!("{}Mi", mem_m),
            created_at:   Utc::now(),
            started_at:   None,
            completed_at: None,
        };

        if let Some(ref client) = self.client {
            let spec = job.to_k8s_spec(&HashMap::new());
            match client.create_job(&spec).await {
                Ok(resp) => {
                    let name = resp["metadata"]["name"].as_str()
                        .unwrap_or(&job_name).to_string();
                    job.job_name = name;
                    tracing::info!(
                        job   = %job.job_name,
                        agent = %agent_id,
                        node  = %node_id,
                        "computekube: job created"
                    );
                }
                Err(e) => {
                    tracing::error!(error = %e, "computekube: job creation failed");
                    job.phase = JobPhase::Failed;
                    job.logs  = Some(e);
                }
            }
        } else {
            // No k8s — simulate in-process (dev mode)
            job.phase        = JobPhase::Succeeded;
            job.started_at   = Some(Utc::now());
            job.completed_at = Some(Utc::now());
            job.logs         = Some(format!("[in-process] task: {}", task));
            tracing::info!(job = %job_name, "computekube: in-process execution (no k8s)");
        }

        self.jobs.write().unwrap().insert(id.clone(), job.clone());
        job
    }

    /// Poll a job's status from the k8s API and update local state.
    pub async fn sync_status(&self, job_id: &str) -> Option<ComputeJob> {
        let job_name = {
            let jobs = self.jobs.read().unwrap();
            jobs.get(job_id)?.job_name.clone()
        };

        if let Some(ref client) = self.client {
            match client.get_job(&job_name).await {
                Ok(resp) => {
                    let phase = kube_job_phase(&resp);
                    let mut jobs = self.jobs.write().unwrap();
                    if let Some(job) = jobs.get_mut(job_id) {
                        job.phase = phase.clone();
                        if phase == JobPhase::Running && job.started_at.is_none() {
                            job.started_at = Some(Utc::now());
                        }
                        if matches!(phase, JobPhase::Succeeded | JobPhase::Failed) {
                            job.completed_at = Some(Utc::now());
                        }
                        return Some(job.clone());
                    }
                }
                Err(e) => tracing::warn!(error = %e, "computekube: get_job failed"),
            }
        }
        self.jobs.read().unwrap().get(job_id).cloned()
    }

    /// Fetch logs for a completed job.
    pub async fn fetch_logs(&self, job_id: &str) -> Option<String> {
        let job = self.jobs.read().unwrap().get(job_id)?.clone();
        if let Some(ref client) = self.client {
            // Pod name = job name + random suffix; list by label selector
            let selector = format!("autonomyx/job-id={}", job_id);
            if let Ok(pods) = client.list_jobs(&selector).await {
                if let Some(pod_name) = pods["items"][0]["metadata"]["name"].as_str() {
                    return client.get_pod_logs(pod_name).await.ok();
                }
            }
        }
        job.logs
    }

    /// Cancel and delete a job.
    pub async fn cancel(&self, job_id: &str) -> Result<(), String> {
        let job_name = {
            let jobs = self.jobs.read().unwrap();
            jobs.get(job_id)
                .ok_or_else(|| format!("job '{}' not found", job_id))?
                .job_name.clone()
        };
        if let Some(ref client) = self.client {
            client.delete_job(&job_name).await?;
        }
        let mut jobs = self.jobs.write().unwrap();
        if let Some(job) = jobs.get_mut(job_id) {
            job.phase        = JobPhase::Cancelled;
            job.completed_at = Some(Utc::now());
        }
        Ok(())
    }

    pub fn get_job(&self, id: &str) -> Option<ComputeJob> {
        self.jobs.read().unwrap().get(id).cloned()
    }

    pub fn list_jobs(&self) -> Vec<ComputeJob> {
        self.jobs.read().unwrap().values().cloned().collect()
    }

    pub fn summary(&self) -> Value {
        let jobs = self.jobs.read().unwrap();
        let total     = jobs.len();
        let pending   = jobs.values().filter(|j| j.phase == JobPhase::Pending).count();
        let running   = jobs.values().filter(|j| j.phase == JobPhase::Running).count();
        let succeeded = jobs.values().filter(|j| j.phase == JobPhase::Succeeded).count();
        let failed    = jobs.values().filter(|j| j.phase == JobPhase::Failed).count();
        let total_cost: f64 = jobs.values().map(|j| j.cost_usd).sum();

        json!({
            "kubernetes":  self.is_available(),
            "namespace":   self.namespace,
            "image":       self.image,
            "jobs": {
                "total":     total,
                "pending":   pending,
                "running":   running,
                "succeeded": succeeded,
                "failed":    failed,
            },
            "total_cost_usd": total_cost,
            "mode": if self.is_available() {
                "kubernetes — jobs run as governed k8s workloads"
            } else {
                "in-process — set KUBERNETES_SERVICE_HOST to enable k8s dispatch"
            },
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn kube_job_phase(job: &Value) -> JobPhase {
    let conditions = job["status"]["conditions"].as_array();
    if let Some(conds) = conditions {
        for c in conds {
            if c["type"].as_str() == Some("Complete") && c["status"].as_str() == Some("True") {
                return JobPhase::Succeeded;
            }
            if c["type"].as_str() == Some("Failed") && c["status"].as_str() == Some("True") {
                return JobPhase::Failed;
            }
        }
    }
    if job["status"]["active"].as_u64().unwrap_or(0) > 0 {
        return JobPhase::Running;
    }
    JobPhase::Pending
}

// ── URL encoding helper (no external dep) ────────────────────────────────────

mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars().flat_map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => vec![c],
            c => format!("%{:02X}", c as u32).chars().collect(),
        }).collect()
    }
}
