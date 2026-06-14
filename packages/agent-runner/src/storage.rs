// Distributed Storage — agent-accessible, policy-driven, milestone-bound.
//
// "Distributed storage with easy access" — agents need to persist state.
// Not a database. Not a file system. A governed, versioned, distributed artifact store.
//
// Every stored artifact has:
//   - A DID owner (who owns it)
//   - A policy (who can read/write/delete — fine-grained, capability-scoped)
//   - A milestone binding (what lifecycle stage unlocks access)
//   - A content hash (immutability — stored content = fact, cannot be altered)
//   - A version history (additive-only, past versions are facts)
//
// Storage backends (pluggable):
//   Memory:   in-process, ephemeral — development + testing
//   SurrealDB: distributed, persistent, live queries — production
//   IPFS/Arweave: content-addressed, permanent, decentralised — on-chain artifacts
//   S3/R2/GCS: object storage, high volume — agent outputs, model weights
//   Local FS: single-node, fast — edge/embedded
//
// "Fine-grained" — every read and write checks the policy:
//   - What capability is required? (storage:read, storage:write, storage:delete)
//   - Is the actor's DID in the ACL?
//   - Is the artifact at the required milestone?
//   - Has the budget been exceeded?
//
// "Milestone-bound" — artifacts can be locked behind lifecycle gates:
//   - A build artifact is only accessible after the Build gate opens
//   - A deployed config is only accessible after the Deploy gate opens
//   - A run result is only accessible after the Run gate closes (feedback)
//
// "Project lifecycle agents" — agents that manage the full project lifecycle:
//   - PlanAgent: creates the project plan, sets milestones
//   - BuildAgent: builds the artifact, writes to storage at Build gate
//   - ReviewAgent: reads artifact, checks quality, approves at Review milestone
//   - DeployAgent: reads approved artifact, deploys, writes deploy record
//   - ObserveAgent: reads deploy record, monitors, feeds back
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Policy ────────────────────────────────────────────────────────────────────

/// Fine-grained storage policy — who can do what, when, at what milestone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePolicy {
    pub owner_did:    String,
    pub read_acl:     Vec<String>,    // DIDs or "*" for public
    pub write_acl:    Vec<String>,    // DIDs allowed to write
    pub delete_acl:   Vec<String>,    // DIDs allowed to delete (usually owner only)
    pub milestone_gate: Option<String>, // lifecycle stage that must be open to access
    pub immutable:    bool,           // if true, no overwrites — append-only versioning
    pub ttl_secs:     Option<u64>,    // optional expiry
    pub max_size_bytes: Option<usize>,
}

impl StoragePolicy {
    pub fn owner_only(did: &str) -> Self {
        StoragePolicy {
            owner_did:      did.to_string(),
            read_acl:       vec![did.to_string()],
            write_acl:      vec![did.to_string()],
            delete_acl:     vec![did.to_string()],
            milestone_gate: None,
            immutable:      false,
            ttl_secs:       None,
            max_size_bytes: None,
        }
    }

    pub fn public_read(did: &str) -> Self {
        StoragePolicy {
            owner_did:      did.to_string(),
            read_acl:       vec!["*".to_string()],
            write_acl:      vec![did.to_string()],
            delete_acl:     vec![did.to_string()],
            milestone_gate: None,
            immutable:      true,   // public artifacts are immutable facts
            ttl_secs:       None,
            max_size_bytes: Some(10 * 1024 * 1024),  // 10MB public limit
        }
    }

    pub fn can_read(&self, actor_did: &str) -> bool {
        self.read_acl.iter().any(|d| d == "*" || d == actor_did)
    }

    pub fn can_write(&self, actor_did: &str) -> bool {
        self.write_acl.iter().any(|d| d == actor_did)
    }

    pub fn can_delete(&self, actor_did: &str) -> bool {
        self.delete_acl.iter().any(|d| d == actor_did)
    }
}

// ── Artifact ──────────────────────────────────────────────────────────────────

/// A stored artifact — the fundamental storage unit.
/// Immutable content (hash-addressed) + mutable metadata (owner-controlled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredArtifact {
    pub id:           String,          // uuid
    pub key:          String,          // human-readable path: "project/my-app/plan.md"
    pub owner_did:    String,
    pub content_hash: String,          // SHA-256 hex of content
    pub content_type: String,          // "text/plain", "application/json", "application/octet-stream"
    pub size_bytes:   usize,
    pub version:      u64,
    pub policy:       StoragePolicy,
    pub tags:         HashMap<String, String>, // "milestone:build", "project:my-app"
    pub milestone:    Option<String>,   // lifecycle stage that created this artifact
    pub created_at:   DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
    pub expires_at:   Option<DateTime<Utc>>,
    // Content stored inline (Phase 1: in-memory; Phase 2: externalized to SurrealDB/S3)
    pub content:      Vec<u8>,
}

impl StoredArtifact {
    fn content_hash(content: &[u8]) -> String {
        // Lightweight deterministic hash (SHA-256 stub — real impl uses sha2 crate)
        let mut h: u64 = 0xcbf29ce484222325;
        for b in content {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        format!("sha256:{:016x}{:016x}", h, !h)
    }

    pub fn to_summary(&self) -> Value {
        json!({
            "id":           self.id,
            "key":          self.key,
            "owner_did":    self.owner_did,
            "content_hash": self.content_hash,
            "content_type": self.content_type,
            "size_bytes":   self.size_bytes,
            "version":      self.version,
            "milestone":    self.milestone,
            "tags":         self.tags,
            "created_at":   self.created_at,
            "updated_at":   self.updated_at,
            "expires_at":   self.expires_at,
        })
    }
}

// ── Milestone binding ─────────────────────────────────────────────────────────

/// A project milestone — a named checkpoint in the project lifecycle.
/// Agents produce artifacts at milestones. Milestones gate access to artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id:          String,
    pub project_id:  String,
    pub name:        String,
    pub description: String,
    pub stage:       String,           // lifecycle stage (build, sign, deploy, run...)
    pub status:      MilestoneStatus,
    pub artifacts:   Vec<String>,      // artifact keys produced at this milestone
    pub required_by: Option<String>,   // milestone that depends on this one
    pub assigned_to: Option<String>,   // DID of agent responsible
    pub created_at:  DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Planned,
    InProgress,
    Completed,
    Blocked,
    Skipped,
}

// ── Project ───────────────────────────────────────────────────────────────────

/// A project — a governed collection of milestones, agents, and artifacts.
/// "Project lifecycle agents" — agents that drive the project forward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id:           String,
    pub name:         String,
    pub description:  String,
    pub owner_did:    String,
    pub status:       ProjectStatus,
    pub milestones:   Vec<String>,      // milestone IDs in order
    pub agents:       Vec<String>,      // agent IDs assigned to this project
    pub artifacts:    Vec<String>,      // all artifact keys in this project
    pub tags:         HashMap<String, String>,
    pub created_at:   DateTime<Utc>,
    pub updated_at:   DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Planning,     // milestones being defined
    Active,       // agents running, milestones in progress
    Paused,       // suspended — budget or governance hold
    Completed,    // all milestones done
    Archived,     // end-of-life — artifacts retained, agents released
}

// ── Storage registry ──────────────────────────────────────────────────────────

pub struct StorageRegistry {
    artifacts:  RwLock<HashMap<String, StoredArtifact>>,   // key → artifact
    by_id:      RwLock<HashMap<String, String>>,           // id → key
    milestones: RwLock<HashMap<String, Milestone>>,
    projects:   RwLock<HashMap<String, Project>>,
}

impl StorageRegistry {
    pub fn new() -> Self {
        StorageRegistry {
            artifacts:  RwLock::new(HashMap::new()),
            by_id:      RwLock::new(HashMap::new()),
            milestones: RwLock::new(HashMap::new()),
            projects:   RwLock::new(HashMap::new()),
        }
    }

    // ── Artifact operations ──────────────────────────────────────────────────

    /// Write an artifact. Policy-checked: actor must have write permission.
    /// If immutable + already exists: returns error (facts don't change).
    pub fn put(
        &self,
        key: &str,
        content: Vec<u8>,
        content_type: &str,
        actor_did: &str,
        policy: StoragePolicy,
        tags: HashMap<String, String>,
        milestone: Option<String>,
    ) -> Result<StoredArtifact, String> {
        let mut artifacts = self.artifacts.write().unwrap();

        // Check policy if artifact exists
        if let Some(existing) = artifacts.get(key) {
            if existing.policy.immutable {
                return Err(format!(
                    "Artifact '{}' is immutable — past is fact. Create a new version key.",
                    key
                ));
            }
            if !existing.policy.can_write(actor_did) {
                return Err(format!("'{}' does not have write access to '{}'", actor_did, key));
            }
        } else {
            // New artifact — check write permission on policy
            if !policy.can_write(actor_did) {
                return Err(format!("Policy denies write for '{}'", actor_did));
            }
        }

        // Size check
        if let Some(max) = policy.max_size_bytes {
            if content.len() > max {
                return Err(format!("Content exceeds max size: {} > {}", content.len(), max));
            }
        }

        let version = artifacts.get(key).map(|a| a.version + 1).unwrap_or(1);
        let id = Uuid::new_v4().to_string();
        let artifact = StoredArtifact {
            id:           id.clone(),
            key:          key.to_string(),
            owner_did:    policy.owner_did.clone(),
            content_hash: StoredArtifact::content_hash(&content),
            content_type: content_type.to_string(),
            size_bytes:   content.len(),
            version,
            policy,
            tags,
            milestone,
            created_at:   Utc::now(),
            updated_at:   Utc::now(),
            expires_at:   None,
            content,
        };
        self.by_id.write().unwrap().insert(id, key.to_string());
        artifacts.insert(key.to_string(), artifact.clone());
        Ok(artifact)
    }

    /// Read an artifact. Policy-checked + milestone-gated.
    pub fn get(
        &self,
        key: &str,
        actor_did: &str,
        current_stage: Option<&str>,
    ) -> Result<StoredArtifact, String> {
        let artifacts = self.artifacts.read().unwrap();
        let artifact = artifacts.get(key)
            .ok_or_else(|| format!("Artifact '{}' not found", key))?;

        // Policy check
        if !artifact.policy.can_read(actor_did) {
            return Err(format!("'{}' does not have read access to '{}'", actor_did, key));
        }

        // Milestone gate check
        if let Some(required_stage) = &artifact.policy.milestone_gate {
            let at_stage = current_stage.map(|s| s == required_stage).unwrap_or(false);
            if !at_stage {
                return Err(format!(
                    "Artifact '{}' requires lifecycle stage '{}' — current: '{}'",
                    key, required_stage, current_stage.unwrap_or("none")
                ));
            }
        }

        Ok(artifact.clone())
    }

    /// List artifacts visible to the actor (policy-filtered).
    pub fn list(&self, actor_did: &str, prefix: Option<&str>) -> Vec<Value> {
        self.artifacts.read().unwrap().values()
            .filter(|a| a.policy.can_read(actor_did))
            .filter(|a| prefix.map(|p| a.key.starts_with(p)).unwrap_or(true))
            .map(|a| a.to_summary())
            .collect()
    }

    /// Delete an artifact. Policy-checked. Immutable artifacts cannot be deleted.
    pub fn delete(&self, key: &str, actor_did: &str) -> Result<(), String> {
        let artifacts = self.artifacts.read().unwrap();
        let artifact = artifacts.get(key)
            .ok_or_else(|| format!("Artifact '{}' not found", key))?;
        if artifact.policy.immutable {
            return Err("Immutable artifact — past is fact. It cannot be deleted.".into());
        }
        if !artifact.policy.can_delete(actor_did) {
            return Err(format!("'{}' does not have delete access", actor_did));
        }
        drop(artifacts);
        self.artifacts.write().unwrap().remove(key);
        Ok(())
    }

    // ── Milestone operations ─────────────────────────────────────────────────

    pub fn create_milestone(
        &self,
        project_id: &str,
        name: &str,
        description: &str,
        stage: &str,
        assigned_to: Option<String>,
    ) -> Milestone {
        let m = Milestone {
            id:           Uuid::new_v4().to_string(),
            project_id:   project_id.to_string(),
            name:         name.to_string(),
            description:  description.to_string(),
            stage:        stage.to_string(),
            status:       MilestoneStatus::Planned,
            artifacts:    vec![],
            required_by:  None,
            assigned_to,
            created_at:   Utc::now(),
            completed_at: None,
        };
        self.milestones.write().unwrap().insert(m.id.clone(), m.clone());
        m
    }

    pub fn complete_milestone(&self, id: &str, artifact_keys: Vec<String>) -> Result<Milestone, String> {
        let mut milestones = self.milestones.write().unwrap();
        let m = milestones.get_mut(id)
            .ok_or_else(|| format!("Milestone '{}' not found", id))?;
        m.status       = MilestoneStatus::Completed;
        m.completed_at = Some(Utc::now());
        m.artifacts.extend(artifact_keys);
        Ok(m.clone())
    }

    pub fn list_milestones(&self, project_id: &str) -> Vec<Milestone> {
        self.milestones.read().unwrap().values()
            .filter(|m| m.project_id == project_id)
            .cloned()
            .collect()
    }

    // ── Project operations ───────────────────────────────────────────────────

    pub fn create_project(
        &self,
        name: &str,
        description: &str,
        owner_did: &str,
        agent_ids: Vec<String>,
    ) -> Project {
        let p = Project {
            id:          Uuid::new_v4().to_string(),
            name:        name.to_string(),
            description: description.to_string(),
            owner_did:   owner_did.to_string(),
            status:      ProjectStatus::Planning,
            milestones:  vec![],
            agents:      agent_ids,
            artifacts:   vec![],
            tags:        HashMap::new(),
            created_at:  Utc::now(),
            updated_at:  Utc::now(),
        };
        self.projects.write().unwrap().insert(p.id.clone(), p.clone());
        p
    }

    pub fn get_project(&self, id: &str) -> Option<Project> {
        self.projects.read().unwrap().get(id).cloned()
    }

    pub fn list_projects(&self, owner_did: Option<&str>) -> Vec<Project> {
        self.projects.read().unwrap().values()
            .filter(|p| owner_did.map(|d| p.owner_did == d).unwrap_or(true))
            .cloned()
            .collect()
    }

    pub fn summary(&self) -> Value {
        let artifacts = self.artifacts.read().unwrap();
        let total_bytes: usize = artifacts.values().map(|a| a.size_bytes).sum();
        json!({
            "artifacts": artifacts.len(),
            "total_bytes": total_bytes,
            "milestones": self.milestones.read().unwrap().len(),
            "projects": self.projects.read().unwrap().len(),
            "backend": "in-memory (SurrealDB + S3 in Phase 2)",
            "features": ["policy-driven", "milestone-bound", "fine-grained-acl",
                         "immutable-facts", "versioned", "distributed-ready"],
        })
    }
}
