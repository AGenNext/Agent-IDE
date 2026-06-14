// Autonomyx Lifecycle Grammar
//
// Gates are the keepers. Oaths are their twins.
// Code is the contract. Idempotent by design.
//
// The loop:
//   build → sign → push → sync → deploy → run → observe → feedback → (iterate)
//
// Each Gate declares:
//   - which stage it guards
//   - the oath (invariant) that must hold
//   - what evidence it requires in the payload
//   - what it publishes on passage (OTel span, SurrealDB event, metric)

grammar AutonomyxLifecycle

// ── Top-level declaration ─────────────────────────────────────────────────────

Lifecycle:
    'lifecycle' name=ID '{'
        stages+=LifecycleStage*
        gates+=GateDecl*
        publishes+=PublishDecl*
    '}'
;

// ── Stage ordering ────────────────────────────────────────────────────────────

LifecycleStage:
    'stage' name=StageName
        ('->' next=StageName)?
    ';'
;

StageName returns string:
    'build' | 'sign' | 'push' | 'sync' | 'deploy' | 'run' | 'observe' | 'feedback'
;

// ── Gate — the keeper ─────────────────────────────────────────────────────────
//
// A Gate guards exactly one stage. It opens iff its oath holds.
// Idempotent: a gate already open returns 'already' without side effects.

GateDecl:
    'gate' stage=StageName '{'
        oath=OathDecl
        requires+=RequireDecl*
        ('on_open' onOpen=EventBlock)?
        ('on_close' onClose=EventBlock)?
    '}'
;

// ── Oath — the invariant twin ─────────────────────────────────────────────────
//
// An Oath is a named predicate. If it breaks, the gate stays closed.
// oaths are composable: all must hold for the gate to open.

OathDecl:
    'oath' name=ID '{' predicates+=PredicateDecl+ '}'
;

PredicateDecl:
    'must' field=ID operator=PredicateOp value=STRING ';'
;

PredicateOp returns string:
    'eq' | 'ne' | 'starts_with' | 'matches' | 'present' | 'gt' | 'gte'
;

// ── Evidence required in the gate payload ────────────────────────────────────

RequireDecl:
    'require' field=ID ':' kind=FieldKind ';'
;

FieldKind returns string:
    'digest' | 'bundle' | 'ref' | 'status' | 'count' | 'id' | 'trace_id' | 'string'
;

// ── Publication — what the gate emits on passage ──────────────────────────────

PublishDecl:
    'publish' kind=PublishKind 'on' stage=StageName ';'
;

PublishKind returns string:
    'otel_span' | 'surreal_event' | 'prometheus_counter' | 'webhook'
;

// ── Event block — hooks on open/close ────────────────────────────────────────

EventBlock:
    '{' actions+=EventAction+ '}'
;

EventAction:
    EmitAction | NotifyAction
;

EmitAction:
    'emit' kind=PublishKind ('with' 'label' label=STRING)? ';'
;

NotifyAction:
    'notify' target=ID ';'
;
