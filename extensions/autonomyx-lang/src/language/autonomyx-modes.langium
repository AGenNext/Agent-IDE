// Autonomyx Agent Modes Grammar
//
// How an agent reasons and acts is a first-class declaration.
// Mode is bound in the DID Document — part of the agent's identity.
// Changing mode bumps the DID Document version.
//
// Modes:
//   react        — ReAct: Reason + Act loop (default)
//   chain        — chain-of-thought, no external tool calls
//   adaptive     — model decides (extended thinking, o1-style)
//   direct       — single-shot, no loop, no tools
//   orchestrator — manages subagents (spawns + coordinates)
//   subagent     — called by an orchestrator; returns result
//   swarm        — N parallel agents on the same task; best result wins
//   pipeline     — sequential chain; A → B → C → output

grammar AutonomyxModes

// ── Agent Mode block ──────────────────────────────────────────────────────────

AgentMode:
    ReactMode | ChainMode | AdaptiveMode | DirectMode |
    OrchestratorMode | SubagentMode | SwarmMode | PipelineMode
;

// ── react — ReAct loop ────────────────────────────────────────────────────────
// Reason → Act → Observe → Reason → ... until done.
// The default mode for tool-using agents.

ReactMode:
    'mode' 'react' '{'
        ('max_steps:'   maxSteps=INT)?
        ('max_tokens:'  maxTokens=INT)?
        ('tools:'       '[' tools+=ID (',' tools+=ID)* ']')?
        ('stop_on:'     '[' stopOn+=StopCondition (',' stopOn+=StopCondition)* ']')?
    '}'
;

StopCondition returns string:
    'answer_found' | 'tool_error' | 'budget_exceeded' | 'timeout' | 'user_interrupt'
;

// ── chain — chain-of-thought ──────────────────────────────────────────────────
// Extended internal reasoning without external tool calls.
// Use for: analysis, writing, classification, summarisation.

ChainMode:
    'mode' 'chain' '{'
        ('max_tokens:'  maxTokens=INT)?
        ('temperature:' temperature=FLOAT)?
        ('format:'      format=OutputFormat)?
    '}'
;

OutputFormat returns string:
    'text' | 'json' | 'markdown' | 'structured'
;

// ── adaptive — model decides ──────────────────────────────────────────────────
// Model uses built-in extended thinking / reasoning (Claude adaptive, o1, etc.)
// The platform does not control the loop — the model does.

AdaptiveMode:
    'mode' 'adaptive' '{'
        ('effort:'      effort=EffortLevel)?
        ('max_tokens:'  maxTokens=INT)?
    '}'
;

EffortLevel returns string:
    'low' | 'medium' | 'high' | 'xhigh' | 'max'
;

// ── direct — single-shot ──────────────────────────────────────────────────────
// One request, one response. No loop. No tools. Lowest latency.
// Use for: classification, extraction, transformation.

DirectMode:
    'mode' 'direct' '{'
        ('max_tokens:'  maxTokens=INT)?
        ('temperature:' temperature=FLOAT)?
    '}'
;

// ── orchestrator — manages subagents ─────────────────────────────────────────
// Spawns, coordinates, and collects results from subagents.
// Orchestrator holds the plan; subagents hold the tools.

OrchestratorMode:
    'mode' 'orchestrator' '{'
        'subagents:' '[' subagents+=ID (',' subagents+=ID)* ']'
        ('strategy:'  strategy=OrchestrationStrategy)?
        ('max_agents:' maxAgents=INT)?
        ('timeout_secs:' timeoutSecs=INT)?
    '}'
;

OrchestrationStrategy returns string:
    'sequential' | 'parallel' | 'fan_out' | 'map_reduce' | 'consensus'
;

// ── subagent — called by an orchestrator ──────────────────────────────────────
// A focused agent with a narrow capability set.
// Does one thing well; returns a structured result.

SubagentMode:
    'mode' 'subagent' '{'
        ('accepts:'     '[' accepts+=ID (',' accepts+=ID)* ']')?
        ('returns:'     returnType=ID)?
        ('timeout_secs:' timeoutSecs=INT)?
    '}'
;

// ── swarm — parallel agents, best result wins ─────────────────────────────────
// N independent agents work the same task simultaneously.
// The fabric collects results; the swarm coordinator picks the best.
// Use for: search, creative generation, high-stakes reasoning.

SwarmMode:
    'mode' 'swarm' '{'
        'agents:' '[' agents+=SwarmAgent (',' agents+=SwarmAgent)* ']'
        ('aggregation:' aggregation=AggregationStrategy)?
        ('quorum:'      quorum=INT)?
    '}'
;

SwarmAgent:
    name=ID ('weight:' weight=FLOAT)?
;

AggregationStrategy returns string:
    'first_success' | 'majority_vote' | 'weighted_score' | 'human_review'
;

// ── pipeline — sequential chain ───────────────────────────────────────────────
// A → B → C: each agent receives the output of the previous.
// Output type must match next agent's input type.
// Failed steps can be retried or skipped per step policy.

PipelineMode:
    'mode' 'pipeline' '{'
        'steps:' '[' steps+=PipelineStep (',' steps+=PipelineStep)* ']'
        ('on_failure:' onFailure=PipelineFailurePolicy)?
    '}'
;

PipelineStep:
    agent=ID ('retry:' retry=INT)? ('skip_on_error:' skipOnError=BOOL)?
;

PipelineFailurePolicy returns string:
    'abort' | 'skip' | 'retry' | 'human_escalate'
;
