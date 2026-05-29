/**
 * Platform persistence layer — localStorage.
 * All reads return empty defaults on missing/corrupt data.
 * Writes silently drop on quota exceeded.
 *
 * Storage keys:
 *   agent-ide:runs         → AgentRun[]     (last 200)
 *   agent-ide:agents       → AgentConfig[]  (all)
 *   agent-ide:token-flows  → TokenFlow[]    (last 100)
 *   agent-ide:evals        → EvalResult[]   (last 50)
 *   agent-ide:settings     → Settings       (singleton)
 */

export interface StoredAgentConfig {
    id: string;
    name: string;
    model: string;
    tools: string[];
    systemPrompt: string;
    temperature: number;
    maxTokens: number;
    createdAt: string;
    updatedAt: string;
    metadata: Record<string, unknown>;
}

export interface StoredRun {
    id: string;
    agentId: string;
    taskId: string;
    status: string;
    model: string;
    inputTokens: number;
    outputTokens: number;
    toolCallCount: number;
    loopCount: number;
    durationMs: number;
    success: boolean;
    startedAt: string;
    completedAt: string;
    output: string;
    error?: string;
}

export interface StoredTokenFlow {
    flowId: string;
    agentId: string;
    model: string;
    totalInputTokens: number;
    totalOutputTokens: number;
    totalCachedTokens: number;
    totalCalls: number;
    loopCount: number;
    durationMs: number;
    recordedAt: string;
}

export interface StoredEval {
    evalId: string;
    agentId: string;
    framework: string;
    scores: Record<string, number>;
    runAt: string;
}

export interface PlatformSettings {
    openAiApiKey?: string;
    openAiBaseUrl?: string;
    defaultModel?: string;
    budgetUsd?: number;
    theme?: 'dark' | 'light';
    enablePersistence?: boolean;
}

const K = {
    runs:      'agent-ide:runs',
    agents:    'agent-ide:agents',
    flows:     'agent-ide:token-flows',
    evals:     'agent-ide:evals',
    settings:  'agent-ide:settings',
};

function get<T>(key: string, def: T): T {
    try {
        const raw = typeof localStorage !== 'undefined' ? localStorage.getItem(key) : null;
        return raw !== null ? JSON.parse(raw) as T : def;
    } catch { return def; }
}

function put(key: string, value: unknown): void {
    try {
        if (typeof localStorage !== 'undefined') localStorage.setItem(key, JSON.stringify(value));
    } catch { /* quota exceeded or SSR */ }
}

export const persistence = {
    // ─── Runs ──────────────────────────────────────────────────────────
    getRuns: (): StoredRun[] => get<StoredRun[]>(K.runs, []),
    saveRun(run: StoredRun): void {
        const list = get<StoredRun[]>(K.runs, []);
        const idx = list.findIndex(r => r.id === run.id);
        if (idx >= 0) list[idx] = run; else list.unshift(run);
        put(K.runs, list.slice(0, 200));
    },
    deleteRun(id: string): void { put(K.runs, get<StoredRun[]>(K.runs, []).filter(r => r.id !== id)); },

    // ─── Agents ─────────────────────────────────────────────────────────
    getAgents: (): StoredAgentConfig[] => get<StoredAgentConfig[]>(K.agents, []),
    saveAgent(agent: StoredAgentConfig): void {
        const list = get<StoredAgentConfig[]>(K.agents, []);
        const idx = list.findIndex(a => a.id === agent.id);
        if (idx >= 0) list[idx] = { ...agent, updatedAt: new Date().toISOString() }; else list.push(agent);
        put(K.agents, list);
    },
    deleteAgent(id: string): void { put(K.agents, get<StoredAgentConfig[]>(K.agents, []).filter(a => a.id !== id)); },

    // ─── Token flows ─────────────────────────────────────────────────────
    getTokenFlows: (): StoredTokenFlow[] => get<StoredTokenFlow[]>(K.flows, []),
    saveTokenFlow(flow: StoredTokenFlow): void {
        const list = get<StoredTokenFlow[]>(K.flows, []);
        list.unshift(flow);
        put(K.flows, list.slice(0, 100));
    },

    // ─── Evals ───────────────────────────────────────────────────────────
    getEvals: (): StoredEval[] => get<StoredEval[]>(K.evals, []),
    saveEval(e: StoredEval): void {
        const list = get<StoredEval[]>(K.evals, []);
        list.unshift(e);
        put(K.evals, list.slice(0, 50));
    },

    // ─── Settings ─────────────────────────────────────────────────────────
    getSettings: (): PlatformSettings => get<PlatformSettings>(K.settings, {}),
    saveSettings(s: Partial<PlatformSettings>): void {
        const current = get<PlatformSettings>(K.settings, {});
        put(K.settings, { ...current, ...s });
    },

    // ─── Utility ───────────────────────────────────────────────────────────
    storageUsedBytes(): number {
        try {
            return Object.values(K).reduce((s, k) => s + (localStorage.getItem(k)?.length ?? 0) * 2, 0);
        } catch { return 0; }
    },
    clearAll(): void {
        try { Object.values(K).forEach(k => localStorage.removeItem(k)); } catch { /* ignore */ }
    },
};
