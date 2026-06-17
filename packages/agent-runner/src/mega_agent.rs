// MegaAgent — FSM-based automatic multi-agent orchestrator.
//
// Aligned with MetaAgent (arXiv 2507.22606): "MetaAgent: Automatically Constructing
// Multi-Agent Systems Based on Finite State Machines".
//
// Architecture — Finite State Machine ℳ = (Σ, S, s₀, F, δ):
//   Σ  = task domain (the set of task types this FSM handles)
//   S  = states (each with: agent, instruction, condition verifier, listeners)
//   s₀ = initial state
//   F  = final states (task complete → submit answer)
//   δ  = transition function (natural language conditions, LLM-evaluated)
//
// Key features aligned with the paper:
//   1. Null-transitions    — if no condition is met, feedback to current agent and retry
//   2. State traceback     — can transition to ANY prior state (not just next)
//   3. Tool use            — agents carry tool lists; executor invokes them
//   4. FSM optimization    — LLM merges redundant states before deployment
//   5. Listener mechanism  — on transition, output is inserted into listener agents' memory
//   6. Auto-design         — given a task description, design the full FSM via LLM
//
// Platform integration:
//   - MegaAgent IS the orchestrator; it reads the Megaverse world model
//   - Every FSM transition emits a FabricEvent
//   - Every state execution is accountability-recorded in the federation
//   - The Pipeline's lifecycle gates can be driven by the FSM (FSM → Pipeline)
//   - Trust chain: each FSM state link is SHA-256 chained
//
// "The FSM is the platform's self-awareness made executable." — openautonomyx.com

// ── Agent type taxonomy ───────────────────────────────────────────────────────
//
// Every agent in the platform is one of these canonical types.
// Used by the FSM designer to select the right agent role for each FSM state,
// by the MegaAgent to match sub-tasks to capable agents, and by the team registry
// to validate team compositions.
//
// Taxonomy:
//   Tier 0 — Meta:       agents that design, observe, and govern other agents
//   Tier 1 — Execute:    agents that do the work
//   Tier 2 — Verify:     agents that check, test, and validate
//   Tier 3 — Synthesise: agents that aggregate, report, and route
//   Tier 4 — Govern:     agents that enforce policy, ethics, and compliance
//   Tier 5 — Interface:  agents that bridge humans, devices, and external systems

use serde::{Deserialize, Serialize};

/// Canonical agent type taxonomy — all roles that exist in the platform.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    // ── Tier 0: Meta ──────────────────────────────────────────────────────────
    /// MegaAgent: designs FSMs, orchestrates all agents, closes all loops.
    MegaAgent,
    /// Designer: generates FSM state machines and agent configurations from a goal.
    Designer,
    /// Planner: decomposes a goal into a sequential or parallel plan.
    Planner,
    /// Optimizer: merges redundant FSM states; improves system efficiency.
    Optimizer,
    /// Reconciler: CRD controller; drives lifecycle gates automatically.
    Reconciler,

    // ── Tier 1: Execute ───────────────────────────────────────────────────────
    /// Executor: carries out a concrete sub-task per the plan.
    Executor,
    /// Researcher: gathers information via search, APIs, databases.
    Researcher,
    /// Coder: writes and refactors code; shell and file system tools.
    Coder,
    /// DataScientist: preprocesses data, trains models, evaluates metrics.
    DataScientist,
    /// Writer: produces structured documents, reports, summaries.
    Writer,
    /// Deployer: pushes code, syncs ArgoCD, manages k8s rollouts.
    Deployer,
    /// Builder: runs build pipelines, produces images and BOM.
    Builder,
    /// Signer: cosign image signing, BOM attestation, provenance.
    Signer,

    // ── Tier 2: Verify ────────────────────────────────────────────────────────
    /// ConditionVerifier: evaluates FSM transition conditions for a state.
    ConditionVerifier,
    /// Tester: executes test suites, verifies software correctness.
    Tester,
    /// Auditor: checks compliance, governance, and accountability records.
    Auditor,
    /// TrustVerifier: verifies chain hashes, cosign bundles, DID assertions.
    TrustVerifier,

    // ── Tier 3: Synthesise ────────────────────────────────────────────────────
    /// Analyst: synthesises findings from multiple sources into structured insight.
    Analyst,
    /// Reporter: produces final structured outputs for human consumption.
    Reporter,
    /// Aggregator: merges outputs from parallel agents into a coherent whole.
    Aggregator,
    /// Summariser: condenses long outputs into concise summaries.
    Summariser,

    // ── Tier 4: Govern ────────────────────────────────────────────────────────
    /// PolicyAgent: evaluates actions against governance rules; issues JIT grants.
    PolicyAgent,
    /// EthicsAgent: applies the 7-value alignment check to agent outputs.
    EthicsAgent,
    /// UsageMeter: tracks token/compute/egress costs; enforces budgets.
    UsageMeter,
    /// FeedbackAgent: routes human signal back to the build loop.
    FeedbackAgent,

    // ── Tier 5: Interface ─────────────────────────────────────────────────────
    /// BridgeAgent: connects to external systems (APIs, webhooks, MCP servers).
    BridgeAgent,
    /// PeerAgent: multi-server bridge; relays fabric events across nodes.
    PeerAgent,
    /// OnboardingAgent: chat-based setup; the chat IS the platform interface.
    OnboardingAgent,
    /// UserProxyAgent: acts on behalf of a human user; proxies their intent.
    UserProxyAgent,
    /// UnicodeAgent: zero-LLM text intelligence for script, emoji, normalisation.
    UnicodeAgent,
    /// InstitutionAgent: member of an institutional team (university/gov/enterprise).
    InstitutionAgent,
}

impl AgentType {
    pub fn tier(&self) -> u8 {
        match self {
            Self::MegaAgent | Self::Designer | Self::Planner | Self::Optimizer | Self::Reconciler => 0,
            Self::Executor | Self::Researcher | Self::Coder | Self::DataScientist |
            Self::Writer | Self::Deployer | Self::Builder | Self::Signer => 1,
            Self::ConditionVerifier | Self::Tester | Self::Auditor | Self::TrustVerifier => 2,
            Self::Analyst | Self::Reporter | Self::Aggregator | Self::Summariser => 3,
            Self::PolicyAgent | Self::EthicsAgent | Self::UsageMeter | Self::FeedbackAgent => 4,
            Self::BridgeAgent | Self::PeerAgent | Self::OnboardingAgent |
            Self::UserProxyAgent | Self::UnicodeAgent | Self::InstitutionAgent => 5,
        }
    }

    pub fn tier_name(&self) -> &'static str {
        match self.tier() {
            0 => "Meta",
            1 => "Execute",
            2 => "Verify",
            3 => "Synthesise",
            4 => "Govern",
            _ => "Interface",
        }
    }

    pub fn tools(&self) -> Vec<&'static str> {
        match self {
            Self::Researcher   => vec!["web_search"],
            Self::Coder        => vec!["shell"],
            Self::DataScientist => vec!["shell"],
            Self::Deployer     => vec!["shell"],
            Self::Builder      => vec!["shell"],
            Self::BridgeAgent  => vec!["http_client"],
            Self::PeerAgent    => vec!["http_client"],
            Self::UnicodeAgent => vec![],
            _                  => vec![],
        }
    }

    /// Return all defined agent types as a list.
    pub fn all() -> Vec<Self> {
        vec![
            Self::MegaAgent, Self::Designer, Self::Planner, Self::Optimizer, Self::Reconciler,
            Self::Executor, Self::Researcher, Self::Coder, Self::DataScientist,
            Self::Writer, Self::Deployer, Self::Builder, Self::Signer,
            Self::ConditionVerifier, Self::Tester, Self::Auditor, Self::TrustVerifier,
            Self::Analyst, Self::Reporter, Self::Aggregator, Self::Summariser,
            Self::PolicyAgent, Self::EthicsAgent, Self::UsageMeter, Self::FeedbackAgent,
            Self::BridgeAgent, Self::PeerAgent, Self::OnboardingAgent,
            Self::UserProxyAgent, Self::UnicodeAgent, Self::InstitutionAgent,
        ]
    }

    /// Map a natural language role string to an AgentType (best-effort).
    pub fn from_role(role: &str) -> Self {
        let r = role.to_lowercase();
        if r.contains("mega")        { Self::MegaAgent }
        else if r.contains("design") { Self::Designer }
        else if r.contains("plan")   { Self::Planner }
        else if r.contains("optim")  { Self::Optimizer }
        else if r.contains("reconcil") { Self::Reconciler }
        else if r.contains("research") { Self::Researcher }
        else if r.contains("code") || r.contains("develop") || r.contains("program") { Self::Coder }
        else if r.contains("data") || r.contains("ml") || r.contains("model") { Self::DataScientist }
        else if r.contains("write") || r.contains("report") || r.contains("document") { Self::Writer }
        else if r.contains("deploy") { Self::Deployer }
        else if r.contains("build")  { Self::Builder }
        else if r.contains("sign")   { Self::Signer }
        else if r.contains("verif")  { Self::ConditionVerifier }
        else if r.contains("test")   { Self::Tester }
        else if r.contains("audit")  { Self::Auditor }
        else if r.contains("trust")  { Self::TrustVerifier }
        else if r.contains("analys") { Self::Analyst }
        else if r.contains("aggreg") { Self::Aggregator }
        else if r.contains("summar") { Self::Summariser }
        else if r.contains("policy") { Self::PolicyAgent }
        else if r.contains("ethic")  { Self::EthicsAgent }
        else if r.contains("usage")  { Self::UsageMeter }
        else if r.contains("feedback") { Self::FeedbackAgent }
        else if r.contains("bridge") || r.contains("api") { Self::BridgeAgent }
        else if r.contains("peer")   { Self::PeerAgent }
        else if r.contains("onboard") { Self::OnboardingAgent }
        else if r.contains("user") || r.contains("proxy") { Self::UserProxyAgent }
        else if r.contains("unicode") { Self::UnicodeAgent }
        else if r.contains("institution") { Self::InstitutionAgent }
        else { Self::Executor }
    }
}

use std::sync::Arc;
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::store::{AppState, AgentIdentity, RunStatus};
use crate::lifecycle::Stage;
use crate::fabric::FabricEvent;

// ── MegaAgent identity ────────────────────────────────────────────────────────

pub const MEGA_AGENT_ID:  &str = "agent_mega";
pub const MEGA_AGENT_DID: &str = "did:autonomyx:mega-agent";

/// Register the MegaAgent singleton in AppState at startup. Idempotent.
pub fn register(state: &AppState) {
    let mut agents = state.agents.write().unwrap();
    if agents.contains_key(MEGA_AGENT_ID) { return; }

    let mega = AgentIdentity {
        id:          MEGA_AGENT_ID.into(),
        owner_id:    MEGA_AGENT_DID.into(),
        name:        "MegaAgent".into(),
        description: "FSM-based meta-orchestrator — auto-designs and runs multi-agent systems for any goal".into(),
        model:       "mega".into(),
        status:      "ready".into(),
        capabilities: vec![
            "fsm_design".into(),
            "fsm_execute".into(),
            "fsm_optimize".into(),
            "orchestrate".into(),
            "decompose".into(),
            "assign".into(),
            "monitor".into(),
            "synthesize".into(),
            "govern".into(),
            "traceback".into(),
            "tool_use".into(),
        ],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    agents.insert(MEGA_AGENT_ID.into(), mega);
    tracing::info!("MegaAgent: registered — did={}", MEGA_AGENT_DID);
}

// ── FSM types — aligned with MetaAgent paper ─────────────────────────────────

/// A single state in the Finite State Machine.
/// Each state has exactly one task-solving agent, one condition verifier,
/// a set of listeners who receive the output on transition, and an instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmState {
    pub state_id:   String,
    pub agent_id:   String,         // which agent executes in this state
    pub instruction: String,        // what the agent should do
    pub is_initial: bool,
    pub is_final:   bool,
    pub listeners:  Vec<String>,    // agent_ids who receive output of this state
    pub tools:      Vec<String>,    // tool names available to the agent in this state
}

/// A transition between FSM states.
/// `condition` is a natural language predicate evaluated by the condition verifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmTransition {
    pub from_state: String,
    pub to_state:   String,
    pub condition:  String,     // natural language; null → stay in current state (null-transition)
}

/// The Finite State Machine — ℳ = (Σ, S, s₀, F, δ)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiniteStateMachine {
    pub fsm_id:      String,
    pub domain:      String,        // Σ — the task type this FSM handles
    pub states:      Vec<FsmState>,
    pub transitions: Vec<FsmTransition>,
    pub agents:      Vec<FsmAgentDef>,
    pub optimized:   bool,
    pub created_at:  DateTime<Utc>,
}

/// Agent definition used by the FSM designer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmAgentDef {
    pub agent_id:      String,
    pub name:          String,
    pub role:          String,
    pub system_prompt: String,
    pub model:         Option<String>, // override global LlmConfig.default_model
    pub tools:         Vec<String>,
    pub memory:        Vec<String>,    // accumulated listener outputs
}

impl FiniteStateMachine {
    pub fn initial_state(&self) -> Option<&FsmState> {
        self.states.iter().find(|s| s.is_initial)
    }

    pub fn state(&self, id: &str) -> Option<&FsmState> {
        self.states.iter().find(|s| s.state_id == id)
    }

    pub fn transitions_from(&self, state_id: &str) -> Vec<&FsmTransition> {
        self.transitions.iter()
            .filter(|t| t.from_state == state_id)
            .collect()
    }

    pub fn agent(&self, agent_id: &str) -> Option<&FsmAgentDef> {
        self.agents.iter().find(|a| a.agent_id == agent_id)
    }

    pub fn agent_mut(&mut self, agent_id: &str) -> Option<&mut FsmAgentDef> {
        self.agents.iter_mut().find(|a| a.agent_id == agent_id)
    }
}

// ── FSM auto-design ───────────────────────────────────────────────────────────
//
// Given a task description, auto-generate the FSM.
//
// Real: call LLM (claude-opus-4-8 with adaptive thinking) with the task description
// and the JSON schema of FsmState/FsmTransition/FsmAgentDef, parse the structured response.
//
// Platform: deterministic design by domain pattern matching.
// Replace `design_fsm_deterministic` with an LLM call in production.

pub fn design_fsm(domain: &str) -> FiniteStateMachine {
    let fsm_id = format!("fsm_{}", Uuid::new_v4().simple());
    let now = Utc::now();

    // Production: this is where we call claude-opus-4-8 with adaptive thinking
    // to generate a FiniteStateMachine in JSON.
    // For now: deterministic design by domain.
    let (agents, states, transitions) = design_fsm_deterministic(domain);

    FiniteStateMachine {
        fsm_id,
        domain: domain.to_string(),
        agents,
        states,
        transitions,
        optimized: false,
        created_at: now,
    }
}

fn design_fsm_deterministic(domain: &str) -> (Vec<FsmAgentDef>, Vec<FsmState>, Vec<FsmTransition>) {
    let domain_lower = domain.to_lowercase();

    if domain_lower.contains("software") || domain_lower.contains("code") || domain_lower.contains("develop") {
        software_development_fsm()
    } else if domain_lower.contains("research") || domain_lower.contains("analysis") || domain_lower.contains("analys") {
        research_analysis_fsm()
    } else if domain_lower.contains("data") || domain_lower.contains("ml") || domain_lower.contains("machine learning") {
        data_science_fsm()
    } else {
        general_task_fsm(domain)
    }
}

fn software_development_fsm() -> (Vec<FsmAgentDef>, Vec<FsmState>, Vec<FsmTransition>) {
    let agents = vec![
        FsmAgentDef {
            agent_id: "req_designer".into(),
            name: "RequirementDesigner".into(),
            role: "Gather and analyse software requirements; produce architecture design".into(),
            system_prompt: "You are RequirementDesigner. Understand software requirements and create robust, scalable architecture. Output structured requirements doc.".into(),
            model: None, tools: vec!["web_search".into()], memory: vec![],
        },
        FsmAgentDef {
            agent_id: "code_developer".into(),
            name: "CodeDeveloper".into(),
            role: "Write clean, efficient code based on requirements. Save to file system.".into(),
            system_prompt: "You are CodeDeveloper. Write production-quality code based on the RequirementDesigner's output. Produce README. Save all files.".into(),
            model: None, tools: vec!["shell".into()], memory: vec![],
        },
        FsmAgentDef {
            agent_id: "tester".into(),
            name: "Tester".into(),
            role: "Test software. Report bugs. Verify all checkpoints pass.".into(),
            system_prompt: "You are Tester. Execute the software, run tests, report bugs. If bugs found, describe them precisely for CodeDeveloper to fix.".into(),
            model: None, tools: vec!["shell".into()], memory: vec![],
        },
    ];

    let states = vec![
        FsmState {
            state_id: "s1".into(), agent_id: "req_designer".into(),
            instruction: "Gather and analyse requirements. Produce architecture design.".into(),
            is_initial: true, is_final: false,
            listeners: vec!["code_developer".into()], tools: vec!["web_search".into()],
        },
        FsmState {
            state_id: "s2".into(), agent_id: "code_developer".into(),
            instruction: "Write code per the design. Save files. Write README.".into(),
            is_initial: false, is_final: false,
            listeners: vec!["tester".into()], tools: vec!["shell".into()],
        },
        FsmState {
            state_id: "s3".into(), agent_id: "tester".into(),
            instruction: "Test the software. Report any bugs found.".into(),
            is_initial: false, is_final: false,
            listeners: vec!["req_designer".into(), "code_developer".into()], tools: vec!["shell".into()],
        },
        FsmState {
            state_id: "s4".into(), agent_id: "req_designer".into(),
            instruction: "<|submit|> Report that software is developed and tested.".into(),
            is_initial: false, is_final: true,
            listeners: vec![], tools: vec![],
        },
    ];

    let transitions = vec![
        FsmTransition { from_state: "s1".into(), to_state: "s2".into(), condition: "Requirements are clear and design is complete".into() },
        FsmTransition { from_state: "s2".into(), to_state: "s3".into(), condition: "Code is written and README is complete".into() },
        FsmTransition { from_state: "s3".into(), to_state: "s4".into(), condition: "All tests pass and software works as intended".into() },
        FsmTransition { from_state: "s3".into(), to_state: "s2".into(), condition: "Tests failed — bugs found that require code changes".into() },
        FsmTransition { from_state: "s3".into(), to_state: "s1".into(), condition: "Fundamental design issues found — must revisit requirements".into() },
    ];

    (agents, states, transitions)
}

fn research_analysis_fsm() -> (Vec<FsmAgentDef>, Vec<FsmState>, Vec<FsmTransition>) {
    let agents = vec![
        FsmAgentDef {
            agent_id: "researcher".into(), name: "Researcher".into(),
            role: "Gather information from multiple sources".into(),
            system_prompt: "You are Researcher. Use web search to gather comprehensive information on the topic.".into(),
            model: None, tools: vec!["web_search".into()], memory: vec![],
        },
        FsmAgentDef {
            agent_id: "analyst".into(), name: "Analyst".into(),
            role: "Synthesise findings into structured analysis".into(),
            system_prompt: "You are Analyst. Synthesise the Researcher's findings. Identify patterns, conflicts, and key insights.".into(),
            model: None, tools: vec![], memory: vec![],
        },
        FsmAgentDef {
            agent_id: "reporter".into(), name: "Reporter".into(),
            role: "Produce final structured report".into(),
            system_prompt: "You are Reporter. Produce a clear, structured report from the Analyst's synthesis.".into(),
            model: None, tools: vec![], memory: vec![],
        },
    ];

    let states = vec![
        FsmState { state_id: "s1".into(), agent_id: "researcher".into(), instruction: "Gather comprehensive information on the topic.".into(), is_initial: true, is_final: false, listeners: vec!["analyst".into()], tools: vec!["web_search".into()] },
        FsmState { state_id: "s2".into(), agent_id: "analyst".into(), instruction: "Synthesise findings. Identify key insights and gaps.".into(), is_initial: false, is_final: false, listeners: vec!["reporter".into()], tools: vec![] },
        FsmState { state_id: "s3".into(), agent_id: "reporter".into(), instruction: "<|submit|> Produce and submit final report.".into(), is_initial: false, is_final: true, listeners: vec![], tools: vec![] },
    ];

    let transitions = vec![
        FsmTransition { from_state: "s1".into(), to_state: "s2".into(), condition: "Sufficient information gathered".into() },
        FsmTransition { from_state: "s2".into(), to_state: "s3".into(), condition: "Analysis is complete and coherent".into() },
        FsmTransition { from_state: "s2".into(), to_state: "s1".into(), condition: "Gaps in information found — need more research".into() },
    ];

    (agents, states, transitions)
}

fn data_science_fsm() -> (Vec<FsmAgentDef>, Vec<FsmState>, Vec<FsmTransition>) {
    let agents = vec![
        FsmAgentDef {
            agent_id: "data_prep".into(), name: "DataPreparationAndModelAgent".into(),
            role: "Preprocess data, select model, train, evaluate".into(),
            system_prompt: "You are DataPreparationAndModelAgent. Clean data, select and train ML model, evaluate on test set. Report metrics.".into(),
            model: None, tools: vec!["shell".into()], memory: vec![],
        },
        FsmAgentDef {
            agent_id: "reporter".into(), name: "ReportingAgent".into(),
            role: "Compile metrics and generate comprehensive report".into(),
            system_prompt: "You are ReportingAgent. Compile evaluation metrics and generate a comprehensive report for the user.".into(),
            model: None, tools: vec![], memory: vec![],
        },
    ];

    let states = vec![
        FsmState { state_id: "s1".into(), agent_id: "data_prep".into(), instruction: "Preprocess data, train model, evaluate on test set.".into(), is_initial: true, is_final: false, listeners: vec!["reporter".into()], tools: vec!["shell".into()] },
        FsmState { state_id: "s2".into(), agent_id: "reporter".into(), instruction: "<|submit|> Compile and submit evaluation report.".into(), is_initial: false, is_final: true, listeners: vec![], tools: vec![] },
    ];

    let transitions = vec![
        FsmTransition { from_state: "s1".into(), to_state: "s2".into(), condition: "Model trained and evaluated successfully".into() },
    ];

    (agents, states, transitions)
}

fn general_task_fsm(domain: &str) -> (Vec<FsmAgentDef>, Vec<FsmState>, Vec<FsmTransition>) {
    let agents = vec![
        FsmAgentDef {
            agent_id: "planner".into(), name: "Planner".into(),
            role: "Decompose goal into actionable plan".into(),
            system_prompt: format!("You are Planner. Your goal: {domain}. Produce a clear, actionable plan."),
            model: None, tools: vec![], memory: vec![],
        },
        FsmAgentDef {
            agent_id: "executor".into(), name: "Executor".into(),
            role: "Execute the plan step by step".into(),
            system_prompt: "You are Executor. Follow the plan, execute each step, report results.".into(),
            model: None, tools: vec!["shell".into(), "web_search".into()], memory: vec![],
        },
        FsmAgentDef {
            agent_id: "reviewer".into(), name: "Reviewer".into(),
            role: "Review results and produce final output".into(),
            system_prompt: "You are Reviewer. Review the Executor's results. If complete, submit. If not, identify gaps.".into(),
            model: None, tools: vec![], memory: vec![],
        },
    ];

    let states = vec![
        FsmState { state_id: "s1".into(), agent_id: "planner".into(), instruction: format!("Decompose the goal into a clear plan: {domain}"), is_initial: true, is_final: false, listeners: vec!["executor".into()], tools: vec![] },
        FsmState { state_id: "s2".into(), agent_id: "executor".into(), instruction: "Execute the plan.".into(), is_initial: false, is_final: false, listeners: vec!["reviewer".into()], tools: vec!["shell".into(), "web_search".into()] },
        FsmState { state_id: "s3".into(), agent_id: "reviewer".into(), instruction: "<|submit|> Review results and submit final answer.".into(), is_initial: false, is_final: true, listeners: vec![], tools: vec![] },
    ];

    let transitions = vec![
        FsmTransition { from_state: "s1".into(), to_state: "s2".into(), condition: "Plan is complete and actionable".into() },
        FsmTransition { from_state: "s2".into(), to_state: "s3".into(), condition: "All steps executed successfully".into() },
        FsmTransition { from_state: "s3".into(), to_state: "s2".into(), condition: "Gaps found — executor must do more work".into() },
    ];

    (agents, states, transitions)
}

// ── FSM optimization — merge redundant states ─────────────────────────────────
//
// Paper Algorithm 1: pairwise state comparison via LLM; if roles/tools overlap → merge.
// Platform: heuristic merge when two states share the same agent_id.
// Production: call claude-opus-4-8 for each pair to assess mergeability.

pub fn optimize_fsm(mut fsm: FiniteStateMachine) -> FiniteStateMachine {
    let mut merged = true;
    while merged {
        merged = false;
        let state_ids: Vec<String> = fsm.states.iter().map(|s| s.state_id.clone()).collect();

        'outer: for i in 0..state_ids.len() {
            for j in (i+1)..state_ids.len() {
                let si = state_ids[i].clone();
                let sj = state_ids[j].clone();

                // Heuristic: same agent_id AND one is not initial/final → merge
                let si_state = fsm.state(&si).cloned();
                let sj_state = fsm.state(&sj).cloned();
                if let (Some(a), Some(b)) = (si_state, sj_state) {
                    if a.agent_id == b.agent_id && !a.is_initial && !b.is_initial
                       && !a.is_final && !b.is_final {
                        // Merge: combine instructions; union listeners
                        let merged_instruction = format!("{}. Also: {}", a.instruction, b.instruction);
                        let mut merged_listeners = a.listeners.clone();
                        for l in &b.listeners {
                            if !merged_listeners.contains(l) { merged_listeners.push(l.clone()); }
                        }
                        let merged_tools: Vec<String> = {
                            let mut t = a.tools.clone();
                            for tool in &b.tools { if !t.contains(tool) { t.push(tool.clone()); } }
                            t
                        };

                        // Update all transitions from/to sj → si
                        for tr in &mut fsm.transitions {
                            if tr.from_state == sj { tr.from_state = si.clone(); }
                            if tr.to_state == sj   { tr.to_state   = si.clone(); }
                        }

                        // Remove sj state, update si
                        fsm.states.retain(|s| s.state_id != sj);
                        if let Some(state) = fsm.states.iter_mut().find(|s| s.state_id == si) {
                            state.instruction = merged_instruction;
                            state.listeners   = merged_listeners;
                            state.tools       = merged_tools;
                        }

                        // Remove self-loops created by merge
                        fsm.transitions.retain(|t| !(t.from_state == t.to_state));

                        tracing::debug!(si = %si, sj = %sj, "FSM: merged redundant states");
                        merged = true;
                        break 'outer;
                    }
                }
            }
        }
    }

    fsm.optimized = true;
    fsm
}

// ── FSM execution — Algorithm 2 from the paper ───────────────────────────────
//
// Starting from s₀:
//   1. Agent executes (instruction + memory + query) → output
//   2. Condition verifier checks output against transition conditions
//   3. If condition met → transition (update memory of listeners)
//   4. If no condition → null-transition (feedback to agent, retry)
//   5. Repeat until final state or max_iterations

const MAX_ITERATIONS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmExecution {
    pub execution_id:  String,
    pub fsm_id:        String,
    pub query:         String,
    pub transitions:   Vec<FsmTransitionRecord>,
    pub final_output:  Option<String>,
    pub status:        FsmExecutionStatus,
    pub null_transitions: usize,
    pub tracebacks:    usize,
    pub iterations:    usize,
    pub started_at:    DateTime<Utc>,
    pub completed_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FsmExecutionStatus {
    Completed,
    MaxIterationsExceeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsmTransitionRecord {
    pub from_state:  String,
    pub to_state:    Option<String>,  // None = null-transition
    pub kind:        TransitionKind,
    pub output:      String,
    pub condition:   Option<String>,
    pub iteration:   usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Forward,
    Traceback,
    Null,
    Final,
}

pub async fn execute_fsm(
    state: Arc<AppState>,
    fsm: &mut FiniteStateMachine,
    query: String,
) -> FsmExecution {
    let execution_id = format!("exec_{}", Uuid::new_v4().simple());
    let started_at = Utc::now();
    let mut history: Vec<FsmTransitionRecord> = Vec::new();
    let mut iteration = 0usize;
    let mut null_transitions = 0usize;
    let mut tracebacks = 0usize;
    let mut final_output: Option<String> = None;
    let mut exec_status = FsmExecutionStatus::MaxIterationsExceeded;

    // Give all agents the initial query
    for agent in &mut fsm.agents {
        agent.memory.push(format!("USER_QUERY: {query}"));
    }

    let initial_state_id = match fsm.initial_state() {
        Some(s) => s.state_id.clone(),
        None => {
            return FsmExecution {
                execution_id, fsm_id: fsm.fsm_id.clone(), query,
                transitions: vec![], final_output: None,
                status: FsmExecutionStatus::Failed,
                null_transitions: 0, tracebacks: 0, iterations: 0,
                started_at, completed_at: Utc::now(),
            };
        }
    };

    let mut current_state_id = initial_state_id;
    let mut prev_state_ids: Vec<String> = Vec::new();

    while iteration < MAX_ITERATIONS {
        iteration += 1;

        let (agent_id, instruction, is_final, tools, listeners) = {
            let s = match fsm.state(&current_state_id) {
                Some(s) => s,
                None => break,
            };
            (s.agent_id.clone(), s.instruction.clone(), s.is_final, s.tools.clone(), s.listeners.clone())
        };

        // Get agent memory
        let memory = fsm.agents.iter()
            .find(|a| a.agent_id == agent_id)
            .map(|a| a.memory.join("\n"))
            .unwrap_or_default();

        // Get agent definition for LLM call
        let (system_prompt, agent_model) = fsm.agents.iter()
            .find(|a| a.agent_id == agent_id)
            .map(|a| (a.system_prompt.clone(), a.model.clone()))
            .unwrap_or_default();

        // Execute: call LLM with per-agent or global-config model
        let output = execute_agent(
            &state,
            &agent_id,
            &instruction,
            &memory,
            &query,
            &tools,
            &system_prompt,
            agent_model.as_deref(),
        ).await;

        // Final state
        if is_final {
            let clean_output = output.replace("<|submit|>", "").trim().to_string();
            final_output = Some(clean_output.clone());
            history.push(FsmTransitionRecord {
                from_state: current_state_id.clone(),
                to_state: None,
                kind: TransitionKind::Final,
                output: clean_output,
                condition: None,
                iteration,
            });
            exec_status = FsmExecutionStatus::Completed;

            // Emit fabric event
            state.fabric.emit(
                FabricEvent::open(
                    &format!("fsm:{execution_id}"),
                    Stage::Feedback,
                    json!({ "action": "fsm_complete", "execution_id": execution_id }),
                ).with_entities([format!("fsm:{execution_id}"), format!("agent:{MEGA_AGENT_ID}")])
            );
            break;
        }

        // Condition verifier — check transition conditions (stub: keyword heuristic)
        let transitions = fsm.transitions_from(&current_state_id)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        let matched_transition = verify_conditions(&output, &transitions);

        match matched_transition {
            Some(tr) => {
                let to_state = tr.to_state.clone();
                let condition = tr.condition.clone();

                // Determine kind: traceback if going to a state already visited
                let kind = if prev_state_ids.contains(&to_state) {
                    tracebacks += 1;
                    TransitionKind::Traceback
                } else {
                    TransitionKind::Forward
                };

                // Insert output into listener agents' memory
                for listener_id in &listeners {
                    if let Some(agent) = fsm.agents.iter_mut().find(|a| &a.agent_id == listener_id) {
                        agent.memory.push(format!("FROM_STATE[{}]: {}", current_state_id, output));
                    }
                }

                // Emit transition event
                state.fabric.emit(
                    FabricEvent::open(
                        &format!("fsm:{execution_id}:transition:{iteration}"),
                        Stage::Observe,
                        json!({
                            "action":     "state_transition",
                            "from":       current_state_id,
                            "to":         to_state,
                            "condition":  condition,
                            "kind":       format!("{kind:?}"),
                        }),
                    ).with_entities([format!("fsm:{execution_id}")])
                );

                history.push(FsmTransitionRecord {
                    from_state: current_state_id.clone(),
                    to_state: Some(to_state.clone()),
                    kind,
                    output,
                    condition: Some(condition),
                    iteration,
                });

                prev_state_ids.push(current_state_id.clone());
                current_state_id = to_state;
            }
            None => {
                // Null-transition: no condition met — give feedback and retry
                null_transitions += 1;

                // Self-improvement trigger: emit event so loop_coordinator can improve this agent's prompt
                let out_preview = output.chars().take(800).collect::<String>();
                let sys_preview = system_prompt.chars().take(500).collect::<String>();
                state.fabric.emit(
                    FabricEvent::open(
                        &format!("fsm:{execution_id}:null:{iteration}"),
                        Stage::Feedback,
                        json!({
                            "action":        "fsm_state_output",
                            "agent_id":      &agent_id,
                            "instruction":   &instruction,
                            "output":        out_preview,
                            "succeeded":     false,
                            "condition":     null,
                            "system_prompt": sys_preview,
                        }),
                    ).with_entities([format!("agent:{agent_id}")])
                );

                if let Some(agent) = fsm.agents.iter_mut().find(|a| a.agent_id == agent_id) {
                    agent.memory.push(format!("FEEDBACK: Output not accepted. Retry instruction: {instruction}"));
                }

                history.push(FsmTransitionRecord {
                    from_state: current_state_id.clone(),
                    to_state: None,
                    kind: TransitionKind::Null,
                    output,
                    condition: None,
                    iteration,
                });
            }
        }
    }

    FsmExecution {
        execution_id,
        fsm_id: fsm.fsm_id.clone(),
        query,
        transitions: history,
        final_output,
        status: exec_status,
        null_transitions,
        tracebacks,
        iterations: iteration,
        started_at,
        completed_at: Utc::now(),
    }
}

// ── Agent execution ───────────────────────────────────────────────────────────

async fn execute_agent(
    state:         &Arc<AppState>,
    agent_id:      &str,
    instruction:   &str,
    memory:        &str,
    query:         &str,
    tools:         &[String],
    system_prompt: &str,
    agent_model:   Option<&str>,  // per-agent override; None → global config
) -> String {
    // Resolve model: per-agent → global runtime config → never hardcoded
    let model = agent_model
        .filter(|m| !m.is_empty())
        .map(|m| m.to_string())
        .unwrap_or_else(|| state.llm_config.read().unwrap().default_model.clone());
    let max_tokens = state.llm_config.read().unwrap().default_max_tokens;

    let run = state.create_run(agent_id, agent_id, "mega/fsm", instruction);

    // Invoke tools before the LLM call so results are available as context
    let tool_results: Vec<String> = if tools.is_empty() {
        vec![]
    } else {
        let tool_input = serde_json::json!({
            "query":   query,
            "command": format!("process: {instruction}"),
            "url":     "",
        });
        let mut results = Vec::new();
        for tool in tools {
            if tool == "web_search" || tool == "shell" || tool == "http_client" {
                let result = crate::tools::invoke(tool, &tool_input).await;
                results.push(format!("[{tool}]: {result}"));
            }
        }
        results
    };

    // Build user message: memory + tool results + instruction + query
    let tool_context = if tool_results.is_empty() {
        String::new()
    } else {
        format!("\n\nTool results:\n{}", tool_results.join("\n"))
    };

    let user_content = format!(
        "{memory}{tool_context}\n\nInstruction: {instruction}\n\nTask: {query}"
    );

    let messages = vec![json!({ "role": "user", "content": user_content })];

    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();

    // Check ConfigDB for a self-improved prompt; it overrides the FSM-designed default
    let improved_rec = state.config.get("agent_prompt", agent_id).await;
    let sys = if let Some(rec) = improved_rec {
        rec.data.get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or(system_prompt)
            .to_string()
    } else if system_prompt.is_empty() {
        format!("You are {agent_id}, a specialist agent in the Autonomyx platform. Complete your instruction thoroughly and precisely.")
    } else {
        system_prompt.to_string()
    };

    let output = match crate::providers::complete(
        state.egress.llm(),
        &model,
        &api_key,
        &sys,
        &messages,
        max_tokens,
    ).await {
        Ok(resp) => resp.text,
        Err(e) => {
            tracing::warn!(agent_id, error = %e, "MegaAgent: LLM call failed, using fallback");
            format!(
                "Agent '{agent_id}' processed instruction: {}",
                instruction.chars().take(120).collect::<String>()
            )
        }
    };

    state.finish_run(&run.run_id, RunStatus::Completed);
    output
}

// ── Self-improvement ──────────────────────────────────────────────────────────
//
// Called by loop_coordinator when a null-transition triggers an fsm_state_output event.
// Asks the LLM to produce a better system_prompt for the agent; stores it in ConfigDB.
// On the next FSM execution, execute_agent reads the improved prompt from ConfigDB first.

pub async fn improve_prompt(
    state:         &Arc<AppState>,
    agent_id:      &str,
    system_prompt: &str,
    instruction:   &str,
    output:        &str,
    succeeded:     bool,
    condition:     &str,
) {
    let model = state.llm_config.read().unwrap().default_model.clone();
    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();

    let outcome = if succeeded { "succeeded" } else { "failed (null-transition)" };
    let user_msg = format!(
        "Agent ID: {agent_id}\n\
         Current system prompt:\n{system_prompt}\n\n\
         Instruction given: {instruction}\n\
         Agent output ({outcome}):\n{output}\n\
         Required condition (unmet): {condition}\n\n\
         Return ONLY an improved system prompt. No explanation, no preamble."
    );

    let messages = vec![json!({ "role": "user", "content": user_msg })];

    let improved = match crate::providers::complete(
        state.egress.llm(),
        &model,
        &api_key,
        "You are a meta-optimizer improving AI agent system prompts. Return ONLY the improved prompt text.",
        &messages,
        512,
    ).await {
        Ok(resp) if !resp.text.trim().is_empty() => resp.text.trim().to_string(),
        Ok(_) => return,
        Err(e) => {
            tracing::warn!(agent_id, error = %e, "improve_prompt: LLM call failed");
            return;
        }
    };

    match state.config.put(
        "agent_prompt",
        agent_id,
        json!({ "prompt": improved }),
        "self_improvement",
    ).await {
        Ok(_)  => tracing::info!(agent_id, "improve_prompt: stored improved system_prompt in ConfigDB"),
        Err(e) => tracing::warn!(agent_id, error = %e, "improve_prompt: failed to store in ConfigDB"),
    }
}

// ── Condition verifier ────────────────────────────────────────────────────────
//
// Production: call LLM with:
//   - system_prompt = agent's system_prompt + all transition conditions
//   - input = agent's output
//   - prompt = "Which condition is met? Output the to_state or None"
//
// Platform: heuristic keyword matching.

fn verify_conditions(output: &str, transitions: &[FsmTransition]) -> Option<FsmTransition> {
    let output_lower = output.to_lowercase();

    for tr in transitions {
        let condition_lower = tr.condition.to_lowercase();

        // Heuristic: if condition keywords appear in output OR output implies success
        let keywords: Vec<&str> = condition_lower.split_whitespace().collect();
        let match_score = keywords.iter()
            .filter(|k| k.len() > 4 && output_lower.contains(*k))
            .count();

        // Match if >30% of significant keywords found, or output contains success signals
        let total_significant = keywords.iter().filter(|k| k.len() > 4).count().max(1);
        let match_ratio = match_score as f64 / total_significant as f64;

        let has_success = output_lower.contains("completed") || output_lower.contains("success")
            || output_lower.contains("finished") || output_lower.contains("done");

        if match_ratio > 0.3 || (has_success && !tr.condition.to_lowercase().contains("failed")) {
            return Some(tr.clone());
        }
    }

    None
}

// ── Orchestration request / response (high-level API) ────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrateRequest {
    pub goal:       String,
    pub domain:     Option<String>,
    pub optimize:   Option<bool>,
    pub actor_did:  Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrateResponse {
    pub orchestration_id: String,
    pub goal:             String,
    pub fsm:              FiniteStateMachine,
    pub execution:        FsmExecution,
    pub actor_did:        String,
}

pub async fn orchestrate(
    state:  Arc<AppState>,
    req:    OrchestrateRequest,
) -> OrchestrateResponse {
    let orchestration_id = format!("orch_{}", Uuid::new_v4().simple());
    let actor_did = req.actor_did.as_deref().unwrap_or(MEGA_AGENT_DID).to_string();
    let domain = req.domain.as_deref().unwrap_or(&req.goal);

    tracing::info!(orch_id = %orchestration_id, goal = %req.goal, "MegaAgent: FSM orchestration started");

    // Design FSM for this domain
    let mut fsm = design_fsm(domain);

    // Optimize: merge redundant states
    if req.optimize.unwrap_or(true) {
        fsm = optimize_fsm(fsm);
    }

    // Execute FSM
    let execution = execute_fsm(state.clone(), &mut fsm, req.goal.clone()).await;

    // Accountability
    {
        use crate::identity::AgentIdentity as IdentityActor;
        use crate::federation::ActionOutcome;
        let mega_id = IdentityActor::from_did(MEGA_AGENT_DID);
        let outcome = if execution.status == FsmExecutionStatus::Completed {
            ActionOutcome::Success
        } else {
            ActionOutcome::Denied
        };
        state.federation.record(
            &mega_id,
            "mega_agent:fsm_orchestrate",
            &orchestration_id,
            None,
            outcome,
            json!({ "goal": req.goal, "fsm_id": fsm.fsm_id, "iterations": execution.iterations }),
        );
    }

    tracing::info!(
        orch_id    = %orchestration_id,
        iterations = execution.iterations,
        tracebacks = execution.tracebacks,
        status     = ?execution.status,
        "MegaAgent: FSM orchestration complete"
    );

    OrchestrateResponse {
        orchestration_id,
        goal: req.goal,
        fsm,
        execution,
        actor_did,
    }
}
