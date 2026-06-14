// Autonomyx Gateway Language Extension
// Defines ingress and egress rules declaratively in .ayx files.
// The autonomyx-runner reads these at startup and configures routing accordingly.
//
// schema.org grounding:
//   Gateway → schema:WebAPI
//   IngressRule → schema:EntryPoint
//   EgressRoute → schema:Action (outbound)

// Import in main grammar via: import './autonomyx-gateway'

// ── Gateway (schema:WebAPI) ───────────────────────────────────────────────────
interface Gateway {
    name:    string
    ingress: IngressBlock
    egress:  EgressBlock
    auth:    GatewayAuth
}

Gateway:
    (schema='@schema' '(' schemaType=STRING ')')?
    'gateway' name=ID
    '{'
        ('auth:'    auth=GatewayAuth)?
        ingress=IngressBlock
        egress=EgressBlock
    '}';

// ── Auth layer ────────────────────────────────────────────────────────────────
GatewayAuth:
    'auth' '{'
        'mode:' mode=AuthMode
        ('jwks_url:'  jwksUrl=STRING)?
        ('api_key_header:' apiKeyHeader=STRING)?
        ('rate_limit:' rateLimit=RateLimitBlock)?
    '}';

AuthMode returns string:
    'bearer_jwt' | 'api_key' | 'mtls' | 'open';

RateLimitBlock:
    '{' 'rpm:' rpm=NUMBER ('burst:' burst=NUMBER)? '}';

// ── Ingress (schema:EntryPoint) ───────────────────────────────────────────────
// Defines how external traffic enters the platform.
// All inbound traffic MUST pass through the gate (no raw port exposure).
IngressBlock:
    'ingress' '{'
        rules+=IngressRule*
        ('tls:'    tls=BOOLEAN)?
        ('port:'   port=NUMBER)?
        ('host:'   host=STRING)?
    '}';

IngressRule:
    'route' path=STRING '->' target=RouteTarget
    ('{'
        ('methods:' '[' methods+=HttpMethod (',' methods+=HttpMethod)* ']')?
        ('auth:'    requireAuth=BOOLEAN)?
        ('cors:'    cors=BOOLEAN)?
        ('timeout:' timeout=NUMBER)?
    '}')?;

RouteTarget:
    service=ID (':' port=NUMBER)?;

HttpMethod returns string:
    'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH' | 'WS';

// ── Egress (outbound only — no raw inbound connections from peers) ─────────────
// All outbound calls (LLM APIs, tool endpoints, peer transfer) go through here.
// This is the security boundary: egress is explicit, ingress is gated.
EgressBlock:
    'egress' '{'
        routes+=EgressRoute*
        ('default_timeout:' defaultTimeout=NUMBER)?
        ('retry:' retryPolicy=RetryPolicy)?
    '}';

EgressRoute:
    'allow' name=ID '->' url=STRING
    ('{'
        ('auth:'    auth=EgressAuth)?
        ('timeout:' timeout=NUMBER)?
        ('tls:'     tls=BOOLEAN)?
        ('headers:' '{' headers+=HeaderDef (',' headers+=HeaderDef)* '}')?
    '}')?;

EgressAuth:
    '{' 'type:' type=EgressAuthKind ('env:' envVar=STRING)? '}';

EgressAuthKind returns string:
    'bearer_env' | 'api_key_env' | 'none';

HeaderDef:
    name=STRING ':' value=STRING;

RetryPolicy:
    '{' 'attempts:' attempts=NUMBER ('backoff:' backoff=BackoffKind)? '}';

BackoffKind returns string:
    'exponential' | 'linear' | 'fixed';

terminal ID:      /[_a-zA-Z][\w-]*/;
terminal STRING:  /"[^"]*"|'[^']*'/;
terminal NUMBER:  /[0-9]+(\.[0-9]+)?/;
terminal BOOLEAN: /true|false/;
