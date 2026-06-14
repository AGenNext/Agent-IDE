// Autonomyx SLA + GPA Grammar
//
// SLA — Service Level Agreement: what the platform commits to.
// GPA — Governance Policy Accountability: the enforcement framework.
//
// SLAs are enforced at the gate level.
// A gate that violates an SLA fires a dead-letter event.
// GPA ties governance to accountability — you can't declare one without the other.
//
// Agent Coral:
//   The cooperative health layer of the ecosystem.
//   Named for the coral reef: diverse agents, symbiotic relationships,
//   resilient to individual failure, emergent collective intelligence.
//   Coral monitors ecosystem health and rebalances when agents drift.

grammar AutonomyxSla

// ── SLA — Service Level Agreement ────────────────────────────────────────────

Sla:
    'sla' name=ID '{'
        'scope:'       scope=SlaScope
        targets+=SlaTarget*
        penalties+=SlaPenalty*
        ('measurement:' measurement=MeasurementBlock)?
        ('reports_to:'  reportsTo=ID)?
    '}'
;

SlaScope returns string:
    'per_run' | 'per_agent' | 'per_operator' | 'per_adopter' | 'platform'
;

SlaTarget:
    'target' metric=SlaMetric '{'
        'p50:'   p50=NUMBER unit=TimeUnit ';'
        ('p95:'  p95=NUMBER)?
        ('p99:'  p99=NUMBER)?
        ('p999:' p999=NUMBER)?
        ('availability:' availability=NUMBER 'nines:' nines=INT)?
        ('error_rate_max:' errorRateMax=FLOAT)?
    '}'
;

SlaMetric returns string:
    'gate_latency'          // time for a gate to open or close
    | 'run_latency'         // time from run request to first agent token
    | 'run_completion'      // time for a full agent run to complete
    | 'handshake_latency'   // AIP handshake round-trip
    | 'fabric_propagation'  // time for a gate event to reach all subscribers
    | 'did_resolution'      // time to resolve a DID
    | 'availability'        // platform uptime
    | 'token_throughput'    // tokens per second
;

TimeUnit returns string:
    'ms' | 'secs' | 'mins'
;

SlaPenalty:
    'on_breach' metric=SlaMetric 'action:' action=SlaAction ';'
;

SlaAction returns string:
    'dead_letter'       // emit to fabric dead-letter
    | 'alert_ops'       // notify ops team
    | 'circuit_break'   // open circuit, stop traffic
    | 'downgrade'       // route to lower-latency provider
    | 'credit_adopter'  // FinOps credit to affected adopter
    | 'human_escalate'  // page on-call
;

MeasurementBlock:
    '{'
        ('window_secs:' windowSecs=INT)?
        ('export:'      export=ExportTarget)?
        ('otel_metric:' otelMetric=STRING)?
    '}'
;

ExportTarget returns string:
    'prometheus' | 'otel' | 'surreal_event' | 'webhook'
;

// ── GPA — Governance Policy Accountability ────────────────────────────────────
//
// GPA is the enforcement framework:
//   Governance: declares what is allowed (policy)
//   Policy:     the rules, encoded in the DID Document
//   Accountability: the record of what actually happened (signed log)
//
// You cannot have governance without accountability.
// GPA ties the declaration to the proof.

Gpa:
    'gpa' name=ID '{'
        governance=GpaGovernance
        policy=GpaPolicy
        accountability=GpaAccountability
    '}'
;

GpaGovernance:
    'governance' '{'
        ('max_grant_ttl:' maxGrantTtl=INT 'secs')?
        ('allowed_caps:'  '[' caps+=STRING (',' caps+=STRING)* ']')?
        ('allowed_ops:'   '[' ops+=STRING  (',' ops+=STRING)*  ']')?
        ('require_bom:'   requireBom=BOOL)?
        ('require_sig:'   requireSig=BOOL)?
        ('federation:'    federation=FedLevel)?
    '}'
;

FedLevel returns string:
    'local' | 'cluster' | 'peer' | 'global'
;

GpaPolicy:
    'policy' '{'
        'enforcement:' enforcement=EnforcementMode
        ('audit_all:'  auditAll=BOOL)?
        ('deny_list:'  '[' denied+=STRING (',' denied+=STRING)* ']')?
        ('allow_list:' '[' allowed+=STRING (',' allowed+=STRING)* ']')?
        ('on_violation:' onViolation=ViolationAction)?
    '}'
;

EnforcementMode returns string:
    'hard'      // gate closes immediately on violation
    | 'soft'    // gate opens with warning, violation logged
    | 'audit'   // gate opens, everything logged for review
;

ViolationAction returns string:
    'reject' | 'warn' | 'escalate' | 'quarantine'
;

GpaAccountability:
    'accountability' '{'
        'log_all:'     logAll=BOOL
        ('sign_records:' signRecords=BOOL)?
        ('replicate_to:' replicateTo=STRING)?
        ('retention_days:' retentionDays=INT)?
        ('export:'      export=ExportTarget)?
    '}'
;

// ── Agent Coral — ecosystem health + cooperative intelligence ─────────────────
//
// Coral is the health layer of the Autonomyx ecosystem.
// Like a coral reef: diverse agents in symbiosis, resilient to failure,
// emergent collective intelligence from individual simplicity.
//
// Coral monitors:
//   - ecosystem diversity (are agents spread across providers and worlds?)
//   - resilience (can the system absorb individual agent failures?)
//   - balance (is no single provider or operator monopolising the ecosystem?)
//   - cooperation (are agents sharing results via the fabric?)
//   - health (SLA compliance, accountability log integrity, gate success rates)
//
// Coral acts when health drifts:
//   - rebalances traffic via the gateway
//   - triggers agent restarts or replacements via k8s
//   - fires dead-letter events when diversity drops below threshold
//   - alerts promoters and operators when ecosystem balance breaks

CoralSystem:
    'coral' name=ID '{'
        'monitors:'    '[' monitors+=CoralMonitor (',' monitors+=CoralMonitor)* ']'
        ('thresholds:' '[' thresholds+=CoralThreshold (',' thresholds+=CoralThreshold)* ']')?
        ('actions:'    '[' actions+=CoralAction (',' actions+=CoralAction)* ']')?
        ('healthcheck:' healthcheckSecs=INT 'secs')?
    '}'
;

CoralMonitor returns string:
    'diversity'          // provider + world spread
    | 'resilience'       // failure tolerance
    | 'balance'          // no single point of concentration
    | 'cooperation'      // fabric event participation rate
    | 'sla_compliance'   // gate SLA success rate
    | 'bom_integrity'    // all running artifacts have valid BOMs
    | 'accountability'   // all actions are signed and logged
    | 'finops'           // spend within budget for all adopters
;

CoralThreshold:
    'threshold' monitor=CoralMonitor 'min:' min=FLOAT ';'
;

CoralAction:
    'on_breach' monitor=CoralMonitor 'action:' action=CoralActionKind ';'
;

CoralActionKind returns string:
    'rebalance_traffic'    // gateway weight adjustment
    | 'spawn_replacement'  // k8s Job: new agent instance
    | 'drain_agent'        // graceful drain + replace
    | 'alert'              // fabric dead-letter + notification
    | 'pause_adopter'      // suspend adopter traffic (budget/GPA breach)
    | 'escalate_human'     // page on-call engineer
;
