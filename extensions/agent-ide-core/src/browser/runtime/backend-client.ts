/**
 * Thin HTTP + WebSocket client for the agent-ide-backend server.
 * Falls back gracefully when the backend is not reachable.
 */

export const BACKEND_URL = (typeof window !== 'undefined' && (window as Window & { AGENT_IDE_BACKEND?: string }).AGENT_IDE_BACKEND)
    ? (window as Window & { AGENT_IDE_BACKEND?: string }).AGENT_IDE_BACKEND!
    : 'http://localhost:3001';

export interface RunRequest {
    agentId:      string;
    agentName:    string;
    model:        string;
    systemPrompt: string;
    task:         string;
    tools:        string[];
    apiKey?:      string;
    temperature?: number;
    maxTokens?:   number;
    maxIterations?: number;
}

export interface RunSummary {
    runId:             string;
    agentId:           string;
    agentName:         string;
    model:             string;
    task:              string;
    status:            string;
    startedAt:         string;
    completedAt?:      string;
    stepCount:         number;
    totalInputTokens:  number;
    totalOutputTokens: number;
    estimatedCostUsd:  number;
}

export interface BackendConfig {
    workspaceRoot:   string;
    allowShell:      boolean;
    hasBraveKey:     boolean;
    hasOpenAiKey:    boolean;
    hasAnthropicKey: boolean;
    hasDatabaseUrl:  boolean;
    port:            number;
    mcpServerCount:  number;
    mcpConnected:    number;
}

export type McpTransport = 'stdio' | 'sse' | 'websocket';
export type McpStatus = 'connected' | 'connecting' | 'disconnected' | 'error';

export interface McpServerState {
    id:           string;
    name:         string;
    transport:    McpTransport;
    command?:     string;
    endpoint?:    string;
    status:       McpStatus;
    tools:        McpToolDef[];
    toolCount:    number;
    error?:       string;
    connectedAt?: string;
    autoConnect?: boolean;
}

export interface McpToolDef {
    serverId:    string;
    serverName:  string;
    name:        string;
    description: string;
    inputSchema: Record<string, unknown>;
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(`${BACKEND_URL}${path}`, {
        ...init,
        headers: { 'Content-Type': 'application/json', ...(init?.headers ?? {}) },
    });
    if (!res.ok) {
        const text = await res.text();
        throw new Error(`Backend ${path} → ${res.status}: ${text.slice(0, 200)}`);
    }
    return res.json() as Promise<T>;
}

export async function isBackendReachable(): Promise<boolean> {
    try {
        await fetch(`${BACKEND_URL}/health`, { signal: AbortSignal.timeout(2000) });
        return true;
    } catch {
        return false;
    }
}

export async function getBackendConfig(): Promise<BackendConfig> {
    return apiFetch<BackendConfig>('/api/config');
}

export async function submitRun(req: RunRequest): Promise<{ runId: string; wsUrl: string }> {
    return apiFetch<{ runId: string; wsUrl: string }>('/api/runs', {
        method: 'POST',
        body: JSON.stringify(req),
    });
}

export async function listRuns(): Promise<RunSummary[]> {
    return apiFetch<RunSummary[]>('/api/runs');
}

export async function getRun(runId: string): Promise<unknown> {
    return apiFetch<unknown>(`/api/runs/${runId}`);
}

export async function cancelRun(runId: string): Promise<void> {
    await apiFetch<unknown>(`/api/runs/${runId}`, { method: 'DELETE' });
}

// ─── Auth API ─────────────────────────────────────────────────────────────────

export interface AuthUser {
    userId: string;
    email:  string;
    name:   string;
}

export interface LoginResult {
    token: string;
    user:  AuthUser;
}

export async function login(email: string, password: string): Promise<LoginResult> {
    return apiFetch<LoginResult>('/api/auth/login', {
        method: 'POST',
        body: JSON.stringify({ email, password }),
    });
}

export async function getMe(token?: string): Promise<AuthUser> {
    const headers: Record<string, string> = {};
    if (token) headers['Authorization'] = `Bearer ${token}`;
    return apiFetch<AuthUser>('/api/auth/me', { headers });
}

// ─── Workspaces API ───────────────────────────────────────────────────────────

export type WorkspaceStatus = 'active' | 'inactive' | 'provisioning' | 'error';

export interface WorkspaceRecord {
    id:         string;
    tenantId:   string;
    name:       string;
    status:     WorkspaceStatus;
    createdAt:  string;
    updatedAt:  string;
    rootPath?:  string;
}

function authHeaders(token?: string): Record<string, string> {
    return token ? { Authorization: `Bearer ${token}` } : {};
}

export async function listWorkspaces(token?: string): Promise<WorkspaceRecord[]> {
    return apiFetch<WorkspaceRecord[]>('/api/workspaces', { headers: authHeaders(token) });
}

export async function createWorkspace(name: string, token?: string): Promise<WorkspaceRecord> {
    return apiFetch<WorkspaceRecord>('/api/workspaces', {
        method: 'POST',
        body: JSON.stringify({ name }),
        headers: authHeaders(token),
    });
}

export async function renameWorkspace(id: string, name: string, token?: string): Promise<WorkspaceRecord> {
    return apiFetch<WorkspaceRecord>(`/api/workspaces/${id}`, {
        method: 'PATCH',
        body: JSON.stringify({ name }),
        headers: authHeaders(token),
    });
}

export async function deleteWorkspace(id: string, token?: string): Promise<void> {
    await apiFetch<unknown>(`/api/workspaces/${id}`, {
        method: 'DELETE',
        headers: authHeaders(token),
    });
}

export async function activateWorkspace(id: string, token?: string): Promise<WorkspaceRecord> {
    return apiFetch<WorkspaceRecord>(`/api/workspaces/${id}/activate`, {
        method: 'POST',
        body: '{}',
        headers: authHeaders(token),
    });
}

// ─── Knowledge API ────────────────────────────────────────────────────────────

export interface KnowledgeChunkSummary {
    id:             string;
    title:          string;
    source:         string;
    createdAt:      string;
    contentPreview: string;
    metadata:       Record<string, unknown>;
}

export interface KnowledgeSearchResult {
    score:          number;
    id:             string;
    title:          string;
    source:         string;
    contentPreview: string;
    createdAt:      string;
}

export async function listKnowledge(token?: string): Promise<KnowledgeChunkSummary[]> {
    return apiFetch<KnowledgeChunkSummary[]>('/api/knowledge', { headers: authHeaders(token) });
}

export async function searchKnowledge(query: string, topK = 5, token?: string): Promise<KnowledgeSearchResult[]> {
    return apiFetch<KnowledgeSearchResult[]>('/api/knowledge/search', {
        method: 'POST',
        body: JSON.stringify({ query, topK }),
        headers: authHeaders(token),
    });
}

export async function ingestText(title: string, content: string, token?: string): Promise<{ chunks: number; ids: string[] }> {
    return apiFetch<{ chunks: number; ids: string[] }>('/api/knowledge/ingest', {
        method: 'POST',
        body: JSON.stringify({ title, content }),
        headers: authHeaders(token),
    });
}

export async function ingestUrl(url: string, token?: string): Promise<{ chunks: number; ids: string[] }> {
    return apiFetch<{ chunks: number; ids: string[] }>('/api/knowledge/ingest', {
        method: 'POST',
        body: JSON.stringify({ type: 'url', url }),
        headers: authHeaders(token),
    });
}

export async function deleteKnowledgeChunk(id: string, token?: string): Promise<void> {
    await apiFetch<unknown>(`/api/knowledge/${id}`, {
        method: 'DELETE',
        headers: authHeaders(token),
    });
}

// ─── MCP API ──────────────────────────────────────────────────────────────────

export async function listMcpServers(): Promise<McpServerState[]> {
    return apiFetch<McpServerState[]>('/api/mcp/servers');
}

export async function getMcpServer(id: string): Promise<McpServerState> {
    return apiFetch<McpServerState>(`/api/mcp/servers/${id}`);
}

export async function addMcpServer(cfg: { id: string; name: string; transport: McpTransport; command?: string; endpoint?: string; env?: Record<string, string> }): Promise<McpServerState> {
    return apiFetch<McpServerState>('/api/mcp/servers', { method: 'POST', body: JSON.stringify(cfg) });
}

export async function removeMcpServer(id: string): Promise<void> {
    await apiFetch<unknown>(`/api/mcp/servers/${id}`, { method: 'DELETE' });
}

export async function connectMcpServer(id: string): Promise<McpServerState> {
    return apiFetch<McpServerState>(`/api/mcp/servers/${id}/connect`, { method: 'POST', body: '{}' });
}

export async function disconnectMcpServer(id: string): Promise<void> {
    await apiFetch<unknown>(`/api/mcp/servers/${id}/disconnect`, { method: 'POST', body: '{}' });
}

export async function listMcpTools(): Promise<McpToolDef[]> {
    return apiFetch<McpToolDef[]>('/api/mcp/tools');
}

export async function callMcpTool(serverId: string, toolName: string, args: Record<string, unknown>): Promise<unknown> {
    return apiFetch<unknown>(`/api/mcp/tools/${serverId}/${toolName}/call`, { method: 'POST', body: JSON.stringify({ args }) });
}

/**
 * Open a WebSocket to stream trace steps for a run.
 * Calls onStep for each trace step, onDone when the run completes or fails.
 */
export function streamRun(
    runId: string,
    onStep: (step: unknown) => void,
    onDone: (run: unknown, error?: string) => void,
): WebSocket {
    const wsBase = BACKEND_URL.replace(/^http/, 'ws');
    const ws = new WebSocket(`${wsBase}/ws/${runId}`);

    ws.onmessage = (event) => {
        try {
            const msg = JSON.parse(event.data as string) as { type: string; runId: string; payload: unknown };
            if (msg.type === 'run:step') {
                onStep(msg.payload);
            } else if (msg.type === 'run:completed') {
                onDone(msg.payload);
                ws.close();
            } else if (msg.type === 'run:failed') {
                const p = msg.payload as { error?: string };
                onDone(null, p.error ?? 'Run failed');
                ws.close();
            }
        } catch { /* ignore parse errors */ }
    };

    ws.onerror = () => onDone(null, 'WebSocket error — backend may be unreachable');

    return ws;
}
