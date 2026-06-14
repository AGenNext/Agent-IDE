// Autonomyx Support Grammar
//
// Foundational support is built into the platform, not bolted on.
// Every gate failure, every dead-letter event, every SLA breach
// routes to the right support channel automatically.
//
// Support tiers:
//   community    — self-service, documentation, open issues, community forum
//   startup      — async support (48h SLA), shared queue, onboarding session
//   growth       — priority support (8h SLA), dedicated CSM, monthly review
//   enterprise   — premium support (1h SLA), named TAM, 24/7 on-call,
//                  custom SLA, dedicated cluster, co-build access
//   partner      — co-build, joint roadmap, white-glove implementation
//
// Enterprise assistance scope:
//   - Implementation: architecture review, deployment, integration
//   - Operations: incident response, runbook co-authoring, chaos testing
//   - Adoption: training, certification, enablement workshops
//   - Governance: GPA policy design, audit review, compliance alignment
//   - FinOps: cost optimisation, provider negotiation, budget modelling
//   - Supply chain: BOM review, cosign policy, SBOM compliance
//
// The Support declaration ties to:
//   - Adopter tier (foundational / enterprise)
//   - SLA response times (gate: support_response)
//   - Coral monitor: support_health
//   - Accountability log: all support interactions are signed + recorded

grammar AutonomyxSupport

// ── Support tier declaration ──────────────────────────────────────────────────

SupportPlan:
    'support' name=ID '{'
        'tier:'         tier=SupportTier
        'adopter:'      adopter=ID              // Adopter reference

        // Channels available at this tier
        'channels:'     '[' channels+=SupportChannel (',' channels+=SupportChannel)* ']'

        // Response time SLAs per severity
        slas+=SupportSla*

        // Enterprise assistance modules (enterprise + partner only)
        ('assistance:'  '[' assistance+=AssistanceKind (',' assistance+=AssistanceKind)* ']')?

        // Named contacts
        ('csm:'         csm=ContactRef)?         // Customer Success Manager
        ('tam:'         tam=ContactRef)?         // Technical Account Manager
        ('on_call:'     onCall=ContactRef)?      // 24/7 on-call engineer

        // Escalation policy
        ('escalation:'  escalation=EscalationPolicy)?

        // Foundational resources
        ('docs:'        docs=STRING)?
        ('runbooks:'    runbooks=STRING)?
        ('community:'   community=STRING)?
    '}'
;

SupportTier returns string:
    'community'   // self-service only; GitHub issues; community forum
    | 'startup'   // async 48h; shared queue; onboarding session x1
    | 'growth'    // 8h business-hours; dedicated CSM; monthly review
    | 'enterprise'// 1h critical; named TAM; 24/7 on-call; custom SLA
    | 'partner'   // white-glove; co-build; joint roadmap; no queue
;

SupportChannel returns string:
    'docs'          // self-service documentation (all tiers)
    | 'community'   // forum + GitHub issues (community+)
    | 'async_ticket'// shared ticket queue (startup+)
    | 'priority_queue' // priority routing (growth+)
    | 'slack_shared'// shared Slack channel (growth+)
    | 'slack_private'// private dedicated channel (enterprise+)
    | 'video_call'  // scheduled calls (growth+)
    | 'on_call'     // 24/7 pager (enterprise+)
    | 'co_build'    // joint engineering sessions (partner)
;

// ── Support SLA — response time per severity ──────────────────────────────────

SupportSla:
    'sla' severity=Severity 'response:' response=INT unit=TimeUnit
    ('resolution:' resolution=INT unit2=TimeUnit)?
    ('escalates_to:' escalatesTo=Severity 'after:' escalateAfter=INT unit3=TimeUnit)?
;

Severity returns string:
    'critical'  // platform down, data loss, security breach
    | 'high'    // major feature broken, gate failing, SLA breached
    | 'medium'  // degraded performance, workaround available
    | 'low'     // minor issue, cosmetic, documentation gap
    | 'info'    // question, how-to, feature request
;

TimeUnit returns string:
    'mins' | 'hours' | 'days'
;

// ── Enterprise Assistance ─────────────────────────────────────────────────────
//
// Assistance is structured engagement, not ad-hoc tickets.
// Each assistance module is a defined deliverable with outcomes.
// All assistance interactions are signed + recorded (accountability log).

AssistanceKind returns string:
    // Implementation
    'architecture_review'    // review .ayx declarations, runtime config
    | 'deployment_assist'    // hands-on k8s/k3s deployment support
    | 'integration_build'    // co-build adapters and integrations
    | 'migration_support'    // migrate from existing agent platforms

    // Operations
    | 'incident_response'    // 24/7 incident handling + post-mortem
    | 'runbook_authoring'    // create operational runbooks for the adopter
    | 'chaos_testing'        // controlled failure injection + resilience review
    | 'capacity_planning'    // right-size resources, predict growth

    // Adoption
    | 'training_workshops'   // hands-on training for the adopter's team
    | 'certification'        // official Autonomyx certification programme
    | 'enablement_materials' // custom docs, guides, reference architectures
    | 'onboarding'           // structured 30/60/90 day onboarding programme

    // Governance
    | 'gpa_policy_design'    // design GPA policies for the adopter's context
    | 'audit_review'         // review accountability logs for compliance
    | 'compliance_alignment' // align with SOC2, ISO27001, GDPR, HIPAA

    // FinOps
    | 'cost_optimisation'    // analyse spend, right-size model selection
    | 'provider_negotiation' // assist with LLM provider contract negotiation
    | 'budget_modelling'     // project costs at scale

    // Supply Chain
    | 'bom_review'           // review BOM for supply chain compliance
    | 'cosign_policy'        // design cosign signing policy
    | 'sbom_compliance'      // CycloneDX / SPDX SBOM compliance review
;

// ── Contact Reference ─────────────────────────────────────────────────────────

ContactRef:
    '{'
        'name:'  name=STRING
        ('email:' email=STRING)?
        ('did:'   did=STRING)?       // contact's DID (accountable, verifiable)
        ('tz:'    tz=STRING)?        // IANA timezone
        ('hours:' hours=STRING)?     // "09:00-18:00 Mon-Fri"
    '}'
;

// ── Escalation Policy ─────────────────────────────────────────────────────────

EscalationPolicy:
    '{'
        steps+=EscalationStep*
        ('notify_ops:' notifyOps=BOOL)?
        ('post_mortem_required:' postMortem=BOOL)?
    '}'
;

EscalationStep:
    'after:' after=INT unit=TimeUnit
    'escalate_to:' target=EscalationTarget ';'
;

EscalationTarget returns string:
    'csm' | 'tam' | 'on_call' | 'engineering_lead' | 'exec' | 'coral_monitor'
;

// ── Foundational Support Resources ───────────────────────────────────────────
//
// Foundational support is self-service. It is always available.
// No ticket required. No SLA wait. Built into the platform.
//
// Every runtime exposes:
//   GET /support          — support tier + channels for this deployment
//   GET /support/runbooks — links to operational runbooks
//   GET /health           — platform health (used by all tiers for triage)
//   GET /api/fabric/dead  — dead-letter events (first escalation signal)
//   GET /api/lifecycle/log— gate audit trail (support evidence)

FoundationalResources:
    'foundational' '{'
        'docs:'      docs=STRING         // documentation URL
        ('api_ref:'  apiRef=STRING)?     // API reference URL
        ('runbooks:' runbooks=STRING)?   // operational runbooks
        ('community:'community=STRING)?  // community forum / Discord / GitHub
        ('status:'   status=STRING)?     // status page URL
        ('changelog:'changelog=STRING)?  // changelog URL
        ('spec:'     spec=STRING)?       // AGENT_SPEC.md / AIP.md
    '}'
;

// ── Built-in support tiers — canonical Autonomyx definitions ─────────────────

support community_support {
    tier: community
    adopter: community_adopter
    channels: [docs, community]
    sla info     response: 72  hours ;
    sla low      response: 72  hours ;
    sla medium   response: 72  hours ;
    sla high     response: 72  hours ;
    sla critical response: 72  hours ;
    docs:      "https://docs.openautonomyx.com"
    community: "https://github.com/AGenNextHub/Agent-IDE/issues"
}

support startup_support {
    tier: startup
    adopter: startup_adopter
    channels: [docs, community, async_ticket, video_call]
    assistance: [onboarding, architecture_review]
    sla info     response: 72 hours ;
    sla low      response: 48 hours ;
    sla medium   response: 24 hours ;
    sla high     response: 8  hours ;
    sla critical response: 4  hours ;
    docs:      "https://docs.openautonomyx.com"
    community: "https://github.com/AGenNextHub/Agent-IDE/issues"
}

support enterprise_support {
    tier: enterprise
    adopter: enterprise_adopter
    channels: [docs, async_ticket, priority_queue, slack_private, video_call, on_call]
    assistance: [
        architecture_review, deployment_assist, integration_build,
        incident_response, runbook_authoring,
        training_workshops, certification,
        gpa_policy_design, audit_review,
        cost_optimisation, bom_review, cosign_policy
    ]
    sla info     response: 24 hours ;
    sla low      response: 8  hours ;
    sla medium   response: 4  hours ;
    sla high     response: 1  hours ;
    sla critical response: 15 mins  ;
    escalation {
        after: 15 mins  escalate_to: on_call ;
        after: 1  hours escalate_to: engineering_lead ;
        after: 4  hours escalate_to: exec ;
        notify_ops: true
        post_mortem_required: true
    }
    docs:      "https://docs.openautonomyx.com"
    runbooks:  "https://docs.openautonomyx.com/runbooks"
}

support partner_support {
    tier: partner
    adopter: partner_adopter
    channels: [docs, slack_private, video_call, on_call, co_build]
    assistance: [
        architecture_review, deployment_assist, integration_build, migration_support,
        incident_response, runbook_authoring, chaos_testing, capacity_planning,
        training_workshops, certification, enablement_materials, onboarding,
        gpa_policy_design, audit_review, compliance_alignment,
        cost_optimisation, provider_negotiation, budget_modelling,
        bom_review, cosign_policy, sbom_compliance
    ]
    sla critical response: 15 mins ;
    sla high     response: 30 mins ;
    sla medium   response: 2  hours ;
    sla low      response: 4  hours ;
    escalation {
        after: 15 mins escalate_to: on_call ;
        after: 1 hours escalate_to: engineering_lead ;
        notify_ops: true
        post_mortem_required: true
    }
    docs:      "https://docs.openautonomyx.com"
    runbooks:  "https://docs.openautonomyx.com/runbooks"
}
