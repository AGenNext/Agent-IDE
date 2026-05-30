import { v4 as uuidv4 } from 'uuid';
import { RunRequest, RunRecord, TraceStepRecord, OAIMessage, OAITool, OAIResponse } from './types';
import { runStore } from './run-store';
import { invokeTool } from './tool-proxy';
import { broadcast } from './websocket';
import { ingestText } from './knowledge-store';
import { mcpManager } from './mcp-manager';

// Cost per 1K tokens (input/output) — Anthropic + OpenAI pricing as of 2026-05
const MODEL_COST: Record<string, { in: number; out: number }> = {
    'claude-opus-4-8':    { in: 0.015,   out: 0.075   },
    'claude-sonnet-4-6':  { in: 0.003,   out: 0.015   },
    'claude-haiku-4-5':   { in: 0.00025, out: 0.00125 },
    'gpt-4o':             { in: 0.005,   out: 0.015   },
    'gpt-4o-mini':        { in: 0.00015, out: 0.0006  },
    'gemini-1.5-pro':     { in: 0.00125, out: 0.005   },
};

function costUsd(model: string, inputTokens: number, outputTokens: number): number {
    const c = MODEL_COST[model] ?? { in: 0.003, out: 0.015 };
    return (inputTokens / 1000) * c.in + (outputTokens / 1000) * c.out;
}

// Build OpenAI-compatible tool schemas for the requested tools
function buildToolSchemas(toolIds: string[]): OAITool[] {
    const SCHEMAS: Record<string, OAITool> = {
        file_rw: {
            type: 'function',
            function: {
                name: 'file_rw',
                description: 'Read, write, list, or delete workspace files.',
                parameters: {
                    type: 'object',
                    properties: {
                        operation: { type: 'string', enum: ['read', 'write', 'list', 'delete'], description: 'Operation to perform' },
                        path:      { type: 'string', description: 'Relative path within workspace' },
                        content:   { type: 'string', description: 'Content to write (write operation only)' },
                    },
                    required: ['operation', 'path'],
                },
            },
        },
        shell: {
            type: 'function',
            function: {
                name: 'shell',
                description: 'Execute a shell command in the workspace root (sandboxed).',
                parameters: {
                    type: 'object',
                    properties: {
                        command: { type: 'string', description: 'Shell command to run' },
                        timeout: { type: 'number', description: 'Timeout in ms (default 10000)' },
                    },
                    required: ['command'],
                },
            },
        },
        http_client: {
            type: 'function',
            function: {
                name: 'http_client',
                description: 'Make an outbound HTTP request.',
                parameters: {
                    type: 'object',
                    properties: {
                        method:  { type: 'string', description: 'HTTP method (GET, POST, …)' },
                        url:     { type: 'string', description: 'Full URL' },
                        body:    { type: 'string', description: 'Request body (optional)' },
                        headers: { type: 'object', description: 'Extra headers as key-value map (optional)' },
                    },
                    required: ['method', 'url'],
                },
            },
        },
        browser: {
            type: 'function',
            function: {
                name: 'browser',
                description: 'Fetch a URL and extract page text content.',
                parameters: {
                    type: 'object',
                    properties: { url: { type: 'string', description: 'URL to fetch' } },
                    required: ['url'],
                },
            },
        },
        web_search: {
            type: 'function',
            function: {
                name: 'web_search',
                description: 'Search the web using the Brave Search API.',
                parameters: {
                    type: 'object',
                    properties: {
                        query: { type: 'string', description: 'Search query' },
                        limit: { type: 'number', description: 'Max results (default 5)' },
                    },
                    required: ['query'],
                },
            },
        },
        vector_search: {
            type: 'function',
            function: {
                name: 'vector_search',
                description: 'Semantic search over the knowledge base using cosine similarity.',
                parameters: {
                    type: 'object',
                    properties: {
                        query: { type: 'string', description: 'Search query' },
                        topK:  { type: 'number', description: 'Number of results (default 3)' },
                    },
                    required: ['query'],
                },
            },
        },
        code_exec: {
            type: 'function',
            function: {
                name: 'code_exec',
                description: 'Execute Python or JavaScript code and return stdout/stderr.',
                parameters: {
                    type: 'object',
                    properties: {
                        language: { type: 'string', enum: ['python', 'javascript'], description: 'Language to run' },
                        code:     { type: 'string', description: 'Code to execute' },
                    },
                    required: ['language', 'code'],
                },
            },
        },
        db_query: {
            type: 'function',
            function: {
                name: 'db_query',
                description: 'Execute a SQL query against the configured database.',
                parameters: {
                    type: 'object',
                    properties: {
                        query:    { type: 'string', description: 'SQL query' },
                        database: { type: 'string', description: 'Database name or connection alias' },
                    },
                    required: ['query', 'database'],
                },
            },
        },
        repo_index: {
            type: 'function',
            function: {
                name: 'repo_index',
                description: 'Walk a local repository directory and ingest all source files into the knowledge store for cheap semantic search. Call this once at the start of a coding task, then use vector_search to find relevant code.',
                parameters: {
                    type: 'object',
                    properties: {
                        path:        { type: 'string', description: 'Relative path to the repo root (default ".")' },
                        extensions:  { type: 'array', items: { type: 'string' }, description: 'File extensions to index (default: .ts .tsx .js .py .go .rs .md)' },
                        maxFileSize: { type: 'number', description: 'Max file size in bytes to ingest (default 102400)' },
                    },
                    required: [],
                },
            },
        },
        openhands_task: {
            type: 'function',
            function: {
                name: 'openhands_task',
                description: 'Run a software development task using the OpenHandS AI agent (requires OPENHANDS_URL env var pointing to a running OpenHandS instance).',
                parameters: {
                    type: 'object',
                    properties: {
                        task:  { type: 'string', description: 'The development task to execute (bug fix, feature, refactor, etc.)' },
                        agent: { type: 'string', description: 'OpenHandS agent to use (default: CodeActAgent)' },
                    },
                    required: ['task'],
                },
            },
        },
    };

    // Built-in tools requested by the agent config
    const builtIn = toolIds.map(id => SCHEMAS[id]).filter(Boolean) as OAITool[];

    // MCP tools from all currently connected servers — namespaced as mcp__<serverId>__<toolName>
    const mcpTools: OAITool[] = mcpManager.getAllTools().map(t => ({
        type: 'function',
        function: {
            name: `mcp__${t.serverId}__${t.name}`,
            description: `[MCP:${t.serverName}] ${t.description}`,
            parameters: (t.inputSchema as OAITool['function']['parameters']) ?? { type: 'object', properties: {}, required: [] },
        },
    }));

    return [...builtIn, ...mcpTools];
}

// Determine base URL from model name
function apiBase(model: string, apiKey: string): { url: string; key: string } {
    if (model.startsWith('claude-')) {
        return { url: 'https://api.anthropic.com/v1', key: apiKey };
    }
    if (model.startsWith('gemini-')) {
        return { url: 'https://generativelanguage.googleapis.com/v1beta/openai', key: apiKey };
    }
    // Ollama / vLLM / any OpenAI-compatible local endpoint
    const ollamaUrl = process.env.OLLAMA_BASE_URL;
    if (ollamaUrl && !model.startsWith('gpt-')) {
        return { url: ollamaUrl, key: apiKey || 'ollama' };
    }
    const openAiBase = process.env.OPENAI_BASE_URL ?? 'https://api.openai.com/v1';
    return { url: openAiBase, key: apiKey };
}

export async function callLLM(
    model: string, apiKey: string,
    messages: OAIMessage[], tools: OAITool[],
    temperature: number, maxTokens: number,
): Promise<OAIResponse> {
    const { url, key } = apiBase(model, apiKey);

    // Anthropic models need the Anthropic-Beta header and slightly different schema;
    // we use their OpenAI-compatible endpoint (messages API) as a best-effort.
    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${key}`,
    };
    if (model.startsWith('claude-')) {
        headers['anthropic-version'] = '2023-06-01';
        headers['anthropic-beta'] = 'tools-2024-04-04';
    }

    const body: Record<string, unknown> = {
        model,
        messages,
        temperature,
        max_tokens: maxTokens,
    };
    if (tools.length > 0) body['tools'] = tools;

    const endpoint = model.startsWith('claude-') ? `${url}/messages` : `${url}/chat/completions`;
    const res = await fetch(endpoint, { method: 'POST', headers, body: JSON.stringify(body) });

    if (!res.ok) {
        const text = await res.text();
        throw new Error(`LLM API error ${res.status}: ${text.slice(0, 400)}`);
    }

    const data = await res.json() as OAIResponse;

    // Normalize Anthropic response → OpenAI shape
    if (model.startsWith('claude-')) {
        const raw = data as unknown as {
            content: Array<{ type: string; text?: string; id?: string; name?: string; input?: unknown }>;
            stop_reason: string;
            usage: { input_tokens: number; output_tokens: number };
        };
        const textBlock = raw.content.find(b => b.type === 'text');
        const toolBlocks = raw.content.filter(b => b.type === 'tool_use');
        return {
            id: uuidv4(),
            choices: [{
                message: {
                    role: 'assistant',
                    content: textBlock?.text ?? null,
                    tool_calls: toolBlocks.length > 0 ? toolBlocks.map(b => ({
                        id: b.id ?? uuidv4(),
                        type: 'function' as const,
                        function: { name: b.name ?? '', arguments: JSON.stringify(b.input ?? {}) },
                    })) : undefined,
                },
                finish_reason: raw.stop_reason === 'tool_use' ? 'tool_calls' : 'stop',
            }],
            usage: { prompt_tokens: raw.usage.input_tokens, completion_tokens: raw.usage.output_tokens },
        };
    }

    return data;
}

// ─── Main agent loop ──────────────────────────────────────────────────────────

export async function startAgentRun(req: RunRequest): Promise<string> {
    const runId = uuidv4();
    const now = new Date().toISOString();
    const apiKey = req.apiKey ?? process.env.OPENAI_API_KEY ?? process.env.ANTHROPIC_API_KEY ?? '';
    const maxIterations = req.maxIterations ?? 10;
    const temperature = req.temperature ?? 0.7;
    const maxTokens = req.maxTokens ?? 4096;

    const record: RunRecord = {
        runId, agentId: req.agentId, agentName: req.agentName,
        model: req.model, task: req.task,
        status: 'running', startedAt: now,
        steps: [], totalInputTokens: 0, totalOutputTokens: 0, estimatedCostUsd: 0,
    };
    runStore.create(record);
    broadcast({ type: 'run:started', runId, payload: record });

    // Run asynchronously — don't await the caller
    (async () => {
        const tools = buildToolSchemas(req.tools);
        const messages: OAIMessage[] = [
            { role: 'system', content: req.systemPrompt },
            { role: 'user',   content: `Task: ${req.task}` },
        ];

        let sequence = 0;
        let loopIndex = 0;
        let loopDetect = 0; // count of consecutive tool calls without a stop

        function emitStep(step: Omit<TraceStepRecord, 'timestamp'>): TraceStepRecord {
            const full: TraceStepRecord = { ...step, timestamp: new Date().toISOString() };
            runStore.appendStep(runId, full);
            broadcast({ type: 'run:step', runId, payload: full });
            return full;
        }

        try {
            for (let iter = 0; iter < maxIterations; iter++) {
                const t0 = Date.now();
                let response: OAIResponse;

                if (!apiKey) {
                    // Offline/demo mode — synthesize a plausible trace without an API call
                    response = syntheticResponse(req.task, req.tools, iter);
                } else {
                    response = await callLLM(req.model, apiKey, messages, tools, temperature, maxTokens);
                }

                const usage = response.usage ?? { prompt_tokens: 300, completion_tokens: 100 };
                const choice = response.choices[0];
                const msg = choice.message;

                // 1. Thought / text content
                if (msg.content) {
                    sequence++;
                    emitStep({
                        sequence, type: 'thought', content: msg.content,
                        inputTokens: usage.prompt_tokens,
                        outputTokens: usage.completion_tokens,
                        durationMs: Date.now() - t0,
                        loopIndex: loopDetect > 0 ? loopIndex : undefined,
                    });
                }

                // 2. Tool calls
                if (msg.tool_calls && msg.tool_calls.length > 0) {
                    loopDetect++;
                    if (loopDetect > 1) loopIndex++;

                    messages.push({ role: 'assistant', content: msg.content ?? null, tool_calls: msg.tool_calls });

                    for (const tc of msg.tool_calls) {
                        const toolInput = JSON.parse(tc.function.arguments || '{}') as Record<string, unknown>;
                        const actionStart = Date.now();
                        sequence++;
                        emitStep({
                            sequence, type: 'action',
                            content: `Calling ${tc.function.name}(${tc.function.arguments.slice(0, 120)})`,
                            toolName: tc.function.name, toolInput,
                            inputTokens: 0, outputTokens: 0,
                            durationMs: 0,
                            loopIndex: loopDetect > 1 ? loopIndex : undefined,
                        });

                        let result: { success: boolean; output?: unknown; error?: string };
                        if (tc.function.name.startsWith('mcp__')) {
                            // Route through MCP manager: mcp__<serverId>__<toolName>
                            const parts = tc.function.name.split('__');
                            const serverId = parts[1];
                            const toolName = parts.slice(2).join('__');
                            try {
                                const output = await mcpManager.callTool(serverId, toolName, toolInput);
                                result = { success: true, output };
                            } catch (err: unknown) {
                                result = { success: false, error: err instanceof Error ? err.message : String(err) };
                            }
                        } else {
                            result = await invokeTool({ toolId: tc.function.name, input: toolInput, agentId: req.agentId, runId });
                        }
                        const obsContent = result.success
                            ? JSON.stringify(result.output, null, 2).slice(0, 2000)
                            : `Error: ${result.error}`;

                        sequence++;
                        emitStep({
                            sequence, type: result.success ? 'observation' : 'error',
                            content: obsContent,
                            toolName: tc.function.name, toolOutput: result.output,
                            inputTokens: 0, outputTokens: 0,
                            durationMs: Date.now() - actionStart,
                            loopIndex: loopDetect > 1 ? loopIndex : undefined,
                        });

                        messages.push({ role: 'tool', tool_call_id: tc.id, name: tc.function.name, content: obsContent });
                    }
                    continue; // iterate again with tool results
                }

                // 3. Final answer (no more tool calls)
                loopDetect = 0;
                sequence++;
                emitStep({
                    sequence, type: 'result', content: msg.content ?? '(no output)',
                    inputTokens: usage.prompt_tokens,
                    outputTokens: usage.completion_tokens,
                    durationMs: Date.now() - t0,
                });
                break;
            }

            const run = runStore.get(runId)!;
            runStore.setStatus(runId, 'completed', {
                estimatedCostUsd: costUsd(req.model, run.totalInputTokens, run.totalOutputTokens),
            });
            broadcast({ type: 'run:completed', runId, payload: runStore.get(runId)! });

            // Long-term agent memory: persist final result as a knowledge chunk
            const finalStep = run.steps.findLast(s => s.type === 'result');
            if (finalStep?.content) {
                const tenantId = req.agentId.startsWith('user_') ? req.agentId : 'user_demo';
                ingestText(tenantId, `Run: ${req.task.slice(0, 80)}`, String(finalStep.content), `run:${runId}`, {
                    runId, agentName: req.agentName, model: req.model,
                }).catch(() => { /* non-blocking */ });
            }

        } catch (err: unknown) {
            const msg = err instanceof Error ? err.message : String(err);
            runStore.setStatus(runId, 'failed', { errorMessage: msg });
            broadcast({ type: 'run:failed', runId, payload: { error: msg } });
        }
    })();

    return runId;
}

// ─── Offline demo — synthetic trace without an API key ────────────────────────

function syntheticResponse(task: string, toolIds: string[], iter: number): OAIResponse {
    const usage = { prompt_tokens: 320 + iter * 80, completion_tokens: 120 + iter * 40 };

    if (iter === 0 && toolIds.length > 0) {
        return {
            id: uuidv4(), usage,
            choices: [{
                message: {
                    role: 'assistant', content: `Analyzing task: "${task}". I will use available tools to gather context before producing a final answer.`,
                    tool_calls: [{
                        id: uuidv4(), type: 'function',
                        function: { name: toolIds[0], arguments: JSON.stringify({ query: task.slice(0, 80) }) },
                    }],
                },
                finish_reason: 'tool_calls',
            }],
        };
    }

    if (iter === 1 && toolIds.length > 1) {
        return {
            id: uuidv4(), usage,
            choices: [{
                message: {
                    role: 'assistant', content: 'Initial results are promising. Running a refinement step to improve accuracy.',
                    tool_calls: [{
                        id: uuidv4(), type: 'function',
                        function: { name: toolIds[1] ?? toolIds[0], arguments: JSON.stringify({ query: `refined: ${task.slice(0, 60)}`, topK: 5 }) },
                    }],
                },
                finish_reason: 'tool_calls',
            }],
        };
    }

    return {
        id: uuidv4(), usage,
        choices: [{
            message: {
                role: 'assistant',
                content: `Task completed (offline/demo mode — no API key configured).\n\nTask: "${task}"\n\nConclusion: Based on available context from the knowledge base and tool results, the task has been processed through ${iter + 1} reasoning iteration(s). In live mode with a valid API key, the agent would provide a substantive answer grounded in real tool outputs.`,
            },
            finish_reason: 'stop',
        }],
    };
}
