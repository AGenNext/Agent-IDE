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
            capabilities: vec![
                "lifecycle_gates", "fabric_events", "federation_did", "usage_metering",
                "bom_provenance", "aip_protocol", "governance_policy", "agent_execution",
            ],
            ecosystems: vec![
                "llm", "cloud", "identity", "data", "observability",
                "gitops", "supply_chain", "mesh", "ide",
            ],
        }
    }
}
