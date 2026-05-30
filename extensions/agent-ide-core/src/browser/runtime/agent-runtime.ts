// Agent Runtime Engine
// executeWithOpenAI: live OpenAI chat completions with streaming + tool dispatch loop
// simulate: deterministic simulation for dev/test when no apiKey

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
    /** Base URL override (proxy, Azure, local Ollama, etc.) */
    baseUrl?: string;
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

// ─── OpenAI wire types ─────────────────────────────────────────────────────

interface OAIToolCall {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
}

interface OAIMessage {
    role: 'system' | 'user' | 'assistant' | 'tool';
    content: string | null;
    tool_calls?: OAIToolCall[];
    tool_call_id?: string;
}

interface OAITool {
    type: 'function';
    function: {
        name: string;
        description: string;
        parameters: { type: 'object'; properties: Record<string, unknown>; required: string[] };
    };
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function approxTokens(text: string): number {
    return Math.ceil(text.length / 3.8);
}

function delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Stream OpenAI chat completions, returning accumulated content + tool calls.
 * Emits onDelta for each content token chunk.
 */
async function streamChatCompletion(
    baseUrl: string,
    apiKey: string,
    model: string,
    messages: OAIMessage[],
    tools: OAITool[],
    temperature: number,
    onDelta: (delta: string) => void
): Promise<{ content: string; toolCalls: OAIToolCall[] }> {
    const url = `${baseUrl}/chat/completions`;
    const body: Record<string, unknown> = {
        model, messages, temperature, stream: true,
    };
    if (tools.length > 0) {
        body['tools'] = tools;
        body['tool_choice'] = 'auto';
    }

    const res = await fetch(url, {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${apiKey}`, 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
    });

    if (!res.ok) {
        let msg = `HTTP ${res.status}`;
        try { const e = await res.json(); msg = (e as any).error?.message ?? msg; } catch { /* ignore */ }
        throw new Error(`OpenAI API error: ${msg}`);
    }

    const reader = res.body!.getReader();
    const decoder = new TextDecoder();
    let content = '';
    // tool call accumulator: index → {id, name, args}
    const tcMap = new Map<number, { id: string; name: string; args: string }>();

    outer: while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        const raw = decoder.decode(value, { stream: true });
        for (const line of raw.split('\n')) {
            if (!line.startsWith('data: ')) continue;
            const data = line.slice(6).trim();
            if (data === '[DONE]') break outer;
            try {
                const parsed = JSON.parse(data);
                const delta = parsed.choices?.[0]?.delta;
                if (!delta) continue;
                if (delta.content) { content += delta.content; onDelta(delta.content); }
                if (delta.tool_calls) {
                    for (const tc of delta.tool_calls as any[]) {
                        if (!tcMap.has(tc.index)) tcMap.set(tc.index, { id: '', name: '', args: '' });
                        const entry = tcMap.get(tc.index)!;
                        if (tc.id)                    entry.id   += tc.id;
                        if (tc.function?.name)        entry.name += tc.function.name;
                        if (tc.function?.arguments)   entry.args += tc.function.arguments;
                    }
                }
            } catch { /* skip malformed SSE chunks */ }
        }
    }

    const toolCalls: OAIToolCall[] = Array.from(tcMap.values()).map(tc => ({
        id: tc.id, type: 'function', function: { name: tc.name, arguments: tc.args },
    }));

    return { content, toolCalls };
}

// ─── Runtime engine ───────────────────────────────────────────────────────────

export class AgentRuntimeEngine {
    private handlers: EventHandler[] = [];

    onEvent(handler: EventHandler): () => void {
        this.handlers.push(handler);
        return () => { this.handlers = this.handlers.filter(h => h !== handler); };
    }

    private emit(type: RuntimeEventType, agentId: string, taskId: string, stage: ExecutionStage, payload: unknown, tokensDelta?: number): void {
        const event: RuntimeEvent = { type, agentId, taskId, stage, payload, timestamp: new Date().toISOString(), tokensDelta };
        this.handlers.forEach(h => h(event));
    }

    async execute(req: ExecutionRequest): Promise<ExecutionResult> {
        return req.apiKey
            ? this.executeWithOpenAI(req)
            : this.simulate(req);
    }

    // ─── Live OpenAI execution ───────────────────────────────────────────

    private async executeWithOpenAI(req: ExecutionRequest): Promise<ExecutionResult> {
        // Import tool helpers lazily to avoid circular deps
        const { getToolById, toOpenAITool } = await import('./tool-registry');

        const start = Date.now();
        const { agentId, taskId, model, systemPrompt, apiKey, maxIterations = 10, temperature = 0.7 } = req;
        const baseUrl = req.baseUrl ?? 'https://api.openai.com/v1';
        const emit = (type: RuntimeEventType, stage: ExecutionStage, payload: unknown, toks?: number) =>
            this.emit(type, agentId, taskId, stage, payload, toks);

        const messages: OAIMessage[] = [
            { role: 'system', content: systemPrompt },
            { role: 'user',   content: req.input },
        ];

        const oaiTools: OAITool[] = req.tools
            .map(id => getToolById(id))
            .filter((t): t is NonNullable<ReturnType<typeof getToolById>> => t !== undefined)
            .map(t => toOpenAITool(t));

        let totalIn = 0, totalOut = 0, toolCallCount = 0, loopCount = 0;
        const stages: ExecutionStage[] = ['planning'];
        emit('stage_change', 'planning', { message: 'Starting live agent execution', model });

        try {
            for (let iteration = 0; iteration < maxIterations; iteration++) {
                if (iteration > 0) {
                    loopCount++;
                    emit('loop_detected', 'thinking', { iteration, reason: 'continuing after tool results' });
                }

                stages.push('thinking');
                emit('stage_change', 'thinking', { iteration });

                // Approximate input token count for this turn
                const inToks = messages.reduce((s, m) => s + approxTokens(m.content ?? JSON.stringify(m.tool_calls ?? '')), 0);
                totalIn += inToks;

                let responseContent = '';
                let responseToolCalls: OAIToolCall[] = [];

                try {
                    const result = await streamChatCompletion(
                        baseUrl, apiKey!, model, messages, oaiTools, temperature,
                        delta => emit('token_delta', 'thinking', { delta }, approxTokens(delta))
                    );
                    responseContent  = result.content;
                    responseToolCalls = result.toolCalls;
                } catch (err) {
                    emit('error', 'error', { message: String(err) });
                    return { agentId, taskId, output: '', stages, totalInputTokens: totalIn, totalOutputTokens: totalOut, toolCallCount, loopCount, durationMs: Date.now() - start, success: false, error: String(err) };
                }

                const outToks = approxTokens(responseContent) +
                    responseToolCalls.reduce((s, tc) => s + approxTokens(tc.function.arguments), 0);
                totalOut += outToks;

                if (responseContent) {
                    emit('thought', 'thinking', { content: responseContent }, outToks);
                }

                messages.push({
                    role: 'assistant',
                    content: responseContent || null,
                    tool_calls: responseToolCalls.length > 0 ? responseToolCalls : undefined,
                });

                if (responseToolCalls.length === 0) break; // no more tool calls — done

                // Dispatch tool calls
                stages.push('acting');
                for (const tc of responseToolCalls) {
                    toolCallCount++;
                    let toolInput: Record<string, unknown> = {};
                    try { toolInput = JSON.parse(tc.function.arguments); } catch { /* ignore bad JSON */ }

                    emit('tool_call', 'acting', { tool: tc.function.name, input: toolInput, callId: tc.id });

                    stages.push('observing');
                    const tool = getToolById(tc.function.name);
                    let toolOutput: unknown;
                    if (tool) {
                        try {
                            toolOutput = await tool.mockExecute(toolInput);
                            emit('tool_result', 'observing', { tool: tc.function.name, output: toolOutput, callId: tc.id });
                        } catch (toolErr) {
                            toolOutput = { error: String(toolErr) };
                            emit('error', 'observing', { tool: tc.function.name, error: String(toolErr) });
                        }
                    } else {
                        toolOutput = { error: `Tool not registered: ${tc.function.name}` };
                    }

                    messages.push({ role: 'tool', content: JSON.stringify(toolOutput), tool_call_id: tc.id });
                }
            }
        } catch (err) {
            emit('error', 'error', { message: String(err) });
            return { agentId, taskId, output: '', stages, totalInputTokens: totalIn, totalOutputTokens: totalOut, toolCallCount, loopCount, durationMs: Date.now() - start, success: false, error: String(err) };
        }

        stages.push('synthesizing');
        emit('stage_change', 'synthesizing', { message: 'Finalizing response' });

        const finalOutput = messages.filter(m => m.role === 'assistant').map(m => m.content ?? '').filter(Boolean).join('\n').trim();

        stages.push('done');
        emit('completed', 'done', { output: finalOutput, totalIn, totalOut, toolCallCount, loopCount });

        return { agentId, taskId, output: finalOutput, stages, totalInputTokens: totalIn, totalOutputTokens: totalOut, toolCallCount, loopCount, durationMs: Date.now() - start, success: true };
    }

    // ─── Simulation (no API key) ───────────────────────────────────────────

    private async simulate(req: ExecutionRequest): Promise<ExecutionResult> {
        const start = Date.now();
        const { agentId, taskId, model, tools, systemPrompt } = req;
        const maxIter = Math.min(req.maxIterations ?? 5, 8);
        let totalIn = 0, totalOut = 0, toolCalls = 0, loopCount = 0;
        const stages: ExecutionStage[] = [];
        const emit = (type: RuntimeEventType, stage: ExecutionStage, payload: unknown, toks?: number) =>
            this.emit(type, agentId, taskId, stage, payload, toks);

        stages.push('planning');
        emit('stage_change', 'planning', { message: 'Simulating agent execution (no API key set)' });
        totalIn += approxTokens(systemPrompt) + approxTokens(req.input) + 80;
        await delay(100);

        for (let i = 0; i < maxIter; i++) {
            stages.push('thinking');
            if (i > 0) { loopCount++; emit('loop_detected', 'thinking', { iteration: i }); }
            const thought = i === 0
                ? `Analyzing: "${req.input.slice(0, 50)}" — selecting approach.`
                : `Refining result (iteration ${i + 1}).`;
            const tIn = approxTokens(thought) + 60, tOut = approxTokens(thought) + 30;
            totalIn += tIn; totalOut += tOut;
            emit('thought', 'thinking', { content: thought }, tIn + tOut);
            await delay(80 + Math.random() * 80);

            if (tools.length > 0) {
                stages.push('acting');
                const tool = tools[i % tools.length];
                emit('tool_call', 'acting', { tool, input: { query: req.input, step: i + 1 } });
                await delay(180 + Math.random() * 250);
                toolCalls++;
                stages.push('observing');
                const obs = `${tool} returned ${2 + i} result(s).`;
                const oIn = approxTokens(obs) + 40, oOut = approxTokens(obs) + 20;
                totalIn += oIn; totalOut += oOut;
                emit('tool_result', 'observing', { tool, output: obs }, oIn + oOut);
                await delay(50);
            }
            if (!loopCount || i >= maxIter - 2) break;
        }

        stages.push('synthesizing');
        emit('stage_change', 'synthesizing', { message: 'Composing final answer' });
        const synth = `[Simulated] Task complete. Model: ${model}. Tool calls: ${toolCalls}. Loops: ${loopCount}.`;
        const sIn = approxTokens(synth) + 100, sOut = approxTokens(synth) + 180;
        totalIn += sIn; totalOut += sOut;
        await delay(120);

        stages.push('done');
        emit('completed', 'done', { output: synth, totalIn, totalOut });

        return { agentId, taskId, output: synth, stages, totalInputTokens: totalIn, totalOutputTokens: totalOut, toolCallCount: toolCalls, loopCount, durationMs: Date.now() - start, success: true };
    }
}

export const globalRuntime = new AgentRuntimeEngine();
