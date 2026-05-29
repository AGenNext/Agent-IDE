export type ToolCategory = 'web' | 'code' | 'data' | 'file' | 'api' | 'memory';

export interface ToolDefinition {
    id: string;
    name: string;
    description: string;
    category: ToolCategory;
    inputSchema: Record<string, { type: string; description: string; required?: boolean }>;
    mockExecute(input: Record<string, unknown>): Promise<unknown>;
}

const REGISTRY: ToolDefinition[] = [
    {
        id: 'browser', name: 'Browser', category: 'web',
        description: 'Navigate URLs, extract text and structured content',
        inputSchema: { url: { type: 'string', description: 'URL to fetch', required: true } },
        async mockExecute(i) { return { title: 'Mock page', text: `Fetched ${i['url']}. Found 3 relevant sections.`, links: [] }; },
    },
    {
        id: 'web_search', name: 'Web Search', category: 'web',
        description: 'Search the web, return ranked results with snippets',
        inputSchema: { query: { type: 'string', required: true }, limit: { type: 'number', description: 'Max results' } },
        async mockExecute(i) { return { results: [{ title: `Result for: ${i['query']}`, snippet: 'Relevant snippet.', url: 'https://example.com' }] }; },
    },
    {
        id: 'code_exec', name: 'Code Executor', category: 'code',
        description: 'Execute Python/JS in a sandboxed environment',
        inputSchema: { language: { type: 'string', required: true }, code: { type: 'string', required: true } },
        async mockExecute(i) { return { stdout: `[mock] Ran ${i['language']} code`, stderr: '', exitCode: 0 }; },
    },
    {
        id: 'file_rw', name: 'File R/W', category: 'file',
        description: 'Read and write files in the workspace',
        inputSchema: {
            operation: { type: 'string', required: true },
            path: { type: 'string', required: true },
            content: { type: 'string' },
        },
        async mockExecute(i) {
            return i['operation'] === 'read'
                ? { content: `[mock content of ${i['path']}]` }
                : { success: true, path: i['path'] };
        },
    },
    {
        id: 'http_client', name: 'HTTP Client', category: 'api',
        description: 'Make outbound HTTP requests to external APIs',
        inputSchema: { method: { type: 'string', required: true }, url: { type: 'string', required: true }, body: { type: 'string' } },
        async mockExecute(i) { return { status: 200, body: `[mock] Response from ${i['url']}` }; },
    },
    {
        id: 'vector_search', name: 'Vector Search', category: 'memory',
        description: 'Semantic search over knowledge base embeddings',
        inputSchema: { query: { type: 'string', required: true }, topK: { type: 'number' } },
        async mockExecute(i) { return { matches: [{ id: 'chunk-1', score: 0.92, content: `Top chunk for: ${i['query']}` }] }; },
    },
    {
        id: 'db_query', name: 'DB Query', category: 'data',
        description: 'Query structured databases (SQL or NoSQL)',
        inputSchema: { query: { type: 'string', required: true }, database: { type: 'string', required: true } },
        async mockExecute(i) { return { rows: [{ id: 1, result: `[mock] ${i['query']}` }], rowCount: 1 }; },
    },
    {
        id: 'shell', name: 'Shell', category: 'code',
        description: 'Run shell commands in a sandboxed container',
        inputSchema: { command: { type: 'string', required: true } },
        async mockExecute(i) { return { stdout: `[mock] ${i['command']}`, stderr: '', exitCode: 0 }; },
    },
];

export function getToolById(id: string): ToolDefinition | undefined {
    return REGISTRY.find(t => t.id === id);
}

export function getAllTools(): ToolDefinition[] {
    return [...REGISTRY];
}

export function getToolsByCategory(cat: ToolCategory): ToolDefinition[] {
    return REGISTRY.filter(t => t.category === cat);
}
