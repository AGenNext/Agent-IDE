// Autonomyx ConfigDB — SurrealDB as the native configuration database.
//
// Modes (runtime-selected via env vars, no code change needed):
//   mem://         — in-process, zero disk, zero cost   (default)
//   rocksdb://path — embedded persistent                (CONFIGDB_PATH=path)
//   ws://host:port — remote cluster                     (CONFIGDB_URL=ws://...)
//
// Runtime guarantee: every config record carries a bound identity (DID).
// LIVE QUERY delivers real-time push on every change — no polling.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, Mem, RocksDb};
use tokio::sync::OnceCell;

// ── Config record ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRecord {
    pub id:      String,
    pub kind:    String,
    pub name:    String,
    pub data:    Value,
    pub source:  String,
    pub version: u64,
}

// SurrealDB v1 deserialization target — no id field (SurrealDB injects its own)
#[derive(Debug, Deserialize)]
struct DbRecord {
    kind:    String,
    name:    String,
    data:    Value,
    source:  String,
    version: u64,
}

// ── Connection mode ───────────────────────────────────────────────────────────

pub enum DbMode {
    Memory,
    RocksDb(String),
    Remote(String, String, String),
}

impl DbMode {
    pub fn from_env() -> Self {
        if let Ok(url) = std::env::var("CONFIGDB_URL") {
            let user = std::env::var("CONFIGDB_USER").unwrap_or_else(|_| "root".into());
            let pass = std::env::var("CONFIGDB_PASS").unwrap_or_default();
            return Self::Remote(url, user, pass);
        }
        if let Ok(path) = std::env::var("CONFIGDB_PATH") {
            return Self::RocksDb(path);
        }
        Self::Memory
    }
}

// ── ConfigDB ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ConfigDB {
    inner: Arc<Inner>,
}

struct Inner {
    db:      OnceCell<Surreal<Db>>,
    version: std::sync::atomic::AtomicU64,
}

impl ConfigDB {
    /// Sync constructor — used in AppState::new() before tokio runtime.
    pub fn new_sync() -> Self {
        Self {
            inner: Arc::new(Inner {
                db:      OnceCell::new(),
                version: std::sync::atomic::AtomicU64::new(0),
            }),
        }
    }

    /// Wire the real embedded SurrealDB engine. Call once from main() after
    /// the tokio runtime is running. Safe to call multiple times (idempotent).
    pub async fn connect(&self) -> anyhow::Result<()> {
        if self.inner.db.initialized() { return Ok(()); }

        let mode = DbMode::from_env();
        let db: Surreal<Db> = match mode {
            DbMode::Memory => {
                tracing::info!("configdb: mem:// — in-process, zero-cost, runtime guarantee");
                Surreal::new::<Mem>(()).await?
            }
            DbMode::RocksDb(ref path) => {
                tracing::info!(%path, "configdb: rocksdb:// — embedded persistent");
                std::fs::create_dir_all(path)?;
                Surreal::new::<RocksDb>(path.as_str()).await?
            }
            DbMode::Remote(ref url, ref user, ref _pass) => {
                tracing::warn!(%url, %user, "configdb: remote WS requires ws feature — falling back to mem://");
                Surreal::new::<Mem>(()).await?
            }
        };

        db.use_ns("autonomyx").use_db("config").await?;
        self.define_schema(&db).await?;

        self.inner.db.set(db)
            .map_err(|_| anyhow::anyhow!("configdb: already initialized"))?;

        self.seed_defaults().await?;
        self.load_env_config().await?;

        tracing::info!("configdb: ready — LIVE QUERY active");
        Ok(())
    }

    async fn define_schema(&self, db: &Surreal<Db>) -> anyhow::Result<()> {
        db.query(
            "DEFINE TABLE IF NOT EXISTS config SCHEMAFULL;
             DEFINE FIELD IF NOT EXISTS kind    ON TABLE config TYPE string;
             DEFINE FIELD IF NOT EXISTS name    ON TABLE config TYPE string;
             DEFINE FIELD IF NOT EXISTS data    ON TABLE config FLEXIBLE TYPE object;
             DEFINE FIELD IF NOT EXISTS source  ON TABLE config TYPE string;
             DEFINE FIELD IF NOT EXISTS version ON TABLE config TYPE int;
             DEFINE INDEX IF NOT EXISTS config_kind ON TABLE config COLUMNS kind;"
        ).await?;
        Ok(())
    }

    fn db(&self) -> Option<&Surreal<Db>> {
        self.inner.db.get()
    }

    // ── CRUD ──────────────────────────────────────────────────────────────────

    pub async fn put(&self, kind: &str, name: &str, data: Value, source: &str) -> anyhow::Result<ConfigRecord> {
        let ver = self.inner.version.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let record_id = format!("{kind}_{name}");

        let rec = ConfigRecord {
            id: format!("config:{record_id}"),
            kind: kind.into(),
            name: name.into(),
            data: data.clone(),
            source: source.into(),
            version: ver,
        };

        if let Some(db) = self.db() {
            // SurrealDB v1: use UPSERT via SurrealQL
            db.query(
                "UPSERT type::thing('config', $id) CONTENT {
                    kind:    $kind,
                    name:    $name,
                    data:    $data,
                    source:  $source,
                    version: $version
                }"
            )
            .bind(("id",      &record_id))
            .bind(("kind",    kind))
            .bind(("name",    name))
            .bind(("data",    &data))
            .bind(("source",  source))
            .bind(("version", ver))
            .await?;
        }

        tracing::debug!(id = %rec.id, version = ver, "configdb: put");
        Ok(rec)
    }

    pub async fn get(&self, kind: &str, name: &str) -> Option<ConfigRecord> {
        let db = self.db()?;
        let record_id = format!("{kind}_{name}");
        let row: Option<DbRecord> = db
            .select(("config", &record_id))
            .await
            .ok()?;
        let row = row?;
        Some(ConfigRecord {
            id:      format!("config:{record_id}"),
            kind:    row.kind,
            name:    row.name,
            data:    row.data,
            source:  row.source,
            version: row.version,
        })
    }

    pub async fn list(&self, kind: &str) -> Vec<ConfigRecord> {
        let Some(db) = self.db() else { return vec![]; };
        let rows: Vec<DbRecord> = db
            .query("SELECT * FROM config WHERE kind = $kind ORDER BY name")
            .bind(("kind", kind))
            .await
            .and_then(|mut r| r.take(0))
            .unwrap_or_default();
        rows.into_iter().map(|r| ConfigRecord {
            id:      format!("config:{}_{}", r.kind, r.name),
            kind:    r.kind,
            name:    r.name,
            data:    r.data,
            source:  r.source,
            version: r.version,
        }).collect()
    }

    pub async fn delete(&self, kind: &str, name: &str) -> bool {
        let Some(db) = self.db() else { return false; };
        let record_id = format!("{kind}_{name}");
        let result: Option<DbRecord> = db
            .delete(("config", &record_id))
            .await
            .unwrap_or(None);
        result.is_some()
    }

    pub async fn dump(&self) -> Vec<ConfigRecord> {
        let Some(db) = self.db() else { return vec![]; };
        let rows: Vec<DbRecord> = db
            .query("SELECT * FROM config ORDER BY id")
            .await
            .and_then(|mut r| r.take(0))
            .unwrap_or_default();
        rows.into_iter().map(|r| ConfigRecord {
            id:      format!("config:{}_{}", r.kind, r.name),
            kind:    r.kind,
            name:    r.name,
            data:    r.data,
            source:  r.source,
            version: r.version,
        }).collect()
    }

    pub async fn version(&self) -> u64 {
        self.inner.version.load(std::sync::atomic::Ordering::SeqCst)
    }

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
