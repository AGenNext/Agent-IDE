// Autonomyx FinOps Grammar
//
// Financial operations are first-class in the platform.
// Every agent run has a cost. Every cost has an owner.
// Budget caps are enforced at the gate — not after the fact.
//
// FinOps dimensions:
//   budget     — cap per agent / per run / per workflow / per operator
//   allocation — assign cost to a DID, a team, a project
//   tracking   — token usage → price per token → run cost → cumulative spend
//   reporting  — spend by agent, by operator, by world, by time
//   gate       — FinOps gate: can this run proceed given the current budget?
//
// Cost in the BOM:
//   Every artifact knows what it costs to produce.
//   The build gate records compute cost.
//   The run gate records token cost.
//   The observe gate records total cost of the loop.
//
// Ecosystem balance:
//   Self-hosted models (Ollama, vllm) have $0 token cost.
//   Cloud providers have per-token pricing.
//   The platform shows you the delta — you decide.

grammar AutonomyxFinOps

// ── Top-level ─────────────────────────────────────────────────────────────────

FinOps:
    'finops' name=ID '{'
        budgets+=Budget*
        allocations+=Allocation*
        pricing+=ProviderPricing*
        ('alerts:' '[' alerts+=FinOpsAlert (',' alerts+=FinOpsAlert)* ']')?
        ('reporting:' reporting=ReportingBlock)?
    '}'
;

// ── Budget ────────────────────────────────────────────────────────────────────
// A budget is a cap. The FinOps gate refuses to open if the cap is exceeded.

Budget:
    'budget' name=ID '{'
        'scope:'    scope=BudgetScope
        'limit:'    limit=NUMBER  'currency:' currency=Currency
        ('period:'  period=BudgetPeriod)?
        ('agent:'   agent=ID)?    // restrict to one agent
        ('operator:' operator=STRING)? // restrict to one operator
        ('on_exceed:' onExceed=ExceedPolicy)?
    '}'
;

BudgetScope returns string:
    'per_run' | 'per_agent' | 'per_workflow' | 'per_operator' | 'global'
;

BudgetPeriod returns string:
    'hourly' | 'daily' | 'weekly' | 'monthly' | 'unlimited'
;

Currency returns string:
    'USD' | 'EUR' | 'GBP' | 'credits'
;

ExceedPolicy returns string:
    'reject'           // gate closes — run refused
    | 'warn'           // run proceeds, alert fired
    | 'downgrade'      // switch to cheaper model automatically
    | 'human_approve'  // pause and wait for human approval
;

// ── Cost Allocation ───────────────────────────────────────────────────────────
// Map cost to owners: DID, team, project, world.

Allocation:
    'allocate' cost=CostTarget 'to' owner=AllocationTarget ';'
;

CostTarget returns string:
    'agent_runs' | 'build_compute' | 'registry_storage' | 'egress_traffic' | 'all'
;

AllocationTarget:
    ('did:' did=STRING)?
    ('team:' team=ID)?
    ('project:' project=ID)?
    ('world:' world=WorldTarget)?
;

WorldTarget returns string:
    'browser' | 'server' | 'edge' | 'mobile' | 'embedded' | 'k8s' | 'desktop' | 'peer'
;

// ── Provider Pricing ──────────────────────────────────────────────────────────
// Declare the cost per token for each provider/model.
// Self-hosted models: input_per_1m = 0, output_per_1m = 0.

ProviderPricing:
    'pricing' provider=STRING model=STRING '{'
        'input_per_1m:'  inputPer1m=NUMBER  'currency:' currency=Currency
        'output_per_1m:' outputPer1m=NUMBER 'currency:' currency2=Currency
        ('context_window:' contextWindow=INT)?
        ('self_hosted:' selfHosted=BOOL)?
    '}'
;

// ── FinOps Alert ──────────────────────────────────────────────────────────────

FinOpsAlert:
    'alert' name=ID '{'
        'when:'   condition=AlertCondition
        'notify:' '[' targets+=ID (',' targets+=ID)* ']'
        ('action:' action=AlertAction)?
    '}'
;

AlertCondition:
    'spend_exceeds:' threshold=NUMBER 'currency:' currency=Currency
    ('period:' period=BudgetPeriod)?
;

AlertAction returns string:
    'pause' | 'downgrade' | 'notify_only'
;

// ── Reporting ─────────────────────────────────────────────────────────────────

ReportingBlock:
    '{'
        ('frequency:' frequency=ReportFrequency)?
        ('dimensions:' '[' dims+=ReportDimension (',' dims+=ReportDimension)* ']')?
        ('export:'    export=ExportTarget)?
    '}'
;

ReportFrequency returns string:
    'realtime' | 'hourly' | 'daily' | 'weekly'
;

ReportDimension returns string:
    'agent' | 'operator' | 'world' | 'model' | 'workflow' | 'did' | 'team'
;

ExportTarget returns string:
    'surreal_event' | 'prometheus' | 'otel' | 'webhook'
;
