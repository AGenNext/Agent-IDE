// Autonomyx Meta Model — the structural foundation of all agents.
//
// The meta model defines what an "agent" IS at the highest abstraction level.
// All concrete agent declarations derive from this base.
// You cannot declare an agent that violates the meta model.
//
// Meta Base vs Schema Base:
//   Schema Base (autonomyx-schema.langium)
//     → semantic grounding in Schema.org vocabulary
//     → what things MEAN to the outside world
//
//   Meta Base (this file)
//     → structural constraints for the Autonomyx type system
//     → what things MUST HAVE to be a valid Autonomyx agent
//     → the abstract interface every agent implements
//
// Meta Model layers:
//
//   MetaBase
//     ├── identity:     DID + keypair (required)
//     ├── lifecycle:    which stages the agent participates in (required)
//     ├── intelligence: model + reasoning mode (required)
//     ├── governance:   GPA policy reference (required)
//     ├── federation:   peer scope (optional, defaults to cluster)
//     ├── worlds:       deployment targets (required, at least one)
//     ├── adapters:     integration surface (optional)
//     ├── finops:       budget + allocation (optional, defaults to open)
//     ├── sla:          latency + availability targets (optional)
//     └── schema:       Schema.org type annotation (optional but recommended)
//
//   Every concrete agent is an instance of MetaBase.
//   The meta model is the grammar of the grammar.

grammar AutonomyxMeta

// ── Meta Base — the abstract agent ───────────────────────────────────────────

MetaBase:
    '@meta' '{'
        // Every agent must declare its intelligence binding
        'intelligence' '{'
            'provider:' provider=STRING
            'model:'    model=STRING
            'reasoning:' reasoning=ReasoningKind
        '}'

        // Every agent must declare at least one world
        'worlds:' '[' worlds+=WorldKind (',' worlds+=WorldKind)+ ']'

        // Every agent must declare its lifecycle participation
        'lifecycle' '{'
            'stages:' '[' stages+=StageName (',' stages+=StageName)* ']'
        '}'

        // Every agent must have a governance policy reference or inline policy
        'governance' '{'
            'max_grant_ttl_secs:' maxTtl=INT
            'allowed_caps:'       '[' caps+=STRING (',' caps+=STRING)* ']'
            'audit_all:'          auditAll=BOOL
        '}'

        // Optional: schema.org type annotation
        ('schema:' schemaType=STRING)?

        // Optional: federation scope
        ('federation:' federation=FedLevel)?

        // Optional: FinOps budget reference
        ('finops:' finopsRef=ID)?

        // Optional: SLA reference
        ('sla:' slaRef=ID)?
    '}'
;

// ── Meta Model — the complete Autonomyx type system ───────────────────────────
//
// The Meta Model is the authoritative definition of all types in the platform.
// It is not instantiated directly — it describes the type system.
// Use it to understand what properties are available and required.
// The validator uses it to type-check all .ayx declarations.

MetaModel:
    'meta_model' name=ID '{'
        types+=MetaType*
        traits+=MetaTrait*
        rules+=MetaRule*
    '}'
;

// ── Meta Type ─────────────────────────────────────────────────────────────────
//
// A MetaType defines a kind of declaration in the .ayx language.
// It declares required + optional fields, their types, and defaults.

MetaType:
    'type' name=ID ('extends' base=ID)? '{'
        fields+=MetaField*
        ('schema:' schemaType=STRING)?
        ('description:' description=STRING)?
    '}'
;

MetaField:
    (required='required' | optional='optional') name=ID ':' type=MetaFieldType
    ('default:' default=Literal)?
    ('description:' description=STRING)?
    ';'
;

MetaFieldType returns string:
    'String' | 'Int' | 'Float' | 'Bool' | 'DID' | 'Ref'
    | 'List<String>' | 'List<Ref>' | 'Block'
    | 'ReasoningKind' | 'WorldKind' | 'FedLevel' | 'StageName'
;

Literal returns string:
    STRING | INT | FLOAT | BOOL
;

// ── Meta Trait — shared capabilities ─────────────────────────────────────────
//
// A MetaTrait is a named set of fields that can be mixed into any MetaType.
// Traits are composable. An agent can implement many traits.
// The Core uses traits to compose module registrations.

MetaTrait:
    'trait' name=ID '{'
        ('provides:' '[' provides+=STRING (',' provides+=STRING)* ']')?
        ('requires:' '[' requires+=ID    (',' requires+=ID)*    ']')?
        fields+=MetaField*
    '}'
;

// ── Meta Rule — invariants ────────────────────────────────────────────────────
//
// A MetaRule is a named invariant over the meta model.
// Rules are enforced by the validator at compile time (LSP error).
// Violated rules prevent the agent from being stored in ConfigDB.

MetaRule:
    'rule' name=ID ':' constraint=RuleConstraint ';'
;

RuleConstraint:
    'required'  field=ID                              // field must be present
    | 'forbids' field=ID                              // field must be absent
    | 'implies' field=ID 'requires' field2=ID         // if X then Y
    | 'format'  field=ID 'matches' pattern=STRING     // regex
    | 'range'   field=ID 'min' min=NUMBER 'max' max=NUMBER
;

// ── Built-in Meta Model — the Autonomyx canonical type system ─────────────────

meta_model autonomyx {

    type AgentBase {
        required identity_backend : String  default: "env_key"  description: "HSM, TPM, Vault, or env_key" ;
        required intelligence     : Block                        description: "provider + model + reasoning" ;
        required worlds           : List<String>                 description: "at least one world required" ;
        required lifecycle_stages : List<String>  default: "run" description: "minimum: run stage" ;
        required audit_all        : Bool     default: "true"     description: "all actions logged" ;
        optional schema_type      : String                       description: "schema.org @type annotation" ;
        optional finops           : Ref                          description: "FinOps budget reference" ;
        optional sla              : Ref                          description: "SLA reference" ;
        optional federation       : FedLevel default: "cluster"  description: "peer scope" ;
    }

    type ToolBase {
        required description : String   description: "what the tool does" ;
        required impl        : String   description: "http, shell, wasm, native, mcp, k8s_job" ;
        optional url         : String   description: "endpoint URL (impl: http or mcp)" ;
        optional auth_env    : String   description: "env var holding auth token" ;
    }

    type WorkflowBase {
        required steps       : List<Ref> description: "ordered list of step references" ;
        optional trigger     : String    description: "manual, schedule, event, webhook" ;
        optional timeout_secs: Int       description: "hard stop after N seconds" ;
    }

    type GateBase {
        required stage       : StageName description: "which lifecycle stage" ;
        required oath        : Block      description: "the invariant that must hold" ;
        optional on_open     : Block      description: "actions when gate opens" ;
        optional on_close    : Block      description: "actions when gate closes" ;
    }

    trait Identifiable {
        provides: ["did", "did_document", "keypair"]
        required did : DID description: "did:autonomyx:<base58-pubkey>" ;
    }

    trait Governed {
        provides: ["governance_policy", "jit_grants"]
        requires: [Identifiable]
        required max_grant_ttl_secs : Int  default: "300" ;
        required allowed_caps       : List<String> ;
        required audit_all          : Bool default: "true" ;
    }

    trait Accountable {
        provides: ["accountability_log", "signed_records"]
        requires: [Identifiable]
        required sign_records : Bool default: "true" ;
        optional replicate_to : String ;
    }

    trait Federal {
        provides: ["did_resolution", "peer_exchange"]
        requires: [Identifiable]
        required federation : FedLevel default: "cluster" ;
    }

    trait Intelligent {
        provides: ["llm_routing", "reasoning_loop"]
        required provider  : String ;
        required model     : String ;
        required reasoning : ReasoningKind ;
    }

    trait FinOpsAware {
        provides: ["token_tracking", "budget_enforcement"]
        optional budget : Ref ;
        optional pricing: Ref ;
    }

    trait SlaEnforced {
        provides: ["gate_sla", "breach_actions"]
        optional sla : Ref ;
    }

    // Rules
    rule no_api_key_in_source    : forbids api_key ;
    rule identity_requires_did   : implies identity requires did ;
    rule governed_requires_audit : implies governance requires audit_all ;
    rule worlds_not_empty        : required worlds ;
    rule intelligence_required   : required intelligence ;
}

// ── Shared terminals ──────────────────────────────────────────────────────────

ReasoningKind returns string:
    'react' | 'chain' | 'adaptive' | 'direct' |
    'orchestrator' | 'subagent' | 'swarm' | 'pipeline'
;

WorldKind returns string:
    'browser' | 'server' | 'edge' | 'mobile' | 'embedded' |
    'k8s' | 'desktop' | 'peer'
;

FedLevel returns string:
    'local' | 'cluster' | 'peer' | 'global'
;

StageName returns string:
    'build' | 'sign' | 'push' | 'sync' | 'deploy' | 'run' | 'observe' | 'feedback'
;
