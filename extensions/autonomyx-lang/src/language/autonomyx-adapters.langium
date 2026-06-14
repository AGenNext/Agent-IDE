// Autonomyx Adapters Grammar
//
// Adapters are the integration layer of the platform.
// An Adapter connects the agent runtime to any external system
// without coupling the agent's identity or logic to that system.
//
// Adapter types:
//   llm          — LLM provider adapter (OpenAI, Anthropic, Ollama, ...)
//   tool         — Tool execution adapter (HTTP, shell, WASM, MCP, ...)
//   store        — Storage adapter (SurrealDB, Redis, S3, local fs, ...)
//   event        — Event adapter (Kafka, NATS, webhook, SurrealDB live, ...)
//   identity     — Identity adapter (HSM, TPM, env-key, Vault, ...)
//   registry     — OCI registry adapter (Zot, ECR, GCR, GHCR, ...)
//   gitops       — GitOps adapter (ArgoCD, Flux, ...)
//   mesh         — Service mesh adapter (Istio, Cilium, Linkerd, ...)
//   observability — OTel adapter (Jaeger, Tempo, Prometheus, Datadog, ...)
//   ide          — IDE adapter (Theia, VS Code, JetBrains, ...)
//   adopter      — End-user adoption adapter (customer integration surface)
//
// Every adapter declares:
//   - what it provides (capability surface)
//   - what it requires (env vars, secrets, config)
//   - what it costs (FinOps allocation)
//   - how it authenticates (identity primitive)
//   - which world it runs in
//
// Adopters (Agent Adopters):
//   An Adopter is a special adapter that represents a customer or end-user
//   integrating Autonomyx into their own product or workflow.
//   Adopters have their own DID, governance policy, and FinOps budget.
//   The feedback gate closes the loop from Adopter signal back to build.

grammar AutonomyxAdapters

// ── Top-level declarations ────────────────────────────────────────────────────

Adapter:
    LlmAdapter | ToolAdapter | StoreAdapter | EventAdapter |
    IdentityAdapter | RegistryAdapter | GitOpsAdapter |
    MeshAdapter | ObservabilityAdapter | IdeAdapter | AdopterAdapter
;

// ── LLM Adapter ───────────────────────────────────────────────────────────────

LlmAdapter:
    'adapter' 'llm' name=ID '{'
        'provider:'    provider=LlmProvider
        'base_url_env:' baseUrlEnv=STRING
        'api_key_env:' apiKeyEnv=STRING
        ('timeout_secs:' timeoutSecs=INT)?
        ('max_retries:'  maxRetries=INT)?
        ('world:'        world=WorldKind)?
        ('cost:'         cost=LlmCostRef)?
    '}'
;

LlmProvider returns string:
    'openai' | 'anthropic' | 'ollama' | 'vllm' | 'groq' |
    'mistral' | 'together' | 'azure_openai' | 'bedrock' |
    'vertex' | 'openai_compatible'
;

LlmCostRef:
    'pricing:' pricingRef=ID
;

// ── Tool Adapter ──────────────────────────────────────────────────────────────

ToolAdapter:
    'adapter' 'tool' name=ID '{'
        'impl:'       impl=ToolImpl
        ('url:'       url=STRING)?
        ('auth_env:'  authEnv=STRING)?
        ('timeout_secs:' timeoutSecs=INT)?
        ('world:'     '[' worlds+=WorldKind (',' worlds+=WorldKind)* ']')?
        ('sandbox:'   sandbox=SandboxKind)?
    '}'
;

ToolImpl returns string:
    'http' | 'shell' | 'python' | 'wasm' | 'native' | 'mcp' | 'k8s_job'
;

SandboxKind returns string:
    'none' | 'container' | 'wasm' | 'gvisor' | 'firecracker'
;

// ── Store Adapter ─────────────────────────────────────────────────────────────

StoreAdapter:
    'adapter' 'store' name=ID '{'
        'engine:'     engine=StoreEngine
        ('url_env:'   urlEnv=STRING)?
        ('path_env:'  pathEnv=STRING)?
        ('ns:'        ns=STRING)?
        ('db:'        db=STRING)?
        ('tls:'       tls=BOOL)?
    '}'
;

StoreEngine returns string:
    'surrealdb' | 'redis' | 'postgres' | 'sqlite' | 'rocksdb' |
    'local_fs' | 's3' | 'minio'
;

// ── Event Adapter ─────────────────────────────────────────────────────────────

EventAdapter:
    'adapter' 'event' name=ID '{'
        'transport:'   transport=EventTransport
        ('url_env:'    urlEnv=STRING)?
        ('topic:'      topic=STRING)?
        ('group_id:'   groupId=STRING)?
        ('delivery:'   delivery=DeliveryGuarantee)?
    '}'
;

EventTransport returns string:
    'surreal_live' | 'nats' | 'kafka' | 'webhook' | 'sse' | 'websocket'
;

DeliveryGuarantee returns string:
    'at_most_once' | 'at_least_once' | 'exactly_once'
;

// ── Identity Adapter ──────────────────────────────────────────────────────────

IdentityAdapter:
    'adapter' 'identity' name=ID '{'
        'backend:'    backend=IdentityBackend
        ('key_env:'   keyEnv=STRING)?
        ('vault_path:' vaultPath=STRING)?
        ('rotation:'  rotation=RotationPolicy)?
    '}'
;

IdentityBackend returns string:
    'env_key' | 'hsm' | 'tpm' | 'vault' | 'aws_kms' | 'gcp_kms'
;

RotationPolicy returns string:
    'never' | 'daily' | 'weekly' | 'monthly' | 'on_compromise'
;

// ── Registry Adapter ──────────────────────────────────────────────────────────

RegistryAdapter:
    'adapter' 'registry' name=ID '{'
        'engine:'     engine=RegistryEngine
        'url_env:'    urlEnv=STRING
        ('auth_env:'  authEnv=STRING)?
        ('sign:'      sign=SignPolicy)?
        ('cosign_key_env:' cosignKeyEnv=STRING)?
    '}'
;

RegistryEngine returns string:
    'zot' | 'ecr' | 'gcr' | 'ghcr' | 'harbor' | 'dockerhub'
;

SignPolicy returns string:
    'required' | 'optional' | 'none'
;

// ── GitOps Adapter ────────────────────────────────────────────────────────────

GitOpsAdapter:
    'adapter' 'gitops' name=ID '{'
        'engine:'     engine=GitOpsEngine
        'url_env:'    urlEnv=STRING
        ('token_env:' tokenEnv=STRING)?
        ('app:'       app=STRING)?
        ('namespace:' ns=STRING)?
        ('sync:'      sync=SyncStrategy)?
    '}'
;

GitOpsEngine returns string:
    'argocd' | 'flux'
;

SyncStrategy returns string:
    'auto' | 'manual' | 'webhook'
;

// ── Mesh Adapter ──────────────────────────────────────────────────────────────

MeshAdapter:
    'adapter' 'mesh' name=ID '{'
        'engine:'     engine=MeshEngine
        ('mtls:'      mtls=MtlsMode)?
        ('namespace:' ns=STRING)?
    '}'
;

MeshEngine returns string:
    'istio' | 'cilium' | 'linkerd' | 'consul'
;

MtlsMode returns string:
    'strict' | 'permissive' | 'disabled'
;

// ── Observability Adapter ─────────────────────────────────────────────────────

ObservabilityAdapter:
    'adapter' 'observability' name=ID '{'
        'endpoint_env:'   endpointEnv=STRING
        ('protocol:'      protocol=OtelProtocol)?
        ('service_name:'  serviceName=STRING)?
        ('sample_rate:'   sampleRate=FLOAT)?
    '}'
;

OtelProtocol returns string:
    'grpc' | 'http'
;

// ── IDE Adapter ───────────────────────────────────────────────────────────────

IdeAdapter:
    'adapter' 'ide' name=ID '{'
        'engine:'     engine=IdeEngine
        ('extensions:' '[' exts+=STRING (',' exts+=STRING)* ']')?
        ('lsp:'       lsp=BOOL)?
        ('port:'      port=INT)?
    '}'
;

IdeEngine returns string:
    'theia' | 'vscode' | 'jetbrains' | 'neovim' | 'emacs'
;

// ── Adopter Adapter ───────────────────────────────────────────────────────────
//
// An Adopter is a customer or end-user integrating Autonomyx.
// Adopters have their own DID, governance policy, and FinOps budget.
// The feedback gate sends Adopter signals back into the build loop.
// Process excellence: every Adopter interaction is measured and accountable.

AdopterAdapter:
    'adopter' name=ID '{'
        'did:'       did=STRING         // adopter's DID (assigned on signup)
        'tier:'      tier=AdopterTier
        ('budget:'   budget=ID)?        // FinOps budget reference
        ('governance:' governance=ID)?  // governance policy reference
        ('worlds:'   '[' worlds+=WorldKind (',' worlds+=WorldKind)* ']')?
        ('feedback:'  feedback=FeedbackChannel)?
        ('contact:'   contact=STRING)?
    '}'
;

AdopterTier returns string:
    'community'   // self-hosted, open source
    | 'startup'   // managed platform, limited budget
    | 'growth'    // managed platform, standard budget
    | 'enterprise'// dedicated cluster, custom policy
    | 'partner'   // co-build, deep integration
;

FeedbackChannel returns string:
    'aip_event'  // AIP protocol feedback event
    | 'webhook'  // HTTP POST to adopter webhook
    | 'surreal'  // SurrealDB live event
    | 'in_band'  // inline in the capability response
;

// ── World kinds (shared) ──────────────────────────────────────────────────────

WorldKind returns string:
    'browser' | 'server' | 'edge' | 'mobile' | 'embedded' |
    'k8s' | 'desktop' | 'peer'
;
