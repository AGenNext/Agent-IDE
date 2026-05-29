// Agent Workspace OS — Core Domain Types

export type AgentStatus = 'idle' | 'running' | 'paused' | 'error' | 'completed';
export type TaskStatus = 'pending' | 'in_progress' | 'completed' | 'failed' | 'cancelled';
export type ArtifactType =
    | 'file' | 'code' | 'document' | 'data' | 'model' | 'report'
    | 'app' | 'agent' | 'llm' | 'api' | 'package' | 'json';
export type GovernanceAction = 'allow' | 'deny' | 'require_approval' | 'audit_only';
export type TraceStepType = 'thought' | 'action' | 'observation' | 'result' | 'error';
export type EdgeType = 'produces' | 'consumes' | 'triggers' | 'governs' | 'references';
export type NodeType = 'agent' | 'task' | 'artifact' | 'knowledge' | 'tool';

export interface Agent {
    id: string;
    name: string;
    description: string;
    version: string;
    skills: Skill[];
    tools: Tool[];
    status: AgentStatus;
    createdAt: string;
    updatedAt: string;
    metadata: Record<string, unknown>;
}

export interface Skill {
    id: string;
    name: string;
    description: string;
    agentId: string;
    inputSchema: Record<string, unknown>;
    outputSchema: Record<string, unknown>;
}

export interface Tool {
    id: string;
    name: string;
    description: string;
    inputSchema: Record<string, unknown>;
    outputSchema: Record<string, unknown>;
    mcpServerId?: string;
}

export interface Task {
    id: string;
    title: string;
    description: string;
    status: TaskStatus;
    assignedAgentId?: string;
    parentTaskId?: string;
    subtaskIds: string[];
    artifactIds: string[];
    createdAt: string;
    updatedAt: string;
    completedAt?: string;
    metadata: Record<string, unknown>;
}

export interface Artifact {
    id: string;
    name: string;
    type: ArtifactType;
    content?: string;
    contentRef?: string;
    taskId: string;
    agentId: string;
    createdAt: string;
    metadata: Record<string, unknown>;
}

export interface AgentRun {
    id: string;
    agentId: string;
    taskId: string;
    status: TaskStatus;
    startedAt: string;
    completedAt?: string;
    trace: TraceStep[];
    artifactIds: string[];
    errorMessage?: string;
    metadata: Record<string, unknown>;
}

export interface TraceStep {
    id: string;
    runId: string;
    sequence: number;
    type: TraceStepType;
    content: string;
    toolName?: string;
    toolInput?: Record<string, unknown>;
    toolOutput?: unknown;
    timestamp: string;
    durationMs?: number;
}

export interface GovernancePolicy {
    id: string;
    name: string;
    description: string;
    scope: 'global' | 'workspace' | 'agent' | 'tool';
    rules: GovernanceRule[];
    enabled: boolean;
    createdAt: string;
    updatedAt: string;
}

export interface GovernanceRule {
    id: string;
    policyId: string;
    condition: string;
    action: GovernanceAction;
    rationale: string;
    priority: number;
}

export interface WorkspaceGraphNode {
    id: string;
    type: NodeType;
    label: string;
    data: Agent | Task | Artifact | Tool | Record<string, unknown>;
    position: { x: number; y: number };
    metadata: Record<string, unknown>;
}

export interface WorkspaceGraphEdge {
    id: string;
    source: string;
    target: string;
    type: EdgeType;
    label?: string;
    metadata: Record<string, unknown>;
}

export interface WorkspaceGraph {
    id: string;
    name: string;
    nodes: WorkspaceGraphNode[];
    edges: WorkspaceGraphEdge[];
    createdAt: string;
    updatedAt: string;
}

export interface WorkspaceSummary {
    name: string;
    agentCount: number;
    taskCount: number;
    runCount: number;
    artifactCount: number;
    governancePolicyCount: number;
    governanceStatus: 'active' | 'degraded' | 'inactive';
}

// ─── Token Flow Recording ───────────────────────────────────────────────────

export interface TokenRecord {
    callIndex: number;
    stepSequence: number;
    stepType: TraceStepType;
    model: string;
    inputTokens: number;
    outputTokens: number;
    cachedTokens: number;
    durationMs: number;
    loopIndex?: number;
}

export interface TokenFlowRecord {
    flowId: string;
    agentId: string;
    taskId: string;
    model: string;
    calls: TokenRecord[];
    totalInputTokens: number;
    totalOutputTokens: number;
    totalCachedTokens: number;
    loopCount: number;
    totalCalls: number;
    startedAt: string;
    completedAt: string;
    durationMs: number;
}

// ─── Performance Metrics ────────────────────────────────────────────────────

/**
 * Network platform metrics.
 * RTT/throughput sourced from Jurasovic 2006 (multi-agent communication study).
 * Connection cost Lt(h), stability istabt(h), security isect(h) from Król 2008.
 */
export interface PlatformMetrics {
    avgRttMs: number;
    minRttMs: number;
    maxRttMs: number;
    rttStdDevMs: number;
    throughputTokensPerSec: number;
    messageOverheadPct: number;
    stabilityScore: number;
    connectionCostMs: number;
    securityScore: number;
}

/**
 * Multi-agent planning metrics.
 * All 7 dimensions from REALM-Bench (Geng & Chang, 2025):
 * task completion rate, on-time rate, makespan, disruption recovery,
 * constraint satisfaction, planning efficiency, inter-agent dependency resolution.
 */
export interface PlanningMetrics {
    taskCompletionRate: number;
    onTimeRate: number;
    makespanMs: number;
    disruptionRecoveryRate: number;
    constraintSatisfactionRate: number;
    planningEfficiency: number;
    interAgentDependencyResolutionRate: number;
}

/**
 * LLM agent quality metrics from Härer 2025:
 * response accuracy rate, tool execution success rate,
 * average reasoning depth, coordination overhead, hallucination rate.
 */
export interface LLMQualityMetrics {
    responseAccuracyRate: number;
    toolExecutionSuccessRate: number;
    avgReasoningDepth: number;
    coordinationOverheadMs: number;
    hallucinationRate: number;
}

/**
 * AgentBench environment scores (Liu et al., 2023).
 * 8 interactive evaluation environments:
 * OS (shell), DB (database), KG (knowledge graph), HH (household),
 * WS (web shopping), ALF (AlfWorld), WB (WebArena), LTP (lateral thinking).
 * Each score: 0–1 success rate. overall = mean across environments.
 */
export interface AgentBenchScores {
    os: number;
    db: number;
    kg: number;
    hh: number;
    ws: number;
    alf: number;
    wb: number;
    ltp: number;
    overall: number;
}

export interface EvaluationMetricSet {
    evaluationId: string;
    agentId: string;
    framework: 'langgraph' | 'autogen' | 'crewai' | 'swarm' | 'custom';
    platform: PlatformMetrics;
    planning: PlanningMetrics;
    quality: LLMQualityMetrics;
    agentBench?: AgentBenchScores;
    tokenFlow: TokenFlowRecord;
    runAt: string;
}

export interface PerformanceSample {
    runIndex: number;
    durationMs: number;
    totalTokens: number;
    inputTokens: number;
    outputTokens: number;
    stepsCount: number;
    loopCount: number;
    toolCallCount: number;
    successRate: number;
}

export interface PerformanceTestResult {
    testId: string;
    agentId: string;
    framework: string;
    model: string;
    samples: PerformanceSample[];
    platform: PlatformMetrics;
    planning: PlanningMetrics;
    quality: LLMQualityMetrics;
    agentBench?: AgentBenchScores;
    runAt: string;
}
