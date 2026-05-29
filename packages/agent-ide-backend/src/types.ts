import { TraceStepType } from '@agennext/agent-ide-types';

// ─── Run lifecycle ────────────────────────────────────────────────────────────

export type RunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface RunRequest {
    agentId: string;
    agentName: string;
    model: string;
    systemPrompt: string;
    task: string;
    tools: string[];
    maxIterations?: number;
    apiKey?: string;         // caller-supplied; never logged or stored
    temperature?: number;
    maxTokens?: number;
}

export interface RunRecord {
    runId: string;
    agentId: string;
    agentName: string;
    model: string;
    task: string;
    status: RunStatus;
    startedAt: string;
    completedAt?: string;
    steps: TraceStepRecord[];
    totalInputTokens: number;
    totalOutputTokens: number;
    estimatedCostUsd: number;
    errorMessage?: string;
}

export interface TraceStepRecord {
    sequence: number;
    type: TraceStepType;
    content: string;
    toolName?: string;
    toolInput?: Record<string, unknown>;
    toolOutput?: unknown;
    inputTokens: number;
    outputTokens: number;
    durationMs: number;
    loopIndex?: number;
    timestamp: string;
}

// ─── WebSocket messages ───────────────────────────────────────────────────────

export type WsMessageType = 'run:started' | 'run:step' | 'run:completed' | 'run:failed' | 'run:cancelled' | 'governance:approval-request' | 'governance:approval-resolved';

export interface WsMessage {
    type: WsMessageType;
    runId: string;
    payload: unknown;
}

// ─── Tool invocation ─────────────────────────────────────────────────────────

export interface ToolInvokeRequest {
    toolId: string;
    input: Record<string, unknown>;
    agentId?: string;
    runId?: string;
}

export interface ToolInvokeResult {
    toolId: string;
    output: unknown;
    durationMs: number;
    success: boolean;
    error?: string;
}

// ─── OpenAI-compatible types ──────────────────────────────────────────────────

export interface OAIMessage {
    role: 'system' | 'user' | 'assistant' | 'tool';
    content: string | null;
    tool_calls?: OAIToolCall[];
    tool_call_id?: string;
    name?: string;
}

export interface OAIToolCall {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
}

export interface OAITool {
    type: 'function';
    function: {
        name: string;
        description: string;
        parameters: { type: 'object'; properties: Record<string, unknown>; required: string[] };
    };
}

export interface OAIResponse {
    id: string;
    choices: Array<{
        message: {
            role: string;
            content: string | null;
            tool_calls?: OAIToolCall[];
        };
        finish_reason: string;
    }>;
    usage?: { prompt_tokens: number; completion_tokens: number };
}
