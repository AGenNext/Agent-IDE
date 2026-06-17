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
        PluginDescriptor {
            id:           "plugin_buildr".into(),
            name:         "Apache Buildr".into(),
            version:      "1.5".into(),
            description:  "JVM build automation — Java, Scala, Groovy, Kotlin; Maven-compatible artifacts".into(),
            author:       "Apache Software Foundation".into(),
            kind:         PluginKind::Tool,
            capabilities: vec![
                "build:jvm".into(), "build:java".into(), "build:scala".into(),
                "build:kotlin".into(), "test:jvm".into(), "artifact:publish".into(),
                "dependency:resolve".into(), "compile:jvm".into(),
            ],
            config_keys:  vec!["BUILDR_HOME".into(), "BUILDR_RAKE_URL".into(), "MAVEN_REPO".into()],
            enabled:      std::env::var("BUILDR_HOME").is_ok()
                          || std::env::var("BUILDR_RAKE_URL").is_ok(),
            loaded:       false,
            homepage:     Some("https://buildr.apache.org".into()),
            nodes: vec![
                PluginNode {
                    id:           "tool:buildr:compile".into(),
                    label:        "Buildr Compile".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["compile:jvm".into(), "build:java".into()],
                    edges_to:     vec!["agent:build".into(), "agent:review".into()],
                    capability_required: "build:jvm".into(),
                },
                PluginNode {
                    id:           "tool:buildr:test".into(),
                    label:        "Buildr Test".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["test:jvm".into()],
                    edges_to:     vec!["agent:review".into(), "agent:observe".into()],
                    capability_required: "test:jvm".into(),
                },
                PluginNode {
                    id:           "sink:buildr:publish".into(),
                    label:        "Buildr Publish".into(),
                    kind:         "sink".into(),
                    capabilities: vec!["artifact:publish".into()],
                    edges_to:     vec!["sink:storage".into(), "agent:deploy".into()],
                    capability_required: "artifact:publish".into(),
                },
            ],
            data_sources: vec!["buildr_tasks".into(), "buildr_artifacts".into()],
        },
        PluginDescriptor {
            id:           "plugin_brooklyn".into(),
            name:         "Apache Brooklyn".into(),
            version:      "1.1".into(),
            description:  "Application management and deployment blueprints — model, deploy, manage any app on any cloud".into(),
            author:       "Apache Software Foundation".into(),
            kind:         PluginKind::Tool,
            capabilities: vec![
                "deploy:blueprint".into(), "manage:application".into(),
                "cloud:multi".into(), "topology:manage".into(),
                "policy:autoscale".into(), "heal:self".into(),
            ],
            config_keys:  vec!["BROOKLYN_URL".into(), "BROOKLYN_USER".into(), "BROOKLYN_PASS".into()],
            enabled:      std::env::var("BROOKLYN_URL").is_ok(),
            loaded:       false,
            homepage:     Some("https://brooklyn.apache.org".into()),
            nodes: vec![
                PluginNode {
                    id:           "tool:brooklyn:deploy".into(),
                    label:        "Brooklyn Deploy".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["deploy:blueprint".into(), "manage:application".into()],
                    edges_to:     vec!["agent:deploy".into(), "agent:observe".into()],
                    capability_required: "deploy:blueprint".into(),
                },
                PluginNode {
                    id:           "tool:brooklyn:heal".into(),
                    label:        "Brooklyn Self-Heal".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["heal:self".into(), "policy:autoscale".into()],
                    edges_to:     vec!["agent:observe".into(), "sink:stream".into()],
                    capability_required: "heal:self".into(),
                },
            ],
            data_sources: vec!["brooklyn_apps".into(), "brooklyn_sensors".into()],
        },
        PluginDescriptor {
            id:           "plugin_buildstream".into(),
            name:         "BuildStream".into(),
            version:      "2.3".into(),
            description:  "Artifact-based build system — reproducible builds, cache sharing, pipeline composition".into(),
            author:       "BuildStream Contributors".into(),
            kind:         PluginKind::Tool,
            capabilities: vec![
                "build:reproducible".into(), "build:artifact".into(),
                "cache:share".into(), "pipeline:compose".into(),
                "build:hermetic".into(), "sandbox:fuse".into(),
            ],
            config_keys:  vec!["BUILDSTREAM_CACHE".into(), "BUILDSTREAM_REMOTE_CACHE".into()],
            enabled:      std::env::var("BUILDSTREAM_CACHE").is_ok(),
            loaded:       false,
            homepage:     Some("https://buildstream.build".into()),
            nodes: vec![
                PluginNode {
                    id:           "tool:buildstream:build".into(),
                    label:        "BuildStream Build".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["build:reproducible".into(), "build:artifact".into()],
                    edges_to:     vec!["agent:build".into(), "agent:review".into(), "sink:storage".into()],
                    capability_required: "build:artifact".into(),
                },
                PluginNode {
                    id:           "tool:buildstream:cache".into(),
                    label:        "BuildStream Cache".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["cache:share".into()],
                    edges_to:     vec!["sink:storage".into()],
                    capability_required: "cache:share".into(),
                },
            ],
            data_sources: vec!["buildstream_pipeline".into(), "buildstream_cache_stats".into()],
        },
        PluginDescriptor {
            id:           "plugin_airavata".into(),
            name:         "Apache Airavata".into(),
            version:      "0.21".into(),
            description:  "Distributed science gateway framework — execute and manage computational jobs and workflows on HPC clusters, supercomputers, national grids, and academic/commercial clouds. Enables large-scale agent workflows.".into(),
            author:       "Apache Software Foundation".into(),
            kind:         PluginKind::DataPipeline,
            capabilities: vec![
                "compute:hpc".into(), "gateway:science".into(), "workflow:distributed".into(),
                "job:submit".into(), "job:monitor".into(), "job:manage".into(),
                "cluster:supercomputer".into(), "grid:national".into(), "cloud:academic".into(),
            ],
            config_keys:  vec![
                "AIRAVATA_API_HOST".into(), "AIRAVATA_API_PORT".into(),
                "AIRAVATA_GATEWAY_ID".into(), "AIRAVATA_AUTH_TOKEN".into(),
            ],
            enabled:      std::env::var("AIRAVATA_API_HOST").is_ok(),
            loaded:       false,
            homepage:     Some("https://airavata.apache.org".into()),
            nodes: vec![
                PluginNode {
                    id:           "tool:airavata:submit".into(),
                    label:        "Airavata Job Submit".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["job:submit".into(), "compute:hpc".into()],
                    edges_to:     vec!["agent:build".into(), "agent:deploy".into(), "agent:observe".into()],
                    capability_required: "job:submit".into(),
                },
                PluginNode {
                    id:           "tool:airavata:monitor".into(),
                    label:        "Airavata Job Monitor".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["job:monitor".into(), "workflow:distributed".into()],
                    edges_to:     vec!["agent:observe".into(), "sink:stream".into()],
                    capability_required: "job:monitor".into(),
                },
                PluginNode {
                    id:           "source:airavata:gateway".into(),
                    label:        "Airavata Science Gateway".into(),
                    kind:         "source".into(),
                    capabilities: vec!["gateway:science".into(), "grid:national".into()],
                    edges_to:     vec!["agent:plan".into(), "agent:observe".into()],
                    capability_required: "gateway:science".into(),
                },
            ],
            data_sources: vec![
                "airavata_experiments".into(), "airavata_jobs".into(),
                "airavata_applications".into(), "airavata_gateways".into(),
            ],
        },
        PluginDescriptor {
            id:           "plugin_llamars".into(),
            name:         "llama-rs (Local LLM)".into(),
            version:      "0.16".into(),
            description:  "Pure Rust local LLM inference — LLaMA-family models (llama2, mistral, phi, etc.) run directly in-process. Zero network, zero cost, full control. pkg:cargo/llama-rs".into(),
            author:       "llama-rs contributors".into(),
            kind:         PluginKind::ComputeProvider,
            capabilities: vec![
                "inference:local".into(), "inference:llama".into(), "inference:rust".into(),
                "offline:true".into(), "model:gguf".into(), "model:llama2".into(),
            ],
            config_keys:  vec!["LLAMA_MODEL_PATH".into(), "LLAMA_THREADS".into()],
            enabled:      std::env::var("LLAMA_MODEL_PATH").is_ok(),
            loaded:       false,
            homepage:     Some("https://crates.io/crates/llama-rs".into()),
            nodes: vec![
                PluginNode {
                    id:           "compute:llamars".into(),
                    label:        "llama-rs Inference".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["inference:local".into(), "inference:llama".into()],
                    edges_to:     vec!["agent:plan".into(), "agent:build".into(), "agent:review".into()],
                    capability_required: "inference:local".into(),
                },
            ],
            data_sources: vec![],
        },
        PluginDescriptor {
            id:           "plugin_hono".into(),
            name:         "Hono".into(),
            version:      "4.x".into(),
            description:  "Ultrafast edge web framework (JS/TS) — runs on Cloudflare Workers, Deno, Bun, Node. Deploy Autonomyx API endpoints to the edge with zero cold-start.".into(),
            author:       "Hono contributors".into(),
            kind:         PluginKind::Tool,
            capabilities: vec![
                "edge:deploy".into(), "api:edge".into(), "runtime:cloudflare".into(),
                "runtime:deno".into(), "runtime:bun".into(), "runtime:node".into(),
                "middleware:pretty_json".into(), "middleware:cors".into(),
            ],
            config_keys:  vec!["HONO_DEPLOY_TARGET".into(), "CF_ACCOUNT_ID".into()],
            enabled:      std::env::var("HONO_DEPLOY_TARGET").is_ok(),
            loaded:       false,
            homepage:     Some("https://hono.dev".into()),
            nodes: vec![
                PluginNode {
                    id:           "tool:hono:edge".into(),
                    label:        "Hono Edge Deploy".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["edge:deploy".into(), "api:edge".into()],
                    edges_to:     vec!["agent:deploy".into(), "sink:stream".into()],
                    capability_required: "edge:deploy".into(),
                },
            ],
            data_sources: vec!["hono_routes".into()],
        },
        PluginDescriptor {
            id:           "plugin_edge_rs".into(),
            name:         "edge-rs".into(),
            version:      "0.4".into(),
            description:  "Rust edge runtime (crates.io/crates/edge) — run Autonomyx agents at the network edge in pure Rust. No WASM overhead. Direct packet processing via DPDK/io_uring.".into(),
            author:       "edge-rs contributors".into(),
            kind:         PluginKind::ComputeProvider,
            capabilities: vec![
                "runtime:edge".into(), "runtime:rust_native".into(),
                "compute:dpdk".into(), "compute:io_uring".into(),
                "agent:edge".into(), "latency:ultra_low".into(),
            ],
            config_keys:  vec!["EDGE_RS_INTERFACE".into(), "EDGE_RS_CPU_CORES".into()],
            enabled:      std::env::var("EDGE_RS_INTERFACE").is_ok(),
            loaded:       false,
            homepage:     Some("https://crates.io/crates/edge".into()),
            nodes: vec![
                PluginNode {
                    id:           "compute:edge_rs".into(),
                    label:        "edge-rs Runtime".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["runtime:edge".into(), "agent:edge".into()],
                    edges_to:     vec!["agent:run".into(), "agent:observe".into()],
                    capability_required: "agent:edge".into(),
                },
            ],
            data_sources: vec![],
        },
        PluginDescriptor {
            id:           "plugin_zig".into(),
            name:         "Zig".into(),
            version:      "0.14".into(),
            description:  "Systems programming language — compile-time safety, no hidden control flow, no garbage collector. Build ultra-lightweight agent runtimes and edge compute modules in Zig. Interops with C/Rust via extern.".into(),
            author:       "Zig Software Foundation".into(),
            kind:         PluginKind::Tool,
            capabilities: vec![
                "build:zig".into(), "compile:systems".into(), "interop:c".into(),
                "interop:rust".into(), "runtime:bare_metal".into(), "runtime:wasm".into(),
                "test:zig".into(), "ffi:c".into(),
            ],
            config_keys:  vec!["ZIG_HOME".into(), "ZIG_CACHE_DIR".into()],
            enabled:      std::env::var("ZIG_HOME").is_ok(),
            loaded:       false,
            homepage:     Some("https://ziglang.org".into()),
            nodes: vec![
                PluginNode {
                    id:           "tool:zig:build".into(),
                    label:        "Zig Build".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["build:zig".into(), "compile:systems".into()],
                    edges_to:     vec!["agent:build".into(), "agent:review".into(), "sink:storage".into()],
                    capability_required: "build:zig".into(),
                },
                PluginNode {
                    id:           "tool:zig:wasm".into(),
                    label:        "Zig→WASM".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["runtime:wasm".into()],
                    edges_to:     vec!["tool:hono:edge".into(), "compute:edge_rs".into()],
                    capability_required: "runtime:wasm".into(),
                },
            ],
            data_sources: vec!["zig_build_steps".into()],
        },
        PluginDescriptor {
            id:           "plugin_turso".into(),
            name:         "Turso".into(),
            version:      "libsql-0.4".into(),
            description:  "Edge SQLite database — libSQL fork with HTTP API, embedded replicas, per-tenant DBs at the edge. Pair with Hono/edge agents for zero-latency reads. docs.turso.tech/sdk/http/reference".into(),
            author:       "Turso".into(),
            kind:         PluginKind::StorageBackend,
            capabilities: vec![
                "storage:sqlite".into(), "storage:edge".into(), "db:per_tenant".into(),
                "db:embedded_replica".into(), "db:libsql".into(), "api:http".into(),
            ],
            config_keys:  vec!["TURSO_URL".into(), "TURSO_AUTH_TOKEN".into()],
            enabled:      std::env::var("TURSO_URL").is_ok(),
            loaded:       std::env::var("TURSO_URL").is_ok(),
            homepage:     Some("https://turso.tech".into()),
            nodes: vec![
                PluginNode {
                    id:           "sink:turso".into(),
                    label:        "Turso Edge DB".into(),
                    kind:         "sink".into(),
                    capabilities: vec!["storage:edge".into(), "db:per_tenant".into()],
                    edges_to:     vec!["agent:observe".into(), "sink:storage".into()],
                    capability_required: "storage:edge".into(),
                },
            ],
            data_sources: vec!["turso_databases".into(), "turso_usage".into()],
        },
        // ── Distribution + P2P ───────────────────────────────────────────────
        PluginDescriptor {
            id:           "plugin_flyio".into(),
            name:         "Fly.io".into(),
            version:      "2.x".into(),
            description:  "Global agent distribution — build at desk, distribute through cloud. Deploy Autonomyx replicas to 35+ regions in one command. Built-in Anycast routing, low-latency P2P via WireGuard mesh. Global distribution with sub-50ms latency.".into(),
            author:       "Fly.io".into(),
            kind:         PluginKind::Tool,
            capabilities: vec![
                "deploy:global".into(), "network:anycast".into(), "network:wireguard".into(),
                "peer:allowed".into(), "latency:low".into(), "region:multi".into(),
                "distribute:edge".into(), "build:local_deploy_cloud".into(),
            ],
            config_keys:  vec!["FLY_API_TOKEN".into(), "FLY_APP_NAME".into()],
            enabled:      std::env::var("FLY_API_TOKEN").is_ok(),
            loaded:       false,
            homepage:     Some("https://fly.io".into()),
            nodes: vec![
                PluginNode {
                    id:           "tool:fly:deploy".into(),
                    label:        "Fly Deploy".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["deploy:global".into(), "distribute:edge".into()],
                    edges_to:     vec!["agent:deploy".into(), "agent:observe".into()],
                    capability_required: "deploy:global".into(),
                },
                PluginNode {
                    id:           "tool:fly:mesh".into(),
                    label:        "Fly WireGuard Mesh".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["network:wireguard".into(), "peer:allowed".into()],
                    edges_to:     vec!["sink:stream".into(), "agent:observe".into()],
                    capability_required: "peer:allowed".into(),
                },
            ],
            data_sources: vec!["fly_machines".into(), "fly_metrics".into()],
        },
        PluginDescriptor {
            id:           "plugin_libp2p".into(),
            name:         "libp2p".into(),
            version:      "0.54".into(),
            description:  "Peer-to-peer networking — direct agent-to-agent connections without servers. Kademlia DHT for peer discovery, QUIC transport, circuit relay, hole punching. Peer-to-peer allowed by design.".into(),
            author:       "libp2p contributors".into(),
            kind:         PluginKind::Connector,
            capabilities: vec![
                "peer:p2p".into(), "peer:direct".into(), "transport:quic".into(),
                "discovery:dht".into(), "relay:circuit".into(), "nat:hole_punch".into(),
                "peer:allowed".into(),
            ],
            config_keys:  vec!["LIBP2P_LISTEN_ADDR".into(), "LIBP2P_BOOTSTRAP_PEERS".into()],
            enabled:      std::env::var("LIBP2P_LISTEN_ADDR").is_ok(),
            loaded:       false,
            homepage:     Some("https://libp2p.io".into()),
            nodes: vec![
                PluginNode {
                    id:           "connector:libp2p".into(),
                    label:        "libp2p P2P".into(),
                    kind:         "api".into(),
                    capabilities: vec!["peer:p2p".into(), "peer:direct".into()],
                    edges_to:     vec!["agent:run".into(), "sink:stream".into()],
                    capability_required: "peer:p2p".into(),
                },
            ],
            data_sources: vec!["libp2p_peers".into()],
        },
        // ── Decision intelligence ─────────────────────────────────────────────
        PluginDescriptor {
            id:           "plugin_decision_engine".into(),
            name:         "Decision Intelligence Engine".into(),
            version:      "1.0".into(),
            description:  "Built-in decision intelligence — rule trees, equation agents, confidence scoring, multi-criteria decision analysis (MCDA). Runs inside the platform at zero cost. No LLM required for decisions.".into(),
            author:       "Autonomyx".into(),
            kind:         PluginKind::ComputeProvider,
            capabilities: vec![
                "decision:rule_tree".into(), "decision:equation".into(),
                "decision:mcda".into(), "decision:confidence".into(),
                "agent:equation".into(), "agent:rule".into(),
                "intelligence:deterministic".into(),
            ],
            config_keys:  vec![],   // built-in, no config required
            enabled:      true,     // always enabled — built into agent runtime
            loaded:       true,
            homepage:     Some("https://openautonomyx.com".into()),
            nodes: vec![
                PluginNode {
                    id:           "compute:decision_engine".into(),
                    label:        "Decision Intelligence".into(),
                    kind:         "tool".into(),
                    capabilities: vec!["decision:rule_tree".into(), "decision:equation".into(), "intelligence:deterministic".into()],
                    edges_to:     vec!["agent:plan".into(), "agent:run".into(), "agent:observe".into()],
                    capability_required: "decision:equation".into(),
                },
            ],
            data_sources: vec!["decision_log".into()],
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
