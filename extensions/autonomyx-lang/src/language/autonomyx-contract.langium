// Autonomyx Contract Language Extension
//
// A Contract declares what the NETWORK enforces — not the application.
// The autonomyx-runner compiles contracts to:
//   - Istio VirtualService + DestinationRule (service mesh)
//   - Cilium NetworkPolicy (eBPF layer)
//   - Kubernetes NetworkPolicy (baseline)
//
// Schema.org grounding:
//   Contract → schema:ServiceChannel / schema:Offer
//   TrafficPolicy → schema:DeliveryChargeSpecification
//   CircuitBreaker → schema:WarrantyScope
//
// Example:
//   contract llm-routing {
//       enforce: network
//       mtls:    required
//       timeout: 120s
//
//       circuit_breaker {
//           threshold: 50     // % error rate to open
//           sleep_window: 30s
//       }
//
//       traffic {
//           primary:  "openai/openai-direct/gpt4o"   weight: 70
//           fallback: "ollama/local/llama3"           weight: 20
//           premium:  "anthropic/anthropic-direct/opus" weight: 10
//           strategy: latency_weighted    // branch at edge using live network data
//       }
//   }

// ── Contract (schema:ServiceChannel) ─────────────────────────────────────────
Contract:
    (schema='@schema' '(' schemaType=STRING ')')?
    'contract' name=ID '{'
        ('enforce:'   enforcer=EnforcerKind)?
        ('mtls:'      mtls=MtlsKind)?
        ('timeout:'   timeout=Duration)?
        ('description:' description=STRING)?
        circuit_breaker=CircuitBreakerBlock?
        traffic=TrafficBlock?
        retry=RetryBlock?
        observability=ObservabilityBlock?
    '}';

EnforcerKind returns string:
    'network'      // Istio / Cilium / Linkerd (recommended)
    | 'application'  // handled in app code (escape hatch)
    | 'both';

MtlsKind returns string:
    'required'   // enforce mTLS — any plaintext rejected
    | 'permissive' // accept both (migration mode)
    | 'disabled';  // no mTLS (dev only)

// ── Circuit Breaker (network holds this, not the app) ────────────────────────
CircuitBreakerBlock:
    'circuit_breaker' '{'
        ('threshold:'    threshold=NUMBER)?    // % error rate to open circuit
        ('sleep_window:' sleepWindow=Duration)?
        ('min_requests:' minRequests=NUMBER)?
        ('half_open_requests:' halfOpen=NUMBER)?
    '}';

// ── Traffic policy — the core of edge branching ───────────────────────────────
TrafficBlock:
    'traffic' '{'
        ('strategy:' strategy=TrafficStrategy)?
        routes+=TrafficRoute*
        ('header_routing:' headerRouting=BOOLEAN)?  // X-Autonomyx-Profile overrides weights
    '}';

TrafficStrategy returns string:
    'latency_weighted'    // branch to fastest available profile (live network data)
    | 'weighted_random'   // weighted random — no health signal
    | 'failover'          // primary always; fallback only on failure
    | 'canary'            // small % to new profile, rest to primary
    | 'session_affinity'; // same client → same profile (sticky)

TrafficRoute:
    role=TrafficRole ':' profile=STRING 'weight:' weight=NUMBER
    ('when:' condition=STRING)?;

TrafficRole returns string:
    'primary' | 'fallback' | 'canary' | 'premium' | 'shadow';

// ── Retry (network-layer retry, not app) ─────────────────────────────────────
RetryBlock:
    'retry' '{'
        'attempts:' attempts=NUMBER
        ('backoff:' backoff=BackoffKind)?
        ('on:' '[' conditions+=RetryCondition (',' conditions+=RetryCondition)* ']')?
    '}';

RetryCondition returns string:
    '5xx' | 'connect-failure' | 'reset' | 'retriable-4xx' | 'gateway-error';

BackoffKind returns string:
    'exponential' | 'linear' | 'fixed';

// ── Observability attachment (CNCF: OTel) ────────────────────────────────────
ObservabilityBlock:
    'observability' '{'
        ('traces:' traces=BOOLEAN)?
        ('metrics:' metrics=BOOLEAN)?
        ('logs:' logs=BOOLEAN)?
        ('sampling:' sampling=NUMBER)?     // 0.0–1.0
        ('otel_endpoint:' endpoint=STRING)?
    '}';

// ── Duration terminal ─────────────────────────────────────────────────────────
Duration returns string:
    NUMBER ('s' | 'ms' | 'm');

terminal ID:      /[_a-zA-Z][\w-]*/;
terminal STRING:  /"[^"]*"|'[^']*'/;
terminal NUMBER:  /[0-9]+(\.[0-9]+)?/;
terminal BOOLEAN: /true|false/;
