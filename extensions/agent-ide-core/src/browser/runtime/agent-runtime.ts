// Agent Runtime Engine
// Phase 1: simulation. OpenAI/Anthropic integration points marked TODO.

export type ExecutionStage =
    | 'idle' | 'planning' | 'thinking' | 'acting'
    | 'observing' | 'synthesizing' | 'done' | 'error';

export type RuntimeEventType =
    | 'stage_change' | 'thought' | 'tool_call' | 'tool_result'
    | 'token_delta' | 'loop_detected' | 'subtask_spawned' | 'completed' | 'error';

export interface RuntimeEvent {
    type: RuntimeEventType;
    agentId: string;
    taskId: string;
    stage: ExecutionStage;
    payload: unknown;
    timestamp: string;
    tokensDelta?: number;
}

export interface ExecutionRequest {
    agentId: string;
    taskId: string;
    input: string;
    model: string;
    /** OpenAI API key — if set, live execution replaces simulation */
    apiKey?: string;
    tools: string[];
    systemPrompt: string;
    maxIterations?: number;
    temperature?: number;
}

export interface ExecutionResult {
    agentId: string;
    taskId: string;
    output: string;
    stages: ExecutionStage[];
    totalInputTokens: number;
    totalOutputTokens: number;
    toolCallCount: number;
    loopCount: number;
    durationMs: number;
    success: boolean;
    error?: string;
}

export type EventHandler = (event: RuntimeEvent) => void;

function approxTokens(text: string): number {
    return Math.ceil(text.length / 3.8);
}

function delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

export class AgentRuntimeEngine {
    private handlers: EventHandler[] = [];

    onEvent(handler: EventHandler): () => void {
        this.handlers.push(handler);
        return () => { this.handlers = this.handlers.filter(h => h !== handler); };
    }

    private emit(event: RuntimeEvent): void {
        this.handlers.forEach(h => h(event));
    }

    async execute(req: ExecutionRequest): Promise<ExecutionResult> {
        // TODO: if req.apiKey is set, route to OpenAI chat completions with tool_choice
        // TODO: stream delta tokens via 'token_delta' events for real-time display
        return this.simulate(req);
    }

    private async simulate(req: ExecutionRequest): Promise<ExecutionResult> {
        const start = Date.now();
        const { agentId, taskId, model, tools, systemPrompt } = req;
        const maxIter = Math.min(req.maxIterations ?? 5, 8);
        let totalIn = 0, totalOut = 0, toolCalls = 0, loopCount = 0;
        const stages: ExecutionStage[] = [];

        const emit = (type: RuntimeEventType, stage: ExecutionStage, payload: unknown, tokensDelta?: number) =>
            this.emit({ type, agentId, taskId, stage, payload, timestamp: new Date().toISOString(), tokensDelta });

        // PLANNING
        stages.push('planning');
        emit('stage_change', 'planning', { message: 'Decomposing task into execution steps' });
        totalIn += approxTokens(systemPrompt) + approxTokens(req.input) + 80;
        await delay(100);

        for (let i = 0; i < maxIter; i++) {
            // THINK
            stages.push('thinking');
            const isLoop = i > 0;
            if (isLoop) loopCount++;
            const thought = isLoop
                ? `Step ${i + 1}: reviewing prior result, refining approach.`
                : `Step ${i + 1}: analyzing "${req.input.slice(0, 50)}" — selecting best tool.`;
            const tIn = approxTokens(thought) + 60, tOut = approxTokens(thought) + 30;
            totalIn += tIn; totalOut += tOut;
            emit('thought', 'thinking', { content: thought }, tIn + tOut);
            if (isLoop) emit('loop_detected', 'thinking', { iteration: i, reason: 'result insufficient' });
            await delay(80 + Math.random() * 80);

            // ACT
            if (tools.length > 0) {
                stages.push('acting');
                const tool = tools[i % tools.length];
                const toolInput = { query: req.input, step: i + 1 };
                emit('tool_call', 'acting', { tool, input: toolInput });
                await delay(180 + Math.random() * 250);
                toolCalls++;

                // OBSERVE
                stages.push('observing');
                const obs = `${tool} returned ${2 + i} result(s). Confidence: ${i < 2 ? 'high' : 'medium'}.`;
                const oIn = approxTokens(obs) + 40, oOut = approxTokens(obs) + 20;
                totalIn += oIn; totalOut += oOut;
                emit('tool_result', 'observing', { tool, output: obs }, oIn + oOut);
                await delay(50);
            }

            if (!isLoop || i >= maxIter - 2) break;
        }

        // SYNTHESIZE
        stages.push('synthesizing');
        emit('stage_change', 'synthesizing', { message: 'Composing final answer' });
        const synth = `Task complete. Model: ${model}. Tools used: ${toolCalls}. Loops: ${loopCount}.`;
        const sIn = approxTokens(synth) + 100, sOut = approxTokens(synth) + 180;
        totalIn += sIn; totalOut += sOut;
        await delay(120);

        stages.push('done');
        emit('completed', 'done', { output: synth, totalIn, totalOut });

        return {
            agentId, taskId, output: synth, stages,
            totalInputTokens: totalIn, totalOutputTokens: totalOut,
            toolCallCount: toolCalls, loopCount,
            durationMs: Date.now() - start, success: true,
        };
    }
}

export const globalRuntime = new AgentRuntimeEngine();
