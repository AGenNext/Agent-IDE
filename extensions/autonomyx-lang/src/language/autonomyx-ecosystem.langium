// Autonomyx Ecosystem Grammar
//
// The platform is not a product. It is an ecosystem.
// Every role in the ecosystem is a first-class declaration.
//
// Roles:
//   ModelProvider  — supplies intelligence (LLMs, reasoning engines)
//   Provider       — supplies capabilities (compute, storage, tools, identity)
//   Operator       — runs the platform on behalf of others
//   Adopter        — integrates the platform into their product/workflow
//   Promoter       — champions, distributes, and grows the ecosystem
//
// Hierarchy:
//   Ecosystem
//     └── ModelProvider (OpenAI, Anthropic, Ollama, Groq, ...)
//           └── Operator (direct, Azure, Bedrock, ...)
//                 └── Profile (gpt-4o, claude-opus-4-8, llama3, ...)
//
//     └── Provider (compute, storage, identity, tools, observability, ...)
//           └── Service (k8s, Zot, SurrealDB, Vault, OTel, ...)
//
//     └── Adopter (end-user, customer, ISV, ...)
//           └── Integration (SDK, API, AIP, ...)
//
//     └── Promoter (community, partner, reseller, ISV, ...)
//           └── Channel (open-source, marketplace, co-sell, ...)
//
// Ecosystem balance is maintained by:
//   - Provider independence: no single provider required
//   - Open protocol: AIP is the standard, not a proprietary API
//   - Self-hostable: every component runs on a k3s home server
//   - Open adoption: community tier is free, self-hosted, full-featured
//   - Accountable promoters: promoter contributions are measured + credited

grammar AutonomyxEcosystem

// ── Ecosystem container ───────────────────────────────────────────────────────

Ecosystem:
    'ecosystem' name=ID '{'
        modelProviders+=ModelProvider*
        providers+=CapabilityProvider*
        operators+=EcosystemOperator*
        adopters+=EcosystemAdopter*
        promoters+=Promoter*
    '}'
;

// ── Model Provider ────────────────────────────────────────────────────────────
//
// An entity that supplies intelligence to the platform.
// Model providers are provider-independent — the platform routes to any.
// Self-hosted (Ollama, vllm) are first-class model providers.

ModelProvider:
    'model_provider' name=ID '{'
        'label:'       label=STRING
        'kind:'        kind=ModelProviderKind
        ('url_env:'    urlEnv=STRING)?
        ('auth_env:'   authEnv=STRING)?
        'models:'      '[' models+=ModelDef (',' models+=ModelDef)* ']'
        ('tier:'       tier=ProviderTier)?
        ('self_hosted:' selfHosted=BOOL)?
        ('open_source:' openSource=BOOL)?
    '}'
;

ModelProviderKind returns string:
    'openai_compatible'   // Any OpenAI-compatible API (Groq, Mistral, Ollama, vllm, ...)
    | 'anthropic'         // Anthropic Messages API
    | 'ollama'            // Ollama native API
    | 'bedrock'           // AWS Bedrock
    | 'vertex'            // Google Vertex AI
    | 'azure_openai'      // Azure OpenAI
    | 'foundry'           // Microsoft Azure AI Foundry
    | 'custom'            // Custom API — bring your own adapter
;

ModelDef:
    name=ID '{'
        'model_id:'      modelId=STRING
        ('context:'      contextK=INT 'k')?
        ('input_per_1m:' inputPer1m=NUMBER)?
        ('output_per_1m:' outputPer1m=NUMBER)?
        ('reasoning:'    reasoning=BOOL)?
        ('vision:'       vision=BOOL)?
        ('tools:'        tools=BOOL)?
    '}'
;

ProviderTier returns string:
    'community' | 'startup' | 'growth' | 'enterprise' | 'sovereign'
;

// ── Capability Provider ───────────────────────────────────────────────────────
//
// An entity that supplies non-intelligence capabilities:
// compute, storage, identity, tools, networking, observability.

CapabilityProvider:
    'provider' name=ID '{'
        'label:'       label=STRING
        'kind:'        kind=CapabilityKind
        ('url_env:'    urlEnv=STRING)?
        ('auth_env:'   authEnv=STRING)?
        ('region:'     region=STRING)?
        ('self_hosted:' selfHosted=BOOL)?
        services+=ProviderService*
    '}'
;

CapabilityKind returns string:
    'compute'        // k8s, bare metal, cloud VMs
    | 'storage'      // Zot, S3, SurrealDB, local fs
    | 'identity'     // HSM, TPM, Vault, KMS
    | 'networking'   // Istio, Cilium, Traefik, Caddy
    | 'observability' // OTel, Prometheus, Jaeger, Grafana
    | 'tools'        // MCP servers, web search, code exec
    | 'gitops'       // ArgoCD, Flux
    | 'registry'     // Zot, ECR, GCR, GHCR
    | 'si'           // Software Integration (Stacker, Bazel)
;

ProviderService:
    'service' name=ID '{'
        'endpoint_env:' endpointEnv=STRING
        ('version:'     version=STRING)?
        ('oss:'         oss=BOOL)?
        ('cncf:'        cncf=BOOL)?
    '}'
;

// ── Operator ──────────────────────────────────────────────────────────────────
//
// An Operator runs the Autonomyx platform on behalf of Adopters.
// Operators configure the gateway, marketplace, and governance.
// Multi-operator: same platform, different operators, different policies.

EcosystemOperator:
    'operator' name=ID '{'
        'did:'         did=STRING
        'label:'       label=STRING
        ('model_providers:' '[' mps+=ID (',' mps+=ID)* ']')?
        ('adopters:'   '[' adopters+=ID (',' adopters+=ID)* ']')?
        ('governance:' governance=ID)?
        ('finops:'     finops=ID)?
        ('region:'     region=STRING)?
    '}'
;

// ── Adopter ───────────────────────────────────────────────────────────────────
//
// An Adopter integrates the platform into their own product or workflow.
// Adopters are the customers. Their feedback closes the loop.
// Every Adopter has a DID, a governance policy, and a FinOps budget.

EcosystemAdopter:
    'adopter' name=ID '{'
        'did:'         did=STRING
        'label:'       label=STRING
        'tier:'        tier=AdopterTier
        ('operator:'   operator=ID)?
        ('domains:'    '[' domains+=STRING (',' domains+=STRING)* ']')?
        ('integration:' integration=IntegrationKind)?
        ('feedback:'   feedback=FeedbackKind)?
        ('budget:'     budget=ID)?
    '}'
;

AdopterTier returns string:
    'community' | 'startup' | 'growth' | 'enterprise' | 'partner'
;

IntegrationKind returns string:
    'aip'        // full Agent Internet Protocol integration
    | 'sdk'      // language SDK (Python, TypeScript, Go, Rust, ...)
    | 'api'      // REST API only
    | 'ide'      // via Agent IDE (Theia extension)
    | 'cli'      // autonomyx CLI
    | 'gitops'   // ArgoCD / GitOps-only (config, no runtime)
    | 'embedded' // embedded in adopter's own product
;

FeedbackKind returns string:
    'aip_event' | 'webhook' | 'surreal' | 'in_band' | 'survey' | 'telemetry'
;

// ── Promoter ──────────────────────────────────────────────────────────────────
//
// A Promoter champions and distributes the platform.
// Promoters are accountable: their contributions are measured and credited.
// Every Promoter has a DID. Their reach is tracked in the accountability log.
//
// Promoter types:
//   community   — open-source contributors, advocates, speakers
//   partner     — co-build, joint GTM, deep integration
//   reseller    — distributes the managed platform to their customers
//   isv         — builds a product on top of Autonomyx
//   developer   — builds adapters, tools, or extensions
//   ambassador  — public champion (blog, talks, social, tutorials)

Promoter:
    'promoter' name=ID '{'
        'did:'         did=STRING
        'label:'       label=STRING
        'kind:'        kind=PromoterKind
        ('channel:'    channel=PromoterChannel)?
        ('region:'     region=STRING)?
        ('reach:'      reach=PromoterReach)?
        ('incentives:' '[' incentives+=IncentiveKind (',' incentives+=IncentiveKind)* ']')?
    '}'
;

PromoterKind returns string:
    'community' | 'partner' | 'reseller' | 'isv' | 'developer' | 'ambassador'
;

PromoterChannel returns string:
    'github'        // open-source contributions
    | 'marketplace' // app store / marketplace listing
    | 'co_sell'     // joint sales with operator
    | 'social'      // social media, blog, talks
    | 'education'   // courses, tutorials, documentation
    | 'events'      // conferences, meetups, hackathons
    | 'enterprise'  // direct enterprise relationships
;

PromoterReach:
    'reach' '{'
        ('monthly_active:' monthlyActive=INT)?
        ('community_size:' communitySize=INT)?
        ('deployments:'    deployments=INT)?
    '}'
;

IncentiveKind returns string:
    'revenue_share'   // % of adopter ARR
    | 'credits'       // platform credits for self-hosted usage
    | 'co_marketing'  // joint marketing + co-branding
    | 'early_access'  // preview features + roadmap input
    | 'certification' // official Autonomyx certification
;
