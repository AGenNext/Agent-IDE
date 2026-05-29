import { AgentRuntimeEngine, ExecutionRequest, ExecutionResult, RuntimeEvent } from './agent-runtime';

export interface AgentHandle {
    id: string;
    name: string;
    model: string;
    tools: string[];
    systemPrompt: string;
    apiKey?: string;
    baseUrl?: string;
}

export interface OrchestratorSubtask { agentId: string; input: string; dependsOn: string[]; }
export interface OrchestratorPlan { planId: string; input: string; subtasks: OrchestratorSubtask[]; }
export interface OrchestratorResult {
    planId: string;
    results: Record<string, ExecutionResult>;
    aggregatedOutput: string;
    totalDurationMs: number;
    totalTokens: number;
    success: boolean;
}

/**
 * Generate an execution plan using an LLM planner.
 * The planner is called with the task and agent descriptions,
 * and must return JSON: { "subtasks": [{ "agentId", "input", "dependsOn" }] }
 *
 * Falls back to sequential assignment if the planner response is not valid JSON.
 */
async function generatePlan(
    input: string,
    agents: AgentHandle[],
    plannerAgent: AgentHandle,
    runtime: AgentRuntimeEngine
): Promise<OrchestratorPlan> {
    const agentList = agents
        .map(a => `  - ${a.id} (${a.name}): ${a.systemPrompt.slice(0, 120)}`)
        .join('\n');

    const plannerPrompt = [
        'You are a task planner. Decompose the given task into subtasks, one per available agent.',
        'Return ONLY valid JSON in this exact shape:',
        '{ "subtasks": [{ "agentId": string, "input": string, "dependsOn": string[] }] }',
        'dependsOn lists agentIds whose output must be available before this subtask starts.',
        'Available agents:',
        agentList,
    ].join('\n');

    const planId = `plan-${Date.now()}`;

    try {
        const result = await runtime.execute({
            agentId: plannerAgent.id,
            taskId: `${planId}-plan`,
            input,
            model: plannerAgent.model,
            apiKey: plannerAgent.apiKey,
            baseUrl: plannerAgent.baseUrl,
            tools: [],
            systemPrompt: plannerPrompt,
            maxIterations: 1,
            temperature: 0.2,
        });

        // Extract JSON from output (model may wrap in markdown code block)
        const jsonMatch = result.output.match(/\{[\s\S]*\}/);
        if (jsonMatch) {
            const parsed = JSON.parse(jsonMatch[0]) as { subtasks: OrchestratorSubtask[] };
            if (Array.isArray(parsed.subtasks)) {
                return { planId, input, subtasks: parsed.subtasks };
            }
        }
    } catch { /* fall through to sequential fallback */ }

    // Fallback: run all agents sequentially, each receiving the original input
    return {
        planId, input,
        subtasks: agents.map((a, i) => ({
            agentId: a.id,
            input,
            dependsOn: i > 0 ? [agents[i - 1].id] : [],
        })),
    };
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
    unregisterAgent(id: string): void { this.agents.delete(id); }
    getAgents(): AgentHandle[] { return Array.from(this.agents.values()); }
    getEventLog(): RuntimeEvent[] { return [...this.eventLog]; }

    /**
     * Auto-generate a plan using the specified planner agent, then execute it.
     * If plannerAgentId is omitted, falls back to sequential execution.
     */
    async run(
        input: string,
        plannerAgentId?: string,
        onEvent?: (e: RuntimeEvent) => void
    ): Promise<OrchestratorResult> {
        const agents = this.getAgents().filter(a => a.id !== plannerAgentId);
        let plan: OrchestratorPlan;

        if (plannerAgentId) {
            const planner = this.agents.get(plannerAgentId);
            if (planner) {
                plan = await generatePlan(input, agents, planner, this.runtime);
            } else {
                plan = { planId: `plan-${Date.now()}`, input, subtasks: agents.map((a, i) => ({ agentId: a.id, input, dependsOn: i > 0 ? [agents[i-1].id] : [] })) };
            }
        } else {
            plan = { planId: `plan-${Date.now()}`, input, subtasks: agents.map((a, i) => ({ agentId: a.id, input, dependsOn: i > 0 ? [agents[i-1].id] : [] })) };
        }

        return this.runPlan(plan, onEvent);
    }

    async runPlan(plan: OrchestratorPlan, onEvent?: (e: RuntimeEvent) => void): Promise<OrchestratorResult> {
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

                    // If a dependency produced output, inject it into the input
                    const depOutput = st.dependsOn
                        .map(dep => results[dep]?.output)
                        .filter(Boolean)
                        .join('\n');
                    const enrichedInput = depOutput ? `Context from prior agents:\n${depOutput}\n\nTask: ${st.input}` : st.input;

                    const req: ExecutionRequest = {
                        agentId: st.agentId,
                        taskId: `${plan.planId}/${st.agentId}`,
                        input: enrichedInput,
                        model: h.model,
                        apiKey: h.apiKey,
                        baseUrl: h.baseUrl,
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
                planId: plan.planId, results,
                aggregatedOutput: Object.values(results).map(r => r.output).join('\n---\n'),
                totalDurationMs: Date.now() - start, totalTokens,
                success: Object.values(results).every(r => r.success),
            };
        } finally {
            off?.();
        }
    }
}

export const globalOrchestrator = new AgentOrchestrator(globalRuntime);
import { globalRuntime } from './agent-runtime';
