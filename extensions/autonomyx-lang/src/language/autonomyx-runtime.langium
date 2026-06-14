// Autonomyx Runtime Grammar
//
// A Runtime is where the agent executes.
// The same agent code runs in any runtime — modular by design.
// Runtime is declared in .ayx; the platform routes accordingly.
//
// Runtime variants:
//   open     — self-hosted, any infrastructure, full control
//   cloud    — managed by Autonomyx (openautonomyx.com), zero-ops
//   provider — hosted by a specific cloud provider (AWS, GCP, Azure)
//
// Provider-specific agents:
//   Some agents are optimised for a specific provider's capabilities
//   (e.g., Bedrock agents use AWS-native tooling, Vertex agents use GCP).
//   The provider runtime handles provider-specific SDK/API differences.
//   The agent's DID is provider-independent — identity is always portable.
//
// Real World Time:
//   All runtime targets operate in real time.
//   Latency targets are declared here and enforced by the SLA gate.
//   The fabric propagation time is sub-second in all runtime targets.
//
// Modular Architecture:
//   Every module is declared independently and composed at the Core.
//   No module depends on another at compile time (only at runtime via DI).
//   Swap any module: LLM provider, storage, identity, mesh, observability.
//   The Core is the only composition point.

grammar AutonomyxRuntime

// ── Runtime declaration ───────────────────────────────────────────────────────

Runtime:
    OpenRuntime | CloudRuntime | ProviderRuntime
;

// ── Open Runtime — self-hosted, full control ──────────────────────────────────
//
// Runs on any infrastructure: k3s, k8s, bare metal, home server, edge device.
// Self-sovereign: the operator controls every layer.
// Supply chain: Stacker SI → Zot → ArgoCD → k8s Job (agent).
// Identity: HSM or /dev/urandom, no external dependency.

OpenRuntime:
    'runtime' 'open' name=ID '{'
        'infra:'       infra=OpenInfra
        ('registry:'   registry=ID)?        // Zot adapter ref
        ('gitops:'     gitops=ID)?          // ArgoCD adapter ref
        ('mesh:'       mesh=ID)?            // Istio/Cilium adapter ref
        ('identity:'   identity=ID)?        // HSM/env adapter ref
        ('observability:' observability=ID)?
        ('sla:'        sla=ID)?
        ('gpa:'        gpa=ID)?
        ('coral:'      coral=ID)?
        ('real_time:'  realTime=RealTimeBlock)?
        modules+=RuntimeModule*
    '}'
;

OpenInfra returns string:
    'k3s'          // Lightweight k8s — home server, laptop, edge
    | 'k8s'        // Full Kubernetes
    | 'bare_metal' // No k8s — native OS (systemd service)
    | 'docker'     // Docker Compose (dev only)
    | 'podman'     // Rootless containers
;

// ── Cloud Runtime — managed by Autonomyx ─────────────────────────────────────
//
// openautonomyx.com hosts the platform.
// Zero-ops: no cluster to manage, no registry to maintain.
// Full Autonomyx feature set: AIP, lifecycle gates, fabric, federation.
// Adopter retains DID ownership — identity is portable out.

CloudRuntime:
    'runtime' 'cloud' name=ID '{'
        'endpoint:'    endpoint=STRING      // e.g. "https://api.openautonomyx.com"
        ('region:'     region=CloudRegion)?
        ('tier:'       tier=CloudTier)?
        ('sla:'        sla=ID)?
        ('finops:'     finops=ID)?
        ('real_time:'  realTime=RealTimeBlock)?
        modules+=RuntimeModule*
    '}'
;

CloudRegion returns string:
    'eu-west' | 'us-east' | 'us-west' | 'ap-southeast' | 'global'
;

CloudTier returns string:
    'community'   // shared infrastructure, community SLA
    | 'startup'   // shared + priority queue
    | 'growth'    // dedicated namespace + standard SLA
    | 'enterprise' // dedicated cluster + enterprise SLA
;

// ── Provider Runtime — hosted by a cloud provider ─────────────────────────────
//
// Agent runs inside a cloud provider's infrastructure.
// Provider-specific agents use the provider's native SDK/tools:
//   AWS:   Bedrock agents, Lambda, ECS, SageMaker
//   GCP:   Vertex AI agents, Cloud Run, GKE
//   Azure: Azure AI Foundry, AKS, Azure Functions
//
// Identity: the agent's DID is ALWAYS provider-independent.
// The provider hosts the compute; the agent owns its identity.

ProviderRuntime:
    'runtime' 'provider' name=ID '{'
        'cloud:'       cloud=CloudProvider
        ('region:'     region=STRING)?
        ('account_env:' accountEnv=STRING)?
        ('native_tools:' '[' nativeTools+=NativeTool (',' nativeTools+=NativeTool)* ']')?
        ('sla:'        sla=ID)?
        ('finops:'     finops=ID)?
        ('real_time:'  realTime=RealTimeBlock)?
        modules+=RuntimeModule*
    '}'
;

CloudProvider returns string:
    'aws' | 'gcp' | 'azure' | 'hetzner' | 'cloudflare' | 'fly' | 'render'
;

NativeTool returns string:
    // AWS
    'bedrock' | 'lambda' | 'ecs' | 'sagemaker' | 'step_functions' |
    // GCP
    'vertex_ai' | 'cloud_run' | 'gke' | 'cloud_functions' |
    // Azure
    'ai_foundry' | 'aks' | 'azure_functions' | 'cognitive_services' |
    // Common
    'managed_k8s' | 'object_storage' | 'managed_db'
;

// ── Real World Time ────────────────────────────────────────────────────────────
//
// Every runtime operates in real time.
// Latency targets here are wired to the SLA gate.
// If the gate exceeds the target, dead-letter fires immediately.
// No batch mode. No async-only. Agents respond in real time.

RealTimeBlock:
    '{'
        ('gate_latency_p99_ms:'   gateP99Ms=INT)?     // gate open/close target
        ('run_latency_p99_ms:'    runP99Ms=INT)?       // first token target
        ('fabric_propagation_ms:' fabricMs=INT)?       // event propagation target
        ('did_resolution_ms:'     didMs=INT)?           // DID lookup target
        ('handshake_ms:'          handshakeMs=INT)?    // AIP handshake target
        ('throughput_tps:'        throughputTps=INT)?  // transactions per second
    '}'
;

// ── Runtime Module — modular architecture ─────────────────────────────────────
//
// A module is an independently deployable component of the runtime.
// All modules are optional — compose only what you need.
// Each module has a declared interface; swap implementations freely.
// The Core is the only module that knows about all others.

RuntimeModule:
    'module' name=ModuleName ('ref:' ref=ID)? ('enabled:' enabled=BOOL)? ';'
;

ModuleName returns string:
    // Autonomyx core modules
    'lifecycle'     // Gate + oath framework
    | 'fabric'      // Event framework (fills gaps between gates)
    | 'federation'  // Identity + DID framework
    | 'core'        // Composition engine (the composer)
    | 'bom'         // Bill of Materials + supply chain
    | 'configdb'    // SurrealDB config store
    | 'marketplace' // Provider/operator/profile registry
    | 'gateway'     // Ingress + egress gate
    | 'transfer'    // Peer transfer (egress-push protocol)
    | 'agent'       // Agent execution (ReAct loop)
    | 'tools'       // Tool registry + invocation
    | 'providers'   // LLM provider router
    | 'coral'       // Ecosystem health monitor
    // Infrastructure modules (adapters)
    | 'mesh'
    | 'registry'
    | 'gitops'
    | 'observability'
    | 'identity'
    | 'finops'
    | 'sla'
    | 'gpa'
    | 'ide'
;

// ── Provider-specific agent binding ───────────────────────────────────────────
//
// Some agents declare a preferred provider runtime.
// The platform honours this binding when scheduling the agent Job.
// The agent's DID Document records the binding (updatable without key rotation).

ProviderBinding:
    'provider_binding' '{'
        'runtime:'   runtimeRef=ID
        ('model:'    modelRef=ID)?
        ('region:'   region=STRING)?
        ('fallback:' fallback=ID)?    // fallback runtime if primary unavailable
    '}'
;
