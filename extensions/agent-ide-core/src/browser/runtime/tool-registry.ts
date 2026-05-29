export type ToolCategory = 'web' | 'code' | 'data' | 'file' | 'api' | 'memory';

export interface ToolDefinition {
    id: string;
    name: string;
    description: string;
    category: ToolCategory;
    /** Whether this tool can execute in the browser without a backend */
    browserNative: boolean;
    inputSchema: Record<string, { type: string; description: string; required?: boolean }>;
    mockExecute(input: Record<string, unknown>): Promise<unknown>;
}

/**
 * In-memory vector store for browser-native vector_search.
 * Cosine similarity: sim(A,B) = A·B / (|A||B|)
 * Source: standard cosine similarity definition
 */
const EMBEDDED_CHUNKS: { id: string; text: string; vec: number[] }[] = [
    { id: 'c1', text: 'AgentBench evaluates LLMs as agents across 8 interactive environments.', vec: [0.8, 0.6, 0.2, 0.1, 0.9, 0.3, 0.4, 0.7] },
    { id: 'c2', text: 'ReAct interleaves reasoning and acting for multi-step problem solving.',    vec: [0.7, 0.5, 0.3, 0.2, 0.8, 0.4, 0.6, 0.5] },
    { id: 'c3', text: 'Prompt caching reduces repeated context tokens to 10% of base cost.',       vec: [0.3, 0.2, 0.9, 0.8, 0.1, 0.7, 0.2, 0.4] },
    { id: 'c4', text: 'Tool use enables agents to call external APIs and execute code.',          vec: [0.5, 0.8, 0.4, 0.3, 0.6, 0.9, 0.7, 0.2] },
];

function cosineSim(a: number[], b: number[]): number {
    const dot = a.reduce((s, v, i) => s + v * b[i], 0);
    const na = Math.sqrt(a.reduce((s, v) => s + v * v, 0));
    const nb = Math.sqrt(b.reduce((s, v) => s + v * v, 0));
    return na && nb ? dot / (na * nb) : 0;
}

function queryVec(query: string): number[] {
    // Deterministic pseudo-embedding from query string hash
    const h = Array.from(query).reduce((s, c) => s + c.charCodeAt(0), 0);
    return Array.from({ length: 8 }, (_, i) => Math.abs(Math.sin(h * (i + 1))) );
}

const REGISTRY: ToolDefinition[] = [
    {
        id: 'browser', name: 'Browser', category: 'web', browserNative: false,
        description: 'Navigate URLs, extract text and structured content (requires backend CORS proxy)',
        inputSchema: { url: { type: 'string', description: 'URL to fetch', required: true } },
        async mockExecute(i) { return { title: 'Mock page', text: `Fetched ${i['url']}. Found 3 sections.`, links: [] }; },
    },
    {
        id: 'web_search', name: 'Web Search', category: 'web', browserNative: false,
        description: 'Search the web via Search API (Brave/SerpAPI — needs backend key)',
        inputSchema: { query: { type: 'string', required: true }, limit: { type: 'number', description: 'Max results' } },
        async mockExecute(i) { return { results: [{ title: `Top result for: ${i['query']}`, snippet: 'Relevant finding.', url: 'https://example.com' }] }; },
    },
    {
        id: 'code_exec', name: 'Code Executor', category: 'code', browserNative: false,
        description: 'Execute Python/JS in sandboxed container (requires backend sandbox)',
        inputSchema: { language: { type: 'string', required: true }, code: { type: 'string', required: true } },
        async mockExecute(i) { return { stdout: `[mock] ran ${i['language']}`, stderr: '', exitCode: 0 }; },
    },
    {
        id: 'file_rw', name: 'File R/W', category: 'file', browserNative: false,
        description: 'Read/write workspace files (requires Theia filesystem service)',
        inputSchema: { operation: { type: 'string', required: true }, path: { type: 'string', required: true }, content: { type: 'string' } },
        async mockExecute(i) {
            return i['operation'] === 'read' ? { content: `[mock content of ${i['path']}]` } : { success: true };
        },
    },
    {
        id: 'http_client', name: 'HTTP Client', category: 'api', browserNative: true,
        description: 'Make outbound HTTP requests (runs natively in browser via fetch)',
        inputSchema: { method: { type: 'string', required: true }, url: { type: 'string', required: true }, body: { type: 'string' }, headers: { type: 'string' } },
        async mockExecute(i) {
            // Real implementation: use fetch() — runs in browser (subject to CORS)
            if (typeof fetch !== 'undefined' && i['url'] && !String(i['url']).includes('localhost')) {
                try {
                    const opts: RequestInit = { method: String(i['method'] ?? 'GET') };
                    if (i['body']) opts.body = String(i['body']);
                    const res = await fetch(String(i['url']), opts);
                    const text = await res.text();
                    return { status: res.status, body: text.slice(0, 2000) };
                } catch (e) { return { error: String(e) }; }
            }
            return { status: 200, body: `[mock] Response from ${i['url']}` };
        },
    },
    {
        id: 'vector_search', name: 'Vector Search', category: 'memory', browserNative: true,
        description: 'In-memory semantic search using cosine similarity (runs in browser)',
        inputSchema: { query: { type: 'string', required: true }, topK: { type: 'number' } },
        async mockExecute(i) {
            // Cosine similarity against embedded chunks
            // sim(A,B) = A·B / (|A||B|) — standard cosine similarity
            const qv = queryVec(String(i['query']));
            const k = Number(i['topK'] ?? 3);
            const scored = EMBEDDED_CHUNKS.map(c => ({ ...c, score: cosineSim(qv, c.vec) }));
            scored.sort((a, b) => b.score - a.score);
            return { matches: scored.slice(0, k).map(c => ({ id: c.id, score: parseFloat(c.score.toFixed(3)), content: c.text })) };
        },
    },
    {
        id: 'db_query', name: 'DB Query', category: 'data', browserNative: false,
        description: 'Query structured databases (requires backend DB connection)',
        inputSchema: { query: { type: 'string', required: true }, database: { type: 'string', required: true } },
        async mockExecute(i) { return { rows: [{ id: 1, result: `[mock] ${i['query']}` }], rowCount: 1 }; },
    },
    {
        id: 'shell', name: 'Shell', category: 'code', browserNative: false,
        description: 'Shell commands in sandboxed container (requires backend sandbox)',
        inputSchema: { command: { type: 'string', required: true } },
        async mockExecute(i) { return { stdout: `[mock] ${i['command']}`, stderr: '', exitCode: 0 }; },
    },
];

/** Convert a ToolDefinition to OpenAI function-calling schema */
export function toOpenAITool(tool: ToolDefinition): import('./agent-runtime').OAITool extends never ? never : {
    type: 'function';
    function: { name: string; description: string; parameters: { type: 'object'; properties: Record<string, unknown>; required: string[] } };
} {
    return {
        type: 'function',
        function: {
            name: tool.id,
            description: tool.description,
            parameters: {
                type: 'object',
                properties: Object.fromEntries(
                    Object.entries(tool.inputSchema).map(([k, v]) => [k, { type: v.type, description: v.description }])
                ),
                required: Object.entries(tool.inputSchema).filter(([, v]) => v.required).map(([k]) => k),
            },
        },
    } as any;
}

export function getToolById(id: string): ToolDefinition | undefined { return REGISTRY.find(t => t.id === id); }
export function getAllTools(): ToolDefinition[] { return [...REGISTRY]; }
export function getToolsByCategory(cat: ToolCategory): ToolDefinition[] { return REGISTRY.filter(t => t.category === cat); }
export function getBrowserNativeTools(): ToolDefinition[] { return REGISTRY.filter(t => t.browserNative); }
