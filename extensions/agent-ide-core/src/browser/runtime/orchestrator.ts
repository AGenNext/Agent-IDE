import { AgentRuntimeEngine, ExecutionRequest, ExecutionResult, RuntimeEvent } from './agent-runtime';

export interface AgentHandle {
    id: string;
    name: string;
    model: string;
    tools: string[];
    systemPrompt: string;
}

export interface OrchestratorSubtask {
    agentId: string;
    input: string;
    dependsOn: string[];
}

export interface OrchestratorPlan {
    planId: string;
    input: string;
    subtasks: OrchestratorSubtask[];
}

export interface OrchestratorResult {
    planId: string;
    results: Record<string, ExecutionResult>;
    aggregatedOutput: string;
    totalDurationMs: number;
    totalTokens: number;
    success: boolean;
}

export class AgentOrchestrator {
    private agents = new Map<string, AgentHandle>();
    private eventLog: RuntimeEvent[] = [];
    private readonly maxLog = 500;

    constructor(private runtime: AgentRuntimeEngine) {
        runtime.onEvent(e => {
            this.eventLog.unshift(e);
            if (this.eventLog.length > this.maxLog) this.eventLog.length = this.maxLog;
        });
    }

    registerAgent(h: AgentHandle): void { this.agents.set(h.id, h); }
    getAgents(): AgentHandle[] { return Array.from(this.agents.values()); }
    getEventLog(): RuntimeEvent[] { return [...this.eventLog]; }

    async runPlan(
        plan: OrchestratorPlan,
        onEvent?: (e: RuntimeEvent) => void
    ): Promise<OrchestratorResult> {
        const start = Date.now();
        const results: Record<string, ExecutionResult> = {};
        const off = onEvent ? this.runtime.onEvent(onEvent) : undefined;

        try {
            const pending = [...plan.subtasks];
            const done = new Set<string>();

            while (pending.length > 0) {
                const ready = pending.filter(st => st.dependsOn.every(d => done.has(d)));
                if (ready.length === 0) break; // circular dep guard

                await Promise.all(ready.map(async st => {
                    const h = this.agents.get(st.agentId);
                    if (!h) return;
                    const req: ExecutionRequest = {
                        agentId: st.agentId,
                        taskId: `${plan.planId}/${st.agentId}`,
                        input: st.input,
                        model: h.model,
                        tools: h.tools,
                        systemPrompt: h.systemPrompt,
                    };
                    results[st.agentId] = await this.runtime.execute(req);
                    done.add(st.agentId);
                }));

                pending.splice(0, pending.length, ...pending.filter(st => !done.has(st.agentId)));
            }

            const totalTokens = Object.values(results)
                .reduce((s, r) => s + r.totalInputTokens + r.totalOutputTokens, 0);

            return {
                planId: plan.planId,
                results,
                aggregatedOutput: Object.values(results).map(r => r.output).join('\n---\n'),
                totalDurationMs: Date.now() - start,
                totalTokens,
                success: Object.values(results).every(r => r.success),
            };
        } finally {
            off?.();
        }
    }
}
