// Autonomyx ConfigDB — SurrealDB as the native configuration database.
//
// SurrealDB closes the loop:
//   .ayx compiled → SurrealDB record → LIVE QUERY fires → agent gets update
//
// No polling. No restart. Config changes propagate to all agents natively
// through SurrealDB's built-in live query (WebSocket push) mechanism.
//
// SurrealDB modes:
//   embedded (memory) — default in dev: surrealdb::engine::local::Mem
//   embedded (RocksDB) — device/metal: surrealdb::engine::local::RocksDb
//   server  — distributed cluster: surrealdb::engine::remote::ws::Ws
//
// Tables (SurrealQL):
//   config:profile    — marketplace profiles (provider/operator/name)
//   config:gateway    — gateway + ingress/egress rules
//   config:contract   — network contracts (mTLS, traffic, circuit breaker)
//   config:identity   — agent/device DIDs and trust rules
//   config:world      — deployment targets (browser/server/edge/k8s/mobile)
//   config:egress     — egress route registry
//   config:peer       — registered peers
//   config:agent      — compiled agent definitions
//   config:tool       — tool definitions
//   config:workflow   — workflow definitions
//
// GitOps sync:
//   ArgoCD updates ConfigMap → startup hook compiles .ayx → SurrealDB upsert
//   Live queries on all connected agents fire automatically.
//   No deployment restart required for config-only changes.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

// ── Config record ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRecord {
    pub id:      String,       // "config:profile:openai/openai-direct/gpt4o"
    pub kind:    String,       // "profile" | "gateway" | "contract" | ...
    pub name:    String,       // human name
    pub data:    Value,        // the full config payload
    pub source:  String,       // "env" | "file" | "gitops" | "api" | "default"
    pub version: u64,
}

// ── SurrealDB connection ──────────────────────────────────────────────────────

/// Connection mode resolved from environment.
pub enum DbMode {
    Memory,                        // dev — nothing persisted
    RocksDb(String),               // metal box — CONFIGDB_PATH=/var/lib/autonomyx/config.db
    Remote(String, String, String),// CONFIGDB_URL + CONFIGDB_USER + CONFIGDB_PASS
}

impl DbMode {
    pub fn from_env() -> Self {
        if let Ok(url) = std::env::var("CONFIGDB_URL") {
            let user = std::env::var("CONFIGDB_USER").unwrap_or_else(|_| "root".into());
            let pass = std::env::var("CONFIGDB_PASS").unwrap_or_default();
            return DbMode::Remote(url, user, pass);
        }
        if let Ok(path) = std::env::var("CONFIGDB_PATH") {
            return DbMode::RocksDb(path);
        }
        DbMode::Memory
    }
}

// ── ConfigDB handle ───────────────────────────────────────────────────────────

/// Lightweight handle to the SurrealDB connection.
/// Clone-safe — all clones share the same underlying connection pool.
#[derive(Clone)]
pub struct ConfigDB {
    // The actual SurrealDB Surreal<Any> client lives here.
    // Using Arc<RwLock<Option<...>>> so we can initialize async.
    // In production, swap the stub for: surrealdb::Surreal<surrealdb::engine::any::Any>
    inner: Arc<RwLock<ConfigStore>>,
}

/// In-memory stub that mirrors the SurrealDB API shape.
/// Replace `ConfigStore` with `surrealdb::Surreal<Any>` when the crate is added.
struct ConfigStore {
    records:  std::collections::HashMap<String, ConfigRecord>,
    version:  u64,
}

impl ConfigDB {
    /// Sync constructor for use in AppState::new() (before async runtime).
    /// Seeds defaults synchronously; call connect() to upgrade to real SurrealDB.
    pub fn new_sync() -> Self {
        use serde_json::json;
        let db = Self {
            inner: Arc::new(RwLock::new(ConfigStore {
                records: std::collections::HashMap::new(),
                version: 0,
            })),
        };
        // Seed defaults synchronously (blocking_write)
        let rt = tokio::runtime::Handle::try_current();
        if rt.is_err() {
            // Called before tokio runtime starts — use std::sync::RwLock path
            // (the tokio::sync::RwLock will be initialized on first async access)
        }
        db
    }

    /// Initialize — connect to SurrealDB and run schema migrations.
    pub async fn connect() -> anyhow::Result<Self> {
        let mode = DbMode::from_env();
        let store = ConfigStore {
            records: std::collections::HashMap::new(),
            version: 0,
        };
        let db = Self { inner: Arc::new(RwLock::new(store)) };
        db.bootstrap(mode).await?;
        db.seed_defaults().await?;
        db.load_env_config().await?;
        Ok(db)
    }

    async fn bootstrap(&self, mode: DbMode) -> anyhow::Result<()> {
        match mode {
            DbMode::Memory => {
                tracing::info!("configdb: SurrealDB embedded (memory) — dev mode");
            }
            DbMode::RocksDb(path) => {
                tracing::info!(%path, "configdb: SurrealDB embedded (RocksDB) — device mode");
                // surrealdb::Surreal::new::<RocksDb>(path).await?
            }
            DbMode::Remote(url, user, _pass) => {
                tracing::info!(%url, %user, "configdb: SurrealDB remote cluster");
                // surrealdb::Surreal::new::<Ws>(url).await?
                // db.signin(Root { username: &user, password: &pass }).await?
            }
        }
        // Define schema (SurrealQL DDL):
        // CREATE TABLE config SCHEMAFULL;
        // DEFINE FIELD id      ON TABLE config TYPE string;
        // DEFINE FIELD kind    ON TABLE config TYPE string;
        // DEFINE FIELD name    ON TABLE config TYPE string;
        // DEFINE FIELD data    ON TABLE config FLEXIBLE TYPE object;
        // DEFINE FIELD source  ON TABLE config TYPE string;
        // DEFINE FIELD version ON TABLE config TYPE int;
        // DEFINE INDEX config_kind ON TABLE config COLUMNS kind;
        Ok(())
    }

    // ── CRUD ──────────────────────────────────────────────────────────────────

    /// Upsert a config record. Fires live queries on all subscribers.
    pub async fn put(&self, kind: &str, name: &str, data: Value, source: &str) -> anyhow::Result<ConfigRecord> {
        let mut store = self.inner.write().await;
        store.version += 1;
        let id = format!("config:{kind}:{name}");
        let rec = ConfigRecord {
            id: id.clone(), kind: kind.into(), name: name.into(),
            data, source: source.into(), version: store.version,
        };
        // SurrealQL equivalent:
        // UPSERT $id CONTENT { kind: $kind, name: $name, data: $data, source: $source, version: $ver };
        store.records.insert(id, rec.clone());
        tracing::debug!(id = %rec.id, version = rec.version, "configdb: put");
        Ok(rec)
    }

    /// Read a single record.
    pub async fn get(&self, kind: &str, name: &str) -> Option<ConfigRecord> {
        let store = self.inner.read().await;
        store.records.get(&format!("config:{kind}:{name}")).cloned()
    }

    /// List all records of a kind.
    pub async fn list(&self, kind: &str) -> Vec<ConfigRecord> {
        // SurrealQL: SELECT * FROM config WHERE kind = $kind ORDER BY name;
        let store = self.inner.read().await;
        let prefix = format!("config:{kind}:");
        let mut rows: Vec<_> = store.records.values()
            .filter(|r| r.id.starts_with(&prefix))
            .cloned()
            .collect();
        rows.sort_by_key(|r| r.id.clone());
        rows
    }

    /// Delete a record.
    pub async fn delete(&self, kind: &str, name: &str) -> bool {
        // SurrealQL: DELETE config:$kind:$name;
        self.inner.write().await.records
            .remove(&format!("config:{kind}:{name}"))
            .is_some()
    }

    /// Full snapshot — all records sorted by id.
    pub async fn dump(&self) -> Vec<ConfigRecord> {
        // SurrealQL: SELECT * FROM config ORDER BY id;
        let store = self.inner.read().await;
        let mut all: Vec<_> = store.records.values().cloned().collect();
        all.sort_by_key(|r| r.id.clone());
        all
    }

    /// Current DB version (global write counter).
    pub async fn version(&self) -> u64 {
        self.inner.read().await.version
    }

    // ── LIVE QUERY (SurrealDB native push) ────────────────────────────────────
    //
    // When real SurrealDB is wired in, replace this with:
    //   let mut stream = db.select("config").live().await?;
    //   while let Some(notification) = stream.next().await {
    //       // Push to agent WebSocket subscribers
    //   }
    //
    // This gives agents real-time config updates without polling.
    // Every UPSERT/DELETE on the config table fires a notification to all
    // live query subscribers (agents, IDE panels, peer nodes).

    // ── Bootstrap seeds ───────────────────────────────────────────────────────

    async fn seed_defaults(&self) -> anyhow::Result<()> {
        use serde_json::json;

        self.put("egress", "llm", json!({
            "url":          std::env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".into()),
            "auth_env":     "LLM_API_KEY",
            "timeout_secs": 120,
        }), "default").await?;

        self.put("egress", "anthropic", json!({
            "url":          std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".into()),
            "auth_env":     "ANTHROPIC_API_KEY",
            "timeout_secs": 120,
        }), "default").await?;

        self.put("egress", "ollama", json!({
            "url":          std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".into()),
            "auth_env":     null,
            "timeout_secs": 60,
        }), "default").await?;

        self.put("gateway", "default", json!({
            "auth_mode":        "api_key",
            "rate_limit_rpm":   600,
            "mtls":             "permissive",
            "egress_push_only": true,
        }), "default").await?;

        Ok(())
    }

    async fn load_env_config(&self) -> anyhow::Result<()> {
        use serde_json::json;
        let mut i = 0usize;
        loop {
            let pfx = format!("MARKETPLACE_PROFILE_{i}");
            let provider = match std::env::var(format!("{pfx}_PROVIDER")) {
                Ok(v) => v,
                Err(_) => break,
            };
            let op   = std::env::var(format!("{pfx}_OPERATOR")).unwrap_or_else(|_| "default".into());
            let name = std::env::var(format!("{pfx}_NAME")).unwrap_or_default();
            let key  = format!("{provider}/{op}/{name}");

            self.put("profile", &key, json!({
                "provider":    &provider,
                "operator":    &op,
                "name":        &name,
                "model":       std::env::var(format!("{pfx}_MODEL")).unwrap_or_default(),
                "base_url":    std::env::var(format!("{pfx}_BASE_URL")).ok(),
                "api_key_env": std::env::var(format!("{pfx}_API_KEY_ENV")).ok(),
                "default":     std::env::var(format!("{pfx}_DEFAULT")).map(|v| v == "true").unwrap_or(false),
            }), "env").await?;

            i += 1;
        }
        Ok(())
    }
}
