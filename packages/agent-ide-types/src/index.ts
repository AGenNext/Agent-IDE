// Agent Workspace OS — Core Domain Types
// Extension point: add new types here as runtime capabilities are added.

export type AgentStatus = 'idle' | 'running' | 'paused' | 'error' | 'completed';
export type TaskStatus = 'pending' | 'in_progress' | 'completed' | 'failed' | 'cancelled';
export type ArtifactType = 'file' | 'code' | 'document' | 'data' | 'model' | 'report';
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
    // TODO: MCP gateway integration — mcpServerId links to a registered MCP server
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
    /** Inline content for small artifacts */
    content?: string;
    /** Reference to external storage for large artifacts */
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
    // TODO: LangGraph runtime integration — trace populated by agent executor
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
    // TODO: XYFlow integration — position used by ReactFlow
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
