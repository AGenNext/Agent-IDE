// CNCF Alignment — Autonomyx Platform ↔ Cloud Native Computing Foundation Landscape
//
// Every platform capability is mapped to its CNCF equivalent.
// Alignment levels: Native (uses CNCF project directly), Compatible (output is compatible),
// Partial (some overlap), Planned (on the roadmap), Alternative (own approach, same goals).
//
// Gaps are explicit — no hidden drift. This module is the source of truth for
// enterprise teams evaluating CNCF conformance and certification paths.
//
// CNCF landscape: https://landscape.cncf.io
// openautonomyx.com

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentLevel {
    Native,       // Autonomyx uses the CNCF project directly
    Compatible,   // Autonomyx output is compatible with the CNCF project
    Partial,      // Some features overlap; partial integration
    Planned,      // Roadmap item — not yet implemented
    Alternative,  // Autonomyx has its own approach with the same goals
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CncfAlignment {
    pub category:             String,
    pub cncf_project:         String,
    pub cncf_status:          String,   // "graduated" | "incubating" | "sandbox" | "n/a"
    pub autonomyx_component:  String,
    pub alignment:            AlignmentLevel,
    pub notes:                String,
}

pub fn alignment_map() -> Vec<CncfAlignment> {
    vec![
        // ── Runtime ──────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Runtime".into(),
            cncf_project:        "containerd".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "OCI images via Stacker SI hermetic builder".into(),
            alignment:           AlignmentLevel::Compatible,
            notes:               "Stacker SI produces spec-compliant OCI images; containerd is the recommended k8s runtime".into(),
        },
        // ── Orchestration ────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Orchestration".into(),
            cncf_project:        "Kubernetes".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "ComputeKube — governed k8s job submission from GovernanceGraph nodes".into(),
            alignment:           AlignmentLevel::Native,
            notes:               "ComputeKube uses the k8s API directly; governance graph drives all scheduling decisions; CRDs planned for AutonomyxAgent + AutonomyxApplication".into(),
        },
        // ── App Definition ───────────────────────────────────────────────────
        CncfAlignment {
            category:            "App Definition & Image Build".into(),
            cncf_project:        "Helm".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: ".ayx declarations — versioned, governed app manifests with built-in DID".into(),
            alignment:           AlignmentLevel::Alternative,
            notes:               ".ayx is the native format; Helm chart export is a planned target for enterprise deployments".into(),
        },
        // ── CI/CD ────────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Continuous Delivery".into(),
            cncf_project:        "Argo CD".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Lifecycle Sync gate + fabric handler wires ArgoCD app refresh".into(),
            alignment:           AlignmentLevel::Compatible,
            notes:               "Fabric Sync gate is the integration point; ArgoCD can be wired via custom FabricHandler".into(),
        },
        CncfAlignment {
            category:            "CI/CD".into(),
            cncf_project:        "Tekton".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Build gate + Stacker SI pipeline".into(),
            alignment:           AlignmentLevel::Partial,
            notes:               "Build gate maps to Tekton PipelineRun concepts; direct Tekton CRD emission is planned".into(),
        },
        CncfAlignment {
            category:            "GitOps".into(),
            cncf_project:        "Flux".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: ".ayx source declarations".into(),
            alignment:           AlignmentLevel::Planned,
            notes:               ".ayx files should be Flux-reconcilable via Flux Source Controller; planned gap".into(),
        },
        // ── Observability ─────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Observability — Tracing".into(),
            cncf_project:        "OpenTelemetry".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Observe gate emits OTel spans; tower TraceLayer wraps all HTTP; fabric event carries trace context".into(),
            alignment:           AlignmentLevel::Native,
            notes:               "OTel is the primary telemetry protocol; every lifecycle stage boundary is a span".into(),
        },
        CncfAlignment {
            category:            "Observability — Metrics".into(),
            cncf_project:        "Prometheus".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Fabric MetricHandler — counter per stage per artifact".into(),
            alignment:           AlignmentLevel::Compatible,
            notes:               "MetricHandler increments Prometheus counters on every fabric event; scrape endpoint planned".into(),
        },
        CncfAlignment {
            category:            "Observability — Tracing UI".into(),
            cncf_project:        "Jaeger".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "OTel SDK → Jaeger OTLP exporter".into(),
            alignment:           AlignmentLevel::Compatible,
            notes:               "OTel SDK configured via OTEL_EXPORTER_OTLP_ENDPOINT; fabric stage = span boundary".into(),
        },
        CncfAlignment {
            category:            "Observability — Service Graph".into(),
            cncf_project:        "Kiali".into(),
            cncf_status:         "incubating".into(),
            autonomyx_component: "Megaverse graph — all entity relationships in one BFS-queryable model".into(),
            alignment:           AlignmentLevel::Partial,
            notes:               "Megaverse captures platform-level topology; Kiali captures mesh-level traffic; correlation planned via trace IDs".into(),
        },
        // ── Security ──────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Security — Identity".into(),
            cncf_project:        "SPIFFE / SPIRE".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "did:autonomyx — Ed25519 DID-based identity; Auth-matic JIT keys".into(),
            alignment:           AlignmentLevel::Alternative,
            notes:               "Platform issues DIDs for agents + apps; SPIRE SVID integration planned for pod-level workload identity in k8s".into(),
        },
        CncfAlignment {
            category:            "Security — Supply Chain".into(),
            cncf_project:        "Cosign (Sigstore)".into(),
            cncf_status:         "incubating".into(),
            autonomyx_component: "Sign gate — cosign signs every image; k8s admission policy rejects unsigned images".into(),
            alignment:           AlignmentLevel::Native,
            notes:               "Cosign is first-class; HSM/TPM-backed signing keys; supply chain proof on every artifact".into(),
        },
        CncfAlignment {
            category:            "Security — Policy".into(),
            cncf_project:        "Open Policy Agent (OPA)".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "GovernanceGraph — compute core wired to governance at every edge; 7-value alignment check".into(),
            alignment:           AlignmentLevel::Alternative,
            notes:               "Governance graph implements policy-as-code natively; Rego export for OPA integration is a planned high-priority gap".into(),
        },
        CncfAlignment {
            category:            "Security — Policy".into(),
            cncf_project:        "Kyverno".into(),
            cncf_status:         "incubating".into(),
            autonomyx_component: "GovernanceGraph node policies".into(),
            alignment:           AlignmentLevel::Planned,
            notes:               "GovernanceGraph node policies should export as Kyverno ClusterPolicy resources; planned".into(),
        },
        CncfAlignment {
            category:            "Security — Runtime Threat Detection".into(),
            cncf_project:        "Falco".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Governance audit trail + fabric dead-letter log".into(),
            alignment:           AlignmentLevel::Partial,
            notes:               "Dead-letter log captures governance violations; Falco integration planned for syscall-level detection alongside fabric events".into(),
        },
        // ── Service Mesh ──────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Service Mesh".into(),
            cncf_project:        "Istio".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "mTLS STRICT PeerAuthentication between all pods; Caddy TLS at ingress".into(),
            alignment:           AlignmentLevel::Native,
            notes:               "Platform REQUIRES Istio mTLS in k8s deployments; PeerAuthentication is a first-class security constraint".into(),
        },
        // ── Messaging ────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Streaming / Messaging".into(),
            cncf_project:        "NATS".into(),
            cncf_status:         "incubating".into(),
            autonomyx_component: "Fabric broadcast channel (tokio broadcast) + multiserver WebSocket bridge".into(),
            alignment:           AlignmentLevel::Alternative,
            notes:               "In-process tokio broadcast for single node; NATS JetStream is the recommended path for persistent multi-cluster pub/sub".into(),
        },
        // ── Storage ───────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Storage — Block/Object".into(),
            cncf_project:        "Rook / Ceph".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "StorageRegistry — policy-driven, milestone-bound artifact storage".into(),
            alignment:           AlignmentLevel::Partial,
            notes:               "StorageRegistry is backend-agnostic; Rook/Ceph recommended for k8s persistent storage".into(),
        },
        CncfAlignment {
            category:            "Database".into(),
            cncf_project:        "n/a (SurrealDB — not in CNCF)".into(),
            cncf_status:         "n/a".into(),
            autonomyx_component: "ConfigDB — SurrealDB with live queries; embedded or remote".into(),
            alignment:           AlignmentLevel::Alternative,
            notes:               "SurrealDB chosen for live query + embedded mode; noted CNCF gap — TiKV or etcd are CNCF-graduated alternatives for config store".into(),
        },
        // ── API Gateway ──────────────────────────────────────────────────────
        CncfAlignment {
            category:            "API Gateway".into(),
            cncf_project:        "Envoy".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Caddy TLS termination + Bearer auth gate; ingress_gate middleware".into(),
            alignment:           AlignmentLevel::Alternative,
            notes:               "Caddy handles TLS + auth at the edge in non-k8s deployments; Envoy filter chain is the recommended gateway in k8s/Istio deployments".into(),
        },
        // ── Registry ─────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Container Registry".into(),
            cncf_project:        "Harbor".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Zot registry — content-addressed OCI store; Push gate".into(),
            alignment:           AlignmentLevel::Compatible,
            notes:               "Zot is the native registry; Harbor adds RBAC + vulnerability scanning for enterprise".into(),
        },
        CncfAlignment {
            category:            "Container Registry".into(),
            cncf_project:        "Zot".into(),
            cncf_status:         "sandbox".into(),
            autonomyx_component: "Push gate — content-addressed OCI store".into(),
            alignment:           AlignmentLevel::Native,
            notes:               "Zot is the default registry; ORAS protocol for artifact push".into(),
        },
        // ── Workflow ─────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Workflow".into(),
            cncf_project:        "Argo Workflows".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Agent ReAct loop + lifecycle Build→Sign→Push→Sync→Deploy→Run→Observe→Feedback".into(),
            alignment:           AlignmentLevel::Compatible,
            notes:               "Each lifecycle stage maps to an Argo Workflow step; direct Argo CRD emission planned".into(),
        },
        CncfAlignment {
            category:            "Distributed Application Runtime".into(),
            cncf_project:        "Dapr".into(),
            cncf_status:         "graduated".into(),
            autonomyx_component: "Fabric event bus + multiserver WS bridge + MCP server".into(),
            alignment:           AlignmentLevel::Partial,
            notes:               "Dapr provides sidecar pub/sub + state + service invocation; fabric is the native equivalent; Dapr state store integration planned".into(),
        },
        // ── Edge ─────────────────────────────────────────────────────────────
        CncfAlignment {
            category:            "Edge".into(),
            cncf_project:        "KubeEdge".into(),
            cncf_status:         "incubating".into(),
            autonomyx_component: "edge-rs plugin + intelligent port assignment + auto-clustering peer mesh".into(),
            alignment:           AlignmentLevel::Partial,
            notes:               "Platform runs natively at the edge (single binary, any OS); KubeEdge is the integration path for managed edge-k8s".into(),
        },
        // ── SBOM / Supply Chain ───────────────────────────────────────────────
        CncfAlignment {
            category:            "Supply Chain Security — SBOM".into(),
            cncf_project:        "Syft / Grype (CNCF-adjacent)".into(),
            cncf_status:         "n/a".into(),
            autonomyx_component: "BOM module (partial)".into(),
            alignment:           AlignmentLevel::Planned,
            notes:               "Sign gate should attach Syft-generated SBOM to every image; BOM module exists but not wired to Sign gate — HIGH PRIORITY gap".into(),
        },
    ]
}

pub fn gaps() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": "gap_sbom",
            "gap": "SBOM generation and attestation",
            "cncf_project": "Syft / Grype",
            "priority": "high",
            "note": "Sign gate must attach SBOM attestation to every image. BOM module exists but is not wired to the Sign gate.",
        }),
        serde_json::json!({
            "id": "gap_policy_export",
            "gap": "Policy-as-code export (Rego / Kyverno)",
            "cncf_project": "OPA / Kyverno",
            "priority": "high",
            "note": "GovernanceGraph rules should be exportable as Rego (OPA) or Kyverno ClusterPolicy for use in k8s admission control.",
        }),
        serde_json::json!({
            "id": "gap_spire",
            "gap": "Workload identity via SPIFFE/SPIRE",
            "cncf_project": "SPIRE",
            "priority": "high",
            "note": "Pod-level workload identity in k8s should use SPIRE SVIDs alongside did:autonomyx. Enables zero-trust mesh without static secrets.",
        }),
        serde_json::json!({
            "id": "gap_nats",
            "gap": "Multi-cluster persistent event bus",
            "cncf_project": "NATS JetStream / Strimzi",
            "priority": "medium",
            "note": "Current multiserver WS bridge is point-to-point and lossy on reconnect. NATS JetStream provides durable, persistent, multi-cluster pub/sub.",
        }),
        serde_json::json!({
            "id": "gap_flux",
            "gap": "GitOps reconciliation for .ayx files",
            "cncf_project": "Flux",
            "priority": "medium",
            "note": ".ayx declarations should be Flux Source Controller reconcilable — enabling GitOps-driven agent deployments.",
        }),
        serde_json::json!({
            "id": "gap_kiali",
            "gap": "Service mesh + platform telemetry correlation",
            "cncf_project": "Kiali",
            "priority": "low",
            "note": "Fabric events carry trace IDs but Kiali graph does not yet show Autonomyx service topology alongside Istio traffic.",
        }),
        serde_json::json!({
            "id": "gap_helm",
            "gap": "Helm chart export for .ayx apps",
            "cncf_project": "Helm",
            "priority": "medium",
            "note": "Enterprise teams expect Helm charts. .ayx → Helm chart translator would lower the adoption barrier significantly.",
        }),
    ]
}

pub fn report() -> serde_json::Value {
    let map = alignment_map();
    let gaps_list = gaps();

    let mut by_level: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for a in &map {
        let level = format!("{:?}", a.alignment).to_lowercase();
        *by_level.entry(level).or_insert(0) += 1;
    }

    serde_json::json!({
        "total_mappings":   map.len(),
        "gaps":             gaps_list.len(),
        "by_alignment":     by_level,
        "cncf_landscape":   "https://landscape.cncf.io",
        "platform":         "Autonomyx — openautonomyx.com",
        "alignment_map":    map,
        "identified_gaps":  gaps_list,
        "summary": "Platform is natively aligned with Kubernetes, OpenTelemetry, Cosign, Istio, and Zot. \
                    Key gaps: SBOM attestation, policy-as-code export (OPA/Kyverno), SPIRE workload identity, \
                    and a persistent multi-cluster event bus (NATS). All gaps are on the roadmap.",
    })
}
