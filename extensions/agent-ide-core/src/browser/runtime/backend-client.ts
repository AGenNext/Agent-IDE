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
