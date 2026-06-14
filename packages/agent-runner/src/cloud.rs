// Cloud provider detection — Autonomyx runs on any cloud.
// Cloud is the platform provider (compute substrate). Autonomyx is the platform layer.
// No lock-in. The binary is identical on all clouds. The DID is portable.
// Cloud provides nodes. Autonomyx provides agency.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CloudProvider {
    Aws,
    Gcp,
    Azure,
    Hetzner,
    K3s,
    BareMetalK8s,
    Local,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudContext {
    pub provider:       CloudProvider,
    pub region:         Option<String>,
    pub zone:           Option<String>,
    pub instance_type:  Option<String>,
    pub node_name:      Option<String>,
    pub cluster:        Option<String>,
    pub namespace:      Option<String>,
}

impl CloudContext {
    /// Detect cloud provider from environment variables injected by k8s downward API
    /// or cloud-specific metadata. No network calls — reads env only.
    pub fn detect() -> Self {
        let node_name  = std::env::var("NODE_NAME").ok();
        let namespace  = std::env::var("POD_NAMESPACE")
                            .or_else(|_| std::env::var("AGENT_JOB_NAMESPACE")).ok();

        // Cloud provider hints from env (injected by cloud-specific node labels or operators)
        let provider_hint = std::env::var("CLOUD_PROVIDER").ok();

        let provider = match provider_hint.as_deref() {
            Some("aws")     => CloudProvider::Aws,
            Some("gcp")     => CloudProvider::Gcp,
            Some("azure")   => CloudProvider::Azure,
            Some("hetzner") => CloudProvider::Hetzner,
            Some("k3s")     => CloudProvider::K3s,
            Some("metal")   => CloudProvider::BareMetalK8s,
            _ => detect_from_env(),
        };

        let region = std::env::var("CLOUD_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .or_else(|_| std::env::var("CLOUDSDK_COMPUTE_REGION"))
            .ok();

        let zone = std::env::var("CLOUD_ZONE")
            .or_else(|_| std::env::var("CLOUDSDK_COMPUTE_ZONE"))
            .ok();

        let instance_type = std::env::var("CLOUD_INSTANCE_TYPE")
            .or_else(|_| std::env::var("NODE_INSTANCE_TYPE"))
            .ok();

        let cluster = std::env::var("CLUSTER_NAME").ok();

        CloudContext { provider, region, zone, instance_type, node_name, cluster, namespace }
    }
}

fn detect_from_env() -> CloudProvider {
    // AWS: EKS sets AWS_DEFAULT_REGION or has aws-specific node labels propagated
    if std::env::var("AWS_DEFAULT_REGION").is_ok() || std::env::var("EKS_CLUSTER_NAME").is_ok() {
        return CloudProvider::Aws;
    }
    // GCP: GKE sets GOOGLE_CLOUD_PROJECT or similar
    if std::env::var("GOOGLE_CLOUD_PROJECT").is_ok() || std::env::var("GKE_CLUSTER").is_ok() {
        return CloudProvider::Gcp;
    }
    // Azure: AKS sets AZURE_SUBSCRIPTION_ID or NODE_RESOURCE_GROUP
    if std::env::var("AZURE_SUBSCRIPTION_ID").is_ok() || std::env::var("AKS_CLUSTER").is_ok() {
        return CloudProvider::Azure;
    }
    // Hetzner: HCloud-specific labels
    if std::env::var("HCLOUD_CLUSTER").is_ok() || std::env::var("HETZNER_DATACENTER").is_ok() {
        return CloudProvider::Hetzner;
    }
    // k3s: K3S_URL is set when joining a cluster
    if std::env::var("K3S_URL").is_ok() || std::env::var("K3S_TOKEN").is_ok() {
        return CloudProvider::K3s;
    }
    // In-cluster but unknown cloud → bare metal k8s
    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        return CloudProvider::BareMetalK8s;
    }
    // Not in any cluster
    if std::env::var("PORT").is_ok() || std::env::var("HOME").is_ok() {
        return CloudProvider::Local;
    }
    CloudProvider::Unknown
}

// ── Device Detection ─────────────────────────────────────────────────────────
// "Build for all devices" — the same binary runs on server, edge, desktop,
// mobile (via API), embedded. Device context shapes defaults and resource limits.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceClass {
    Server,       // full resources: unlimited agents, high throughput
    Edge,         // constrained: small memory footprint, local inference preferred
    Desktop,      // interactive: MCP + IDE extensions primary interface
    Mobile,       // API-first: no direct binary, reach via HTTPS or WebSocket
    Embedded,     // tiny: single-agent, no fabric, local model only
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceContext {
    pub class:       DeviceClass,
    pub arch:        &'static str,   // x86_64, aarch64, riscv64, wasm32
    pub os:          &'static str,   // linux, macos, windows, none (WASM)
    pub cores:       Option<usize>,
    pub ram_hint:    Option<&'static str>,
    pub capabilities: Vec<&'static str>,
}

impl DeviceContext {
    pub fn detect() -> Self {
        let class = detect_device_class();
        let (caps, ram_hint) = device_capabilities(&class);

        DeviceContext {
            class,
            arch: std::env::consts::ARCH,
            os:   std::env::consts::OS,
            cores: std::thread::available_parallelism().ok().map(|n| n.get()),
            ram_hint,
            capabilities: caps,
        }
    }
}

fn detect_device_class() -> DeviceClass {
    // Explicit override
    if let Ok(cls) = std::env::var("DEVICE_CLASS") {
        return match cls.as_str() {
            "server"   => DeviceClass::Server,
            "edge"     => DeviceClass::Edge,
            "desktop"  => DeviceClass::Desktop,
            "mobile"   => DeviceClass::Mobile,
            "embedded" => DeviceClass::Embedded,
            _          => DeviceClass::Unknown,
        };
    }
    // Heuristics: in-cluster = server; low CPU = edge; interactive = desktop
    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        return DeviceClass::Server;
    }
    let cores = std::thread::available_parallelism().ok().map(|n| n.get()).unwrap_or(1);
    if cores >= 8 {
        // Large machine with display environment = desktop
        if std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::consts::OS == "macos" || std::env::consts::OS == "windows" {
            return DeviceClass::Desktop;
        }
        return DeviceClass::Server;
    }
    if cores <= 2 {
        return DeviceClass::Edge;
    }
    DeviceClass::Unknown
}

fn device_capabilities(class: &DeviceClass) -> (Vec<&'static str>, Option<&'static str>) {
    match class {
        DeviceClass::Server => (
            vec!["all_agents", "full_fabric", "surreal_db", "mcp_server",
                 "ws_stream", "otel", "federation", "high_throughput"],
            Some(">= 2GB"),
        ),
        DeviceClass::Edge => (
            vec!["single_agent", "local_inference", "lite_fabric", "ws_stream"],
            Some("256MB – 1GB"),
        ),
        DeviceClass::Desktop => (
            vec!["ide_extension", "mcp_server", "full_fabric", "ws_stream", "local_inference"],
            Some(">= 8GB"),
        ),
        DeviceClass::Mobile => (
            vec!["api_client", "ws_consumer"],
            Some("< 256MB"),
        ),
        DeviceClass::Embedded => (
            vec!["single_agent", "local_model_only", "no_fabric"],
            Some("< 64MB"),
        ),
        _ => (vec!["unknown"], None),
    }
}

/// The platform identity — who we are, where we run, what we serve.
/// "platform makes things real" — this is the runtime self-description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformIdentity {
    pub name:           &'static str,
    pub version:        &'static str,
    pub did_method:     &'static str,
    pub protocol:       &'static str,
    pub philosophy:     &'static str,
    pub homepage:       &'static str,
    pub cloud:          CloudContext,
    pub device:         DeviceContext,
    pub capabilities:   Vec<&'static str>,
    pub ecosystems:     Vec<&'static str>,
}

impl PlatformIdentity {
    pub fn new() -> Self {
        PlatformIdentity {
            name:       "Autonomyx",
            version:    env!("CARGO_PKG_VERSION"),
            did_method: "did:autonomyx",
            protocol:   "AIP/1.0",
            philosophy: "multi-ecosystem single world model — everyone and everything is an agent",
            homepage:   "https://openautonomyx.com",
            cloud:      CloudContext::detect(),
            device:     DeviceContext::detect(),
            capabilities: vec![
                "lifecycle_gates", "fabric_events", "federation_did", "usage_metering",
                "bom_provenance", "aip_protocol", "governance_policy", "agent_execution",
                "agent_manifest", "feasibility_check", "multi_device",
            ],
            ecosystems: vec![
                "llm", "cloud", "identity", "data", "observability",
                "gitops", "supply_chain", "mesh", "ide",
                "mobile", "edge", "embedded", "desktop",
            ],
        }
    }
}
