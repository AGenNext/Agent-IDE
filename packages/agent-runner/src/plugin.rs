// Plugin registry — everything extendable.
//
// Every capability in Autonomyx is a plugin.
// Plugins register nodes into the governance graph,
// data sources into dashboards, compute providers into the engine,
// storage backends into the artifact store, and search handlers into the index.
//
// Built-in plugins ship with the platform.
// Custom plugins register at startup via AUTONOMYX_PLUGINS env var or API.
// No recompile required. No restart required for new data sources.
//
// Plugin lifecycle:
//   1. Plugin::register() — declare capabilities, nodes, sources
//   2. PluginRegistry::load() — wires everything into AppState
//   3. Plugin capabilities appear in governance graph immediately
//   4. Plugin data sources appear in dashboard widget picker immediately
//   5. Plugin compute providers appear in compute engine immediately
//
// "Everything extendable" — the loop is never closed to new participants.
// New agents, new tools, new data sources, new chains — all register as plugins.
//
// openautonomyx.com

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::RwLock;

// ── Plugin descriptor ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub id:           String,
    pub name:         String,
    pub version:      String,
    pub description:  String,
    pub author:       String,
    pub kind:         PluginKind,
    pub capabilities: Vec<String>,
    pub config_keys:  Vec<String>,   // env vars this plugin reads
    pub enabled:      bool,
    pub loaded:       bool,
    pub homepage:     Option<String>,
    pub nodes:        Vec<PluginNode>,     // governance graph nodes this plugin contributes
    pub data_sources: Vec<String>,          // dashboard DataSource IDs this plugin provides
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    ComputeProvider,   // adds a new LLM/inference backend
    StorageBackend,    // adds a new artifact storage backend
    DataPipeline,      // adds a new data source (Kafka, SeaTunnel, Spark, etc.)
    Connector,         // adds an external API connector (Slack, GitHub, Jira, etc.)
    ChainAdapter,      // adds a blockchain network (non-EVM chains, etc.)
    GovernanceRule,    // adds governance policy rules
    DashboardSource,   // adds a new dashboard data source
    Tool,              // adds a new tool into the governance graph
    Observer,          // adds an observability backend (OTel, Prometheus, Grafana)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginNode {
    pub id:           String,
    pub label:        String,
    pub kind:         String,    // govgraph NodeKind
    pub capabilities: Vec<String>,
    pub edges_to:     Vec<String>,  // node IDs this plugin connects to
    pub capability_required: String, // capability needed to reach this node
}

// ── Built-in plugin descriptors ───────────────────────────────────────────────

pub fn builtin_plugins() -> Vec<PluginDescriptor> {
    vec![
        PluginDescriptor {
            id:           "plugin_anthropic".into(),
            name:         "Anthropic Claude".into(),
            version:      "4.8".into(),
            description:  "Claude Opus, Sonnet, Haiku — adaptive thinking, streaming".into(),
            author:       "Anthropic".into(),
            kind:         PluginKind::ComputeProvider,
            capabilities: vec!["inference:claude".into(), "thinking:adaptive".into(), "streaming:sse".into()],
            config_keys:  vec!["ANTHROPIC_API_KEY".into()],
            enabled:      std::env::var("ANTHROPIC_API_KEY").is_ok(),
            loaded:       std::env::var("ANTHROPIC_API_KEY").is_ok(),
            homepage:     Some("https://anthropic.com".into()),
            nodes: vec![PluginNode {
                id:           "compute:anthropic".into(),
                label:        "Claude Inference".into(),
                kind:         "tool".into(),
                capabilities: vec!["inference:claude".into()],
                edges_to:     vec!["agent:plan".into(), "agent:build".into(), "agent:review".into()],
                capability_required: "inference:claude".into(),
            }],
            data_sources: vec![],
        },
        PluginDescriptor {
            id:           "plugin_ollama".into(),
            name:         "Ollama".into(),
            version:      "0.6".into(),
            description:  "Local inference — llama3, mistral, phi3, qwen2. $0 token cost.".into(),
            author:       "Ollama".into(),
            kind:         PluginKind::ComputeProvider,
            capabilities: vec!["inference:local".into(), "inference:ollama".into(), "offline:true".into()],
            config_keys:  vec!["OLLAMA_HOST".into()],
            enabled:      true,
            loaded:       true,
            homepage:     Some("https://ollama.com".into()),
            nodes: vec![PluginNode {
                id:           "compute:ollama".into(),
                label:        "Ollama Local".into(),
                kind:         "tool".into(),
                capabilities: vec!["inference:local".into()],
                edges_to:     vec!["agent:build".into(), "agent:review".into()],
                capability_required: "inference:local".into(),
            }],
            data_sources: vec![],
        },
        PluginDescriptor {
            id:           "plugin_surrealdb".into(),
            name:         "SurrealDB".into(),
            version:      "2.0".into(),
            description:  "Distributed config store — live queries, real-time sync".into(),
            author:       "SurrealDB".into(),
            kind:         PluginKind::StorageBackend,
            capabilities: vec!["storage:surreal".into(), "live_query:true".into(), "distributed:true".into()],
            config_keys:  vec!["SURREAL_URL".into(), "SURREAL_USER".into(), "SURREAL_PASS".into()],
            enabled:      std::env::var("SURREAL_URL").is_ok(),
            loaded:       std::env::var("SURREAL_URL").is_ok(),
            homepage:     Some("https://surrealdb.com".into()),
            nodes: vec![],
            data_sources: vec!["surreal_live".into()],
        },
        PluginDescriptor {
            id:           "plugin_seatunnel".into(),
            name:         "Apache SeaTunnel".into(),
            version:      "2.3".into(),
            description:  "Distributed data integration — ETL, streaming, 100+ connectors".into(),
            author:       "Apache Software Foundation".into(),
            kind:         PluginKind::DataPipeline,
            capabilities: vec!["pipeline:etl".into(), "pipeline:stream".into(), "connector:kafka".into(),
                               "connector:jdbc".into(), "connector:s3".into(), "connector:http".into()],
            config_keys:  vec!["SEATUNNEL_API_URL".into()],
            enabled:      std::env::var("SEATUNNEL_API_URL").is_ok(),
            loaded:       false,
            homepage:     Some("https://seatunnel.apache.org".into()),
            nodes: vec![PluginNode {
                id:           "source:seatunnel".into(),
                label:        "SeaTunnel Pipeline".into(),
                kind:         "source".into(),
                capabilities: vec!["pipeline:etl".into(), "pipeline:stream".into()],
                edges_to:     vec!["agent:observe".into(), "sink:storage".into()],
                capability_required: "pipeline:stream".into(),
            }],
            data_sources: vec!["seatunnel_jobs".into(), "seatunnel_metrics".into()],
        },
        PluginDescriptor {
            id:           "plugin_kafka".into(),
            name:         "Apache Kafka".into(),
            version:      "3.7".into(),
            description:  "Distributed event streaming — fabric events as Kafka topics".into(),
            author:       "Apache Software Foundation".into(),
            kind:         PluginKind::DataPipeline,
            capabilities: vec!["stream:kafka".into(), "event:publish".into(), "event:subscribe".into()],
            config_keys:  vec!["KAFKA_BOOTSTRAP_SERVERS".into()],
            enabled:      std::env::var("KAFKA_BOOTSTRAP_SERVERS").is_ok(),
            loaded:       false,
            homepage:     Some("https://kafka.apache.org".into()),
            nodes: vec![PluginNode {
                id:           "sink:kafka".into(),
                label:        "Kafka Topic".into(),
                kind:         "sink".into(),
                capabilities: vec!["stream:kafka".into()],
                edges_to:     vec![],
                capability_required: "stream:kafka".into(),
            }],
            data_sources: vec!["kafka_topics".into()],
        },
        PluginDescriptor {
            id:           "plugin_spark".into(),
            name:         "Apache Spark".into(),
            version:      "3.5".into(),
            description:  "Large-scale data processing — batch analytics on agent outputs".into(),
            author:       "Apache Software Foundation".into(),
            kind:         PluginKind::DataPipeline,
            capabilities: vec!["compute:spark".into(), "analytics:batch".into(), "ml:training".into()],
            config_keys:  vec!["SPARK_MASTER_URL".into()],
            enabled:      std::env::var("SPARK_MASTER_URL").is_ok(),
            loaded:       false,
            homepage:     Some("https://spark.apache.org".into()),
            nodes: vec![PluginNode {
                id:           "compute:spark".into(),
                label:        "Spark Cluster".into(),
                kind:         "tool".into(),
                capabilities: vec!["compute:spark".into(), "analytics:batch".into()],
                edges_to:     vec!["sink:storage".into(), "agent:observe".into()],
                capability_required: "compute:spark".into(),
            }],
            data_sources: vec!["spark_jobs".into()],
        },
        PluginDescriptor {
            id:           "plugin_flink".into(),
            name:         "Apache Flink".into(),
            version:      "1.20".into(),
            description:  "Stateful stream processing — real-time agent output analytics".into(),
            author:       "Apache Software Foundation".into(),
            kind:         PluginKind::DataPipeline,
            capabilities: vec!["compute:flink".into(), "analytics:stream".into(), "stateful:true".into()],
            config_keys:  vec!["FLINK_JOBMANAGER_URL".into()],
            enabled:      std::env::var("FLINK_JOBMANAGER_URL").is_ok(),
            loaded:       false,
            homepage:     Some("https://flink.apache.org".into()),
            nodes: vec![],
            data_sources: vec!["flink_jobs".into()],
        },
        PluginDescriptor {
            id:           "plugin_github".into(),
            name:         "GitHub".into(),
            version:      "api-v4".into(),
            description:  "Source control, CI/CD, issues, PRs — project lifecycle integration".into(),
            author:       "GitHub".into(),
            kind:         PluginKind::Connector,
            capabilities: vec!["code:read".into(), "code:write".into(), "pr:create".into(),
                               "issue:create".into(), "ci:trigger".into()],
            config_keys:  vec!["GITHUB_TOKEN".into()],
            enabled:      std::env::var("GITHUB_TOKEN").is_ok(),
            loaded:       std::env::var("GITHUB_TOKEN").is_ok(),
            homepage:     Some("https://github.com".into()),
            nodes: vec![PluginNode {
                id:           "connector:github".into(),
                label:        "GitHub".into(),
                kind:         "api".into(),
                capabilities: vec!["code:read".into(), "pr:create".into()],
                edges_to:     vec!["agent:review".into(), "agent:build".into()],
                capability_required: "code:read".into(),
            }],
            data_sources: vec!["github_prs".into(), "github_issues".into()],
        },
        PluginDescriptor {
            id:           "plugin_opensea".into(),
            name:         "OpenSea".into(),
            version:      "api-v2".into(),
            description:  "NFT marketplace — list agent NFTs, transfer ownership, trade".into(),
            author:       "OpenSea".into(),
            kind:         PluginKind::Connector,
            capabilities: vec!["nft:list".into(), "nft:transfer".into(), "nft:price".into(), "agent:marketplace".into()],
            config_keys:  vec!["OPENSEA_API_KEY".into()],
            enabled:      std::env::var("OPENSEA_API_KEY").is_ok(),
            loaded:       false,
            homepage:     Some("https://opensea.io".into()),
            nodes: vec![PluginNode {
                id:           "sink:opensea".into(),
                label:        "OpenSea Marketplace".into(),
                kind:         "sink".into(),
                capabilities: vec!["nft:list".into()],
                edges_to:     vec![],
                capability_required: "agent:marketplace".into(),
            }],
            data_sources: vec!["opensea_listings".into()],
        },
    ]
}

// ── Plugin registry ───────────────────────────────────────────────────────────

pub struct PluginRegistry {
    plugins:  RwLock<HashMap<String, PluginDescriptor>>,
    handlers: RwLock<HashMap<String, Value>>,   // plugin_id → runtime config
}

impl PluginRegistry {
    pub fn new() -> Self {
        let reg = PluginRegistry {
            plugins:  RwLock::new(HashMap::new()),
            handlers: RwLock::new(HashMap::new()),
        };
        // Load built-in plugins
        for p in builtin_plugins() {
            if p.enabled {
                tracing::info!(plugin = %p.id, name = %p.name, "plugin: loaded");
            }
            reg.plugins.write().unwrap().insert(p.id.clone(), p);
        }
        reg
    }

    pub fn register(&self, plugin: PluginDescriptor) -> PluginDescriptor {
        tracing::info!(plugin = %plugin.id, name = %plugin.name, "plugin: registered");
        let mut plugins = self.plugins.write().unwrap();
        plugins.insert(plugin.id.clone(), plugin.clone());
        plugin
    }

    pub fn get(&self, id: &str) -> Option<PluginDescriptor> {
        self.plugins.read().unwrap().get(id).cloned()
    }

    pub fn list(&self) -> Vec<PluginDescriptor> {
        self.plugins.read().unwrap().values().cloned().collect()
    }

    pub fn list_by_kind(&self, kind: &PluginKind) -> Vec<PluginDescriptor> {
        self.plugins.read().unwrap().values()
            .filter(|p| &p.kind == kind)
            .cloned().collect()
    }

    pub fn enabled(&self) -> Vec<PluginDescriptor> {
        self.plugins.read().unwrap().values()
            .filter(|p| p.enabled)
            .cloned().collect()
    }

    pub fn all_capabilities(&self) -> Vec<String> {
        let mut caps: std::collections::HashSet<String> = std::collections::HashSet::new();
        for p in self.plugins.read().unwrap().values() {
            for c in &p.capabilities { caps.insert(c.clone()); }
        }
        let mut v: Vec<String> = caps.into_iter().collect();
        v.sort();
        v
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> bool {
        let mut plugins = self.plugins.write().unwrap();
        match plugins.get_mut(id) {
            Some(p) => { p.enabled = enabled; true }
            None    => false,
        }
    }

    pub fn all_nodes(&self) -> Vec<PluginNode> {
        self.plugins.read().unwrap().values()
            .filter(|p| p.enabled)
            .flat_map(|p| p.nodes.clone())
            .collect()
    }

    pub fn summary(&self) -> Value {
        let plugins = self.plugins.read().unwrap();
        let enabled = plugins.values().filter(|p| p.enabled).count();
        json!({
            "total":    plugins.len(),
            "enabled":  enabled,
            "disabled": plugins.len() - enabled,
            "kinds": {
                "compute":    plugins.values().filter(|p| p.kind == PluginKind::ComputeProvider).count(),
                "storage":    plugins.values().filter(|p| p.kind == PluginKind::StorageBackend).count(),
                "pipeline":   plugins.values().filter(|p| p.kind == PluginKind::DataPipeline).count(),
                "connector":  plugins.values().filter(|p| p.kind == PluginKind::Connector).count(),
                "chain":      plugins.values().filter(|p| p.kind == PluginKind::ChainAdapter).count(),
                "tool":       plugins.values().filter(|p| p.kind == PluginKind::Tool).count(),
                "observer":   plugins.values().filter(|p| p.kind == PluginKind::Observer).count(),
            },
            "capabilities": self.all_capabilities().len(),
        })
    }
}
