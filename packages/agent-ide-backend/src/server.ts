import * as http from 'http';
import express, { Request, Response, NextFunction } from 'express';
import cors from 'cors';
import { v4 as uuidv4 } from 'uuid';
import { attachWebSocket } from './websocket';
import { startAgentRun } from './agent-loop';
import { invokeTool } from './tool-proxy';
import { runStore } from './run-store';
import { mcpManager } from './mcp-manager';
import { RunRequest, ToolInvokeRequest } from './types';

const app = express();
const PORT = Number(process.env.PORT ?? 3001);

// ─── Middleware ───────────────────────────────────────────────────────────────

app.use(cors({ origin: '*' }));
app.use(express.json({ limit: '2mb' }));

// Request logger
app.use((req: Request, _res: Response, next: NextFunction) => {
    console.log(`${new Date().toISOString()} ${req.method} ${req.path}`);
    next();
});

// ─── Health ───────────────────────────────────────────────────────────────────

app.get('/health', (_req, res) => {
    res.json({
        status: 'ok',
        version: '0.1.0',
        uptime: process.uptime(),
        ts: new Date().toISOString(),
    });
});

// ─── Runs ─────────────────────────────────────────────────────────────────────

// POST /api/runs — submit a new agent run
app.post('/api/runs', async (req: Request, res: Response) => {
    const body = req.body as Partial<RunRequest>;

    if (!body.task || !body.model) {
        res.status(400).json({ error: 'task and model are required' });
        return;
    }

    const runReq: RunRequest = {
        agentId:      body.agentId     ?? uuidv4(),
        agentName:    body.agentName   ?? 'Agent',
        model:        body.model,
        systemPrompt: body.systemPrompt ?? 'You are a helpful assistant. Break tasks into steps, use available tools, and produce clear outputs.',
        task:         body.task,
        tools:        body.tools       ?? [],
        maxIterations: body.maxIterations ?? 10,
        apiKey:       body.apiKey,
        temperature:  body.temperature  ?? 0.7,
        maxTokens:    body.maxTokens    ?? 4096,
    };

    const runId = await startAgentRun(runReq);
    res.status(202).json({ runId, status: 'running', wsUrl: `/ws/${runId}` });
});

// GET /api/runs — list recent runs
app.get('/api/runs', (_req: Request, res: Response) => {
    res.json(runStore.list().map(r => ({
        runId: r.runId,
        agentId: r.agentId,
        agentName: r.agentName,
        model: r.model,
        task: r.task,
        status: r.status,
        startedAt: r.startedAt,
        completedAt: r.completedAt,
        stepCount: r.steps.length,
        totalInputTokens: r.totalInputTokens,
        totalOutputTokens: r.totalOutputTokens,
        estimatedCostUsd: r.estimatedCostUsd,
    })));
});

// GET /api/runs/:id — get a single run (full trace)
app.get('/api/runs/:id', (req: Request, res: Response) => {
    const run = runStore.get(req.params['id'] ?? '');
    if (!run) { res.status(404).json({ error: 'Run not found' }); return; }
    res.json(run);
});

// DELETE /api/runs/:id — cancel a running run
app.delete('/api/runs/:id', (req: Request, res: Response) => {
    const cancelled = runStore.cancel(req.params['id'] ?? '');
    if (!cancelled) { res.status(404).json({ error: 'Run not found or already finished' }); return; }
    res.json({ cancelled: true });
});

// ─── Tools ────────────────────────────────────────────────────────────────────

// GET /api/tools — list all available tools
app.get('/api/tools', (_req: Request, res: Response) => {
    res.json([
        { id: 'file_rw',       name: 'File R/W',       category: 'file',   browserNative: false, description: 'Read, write, list, or delete workspace files.' },
        { id: 'shell',         name: 'Shell',           category: 'code',   browserNative: false, description: 'Execute shell commands (requires ALLOW_SHELL=true).' },
        { id: 'http_client',   name: 'HTTP Client',     category: 'api',    browserNative: true,  description: 'Make outbound HTTP requests.' },
        { id: 'browser',       name: 'Browser',         category: 'web',    browserNative: false, description: 'Fetch a URL and extract page text.' },
        { id: 'web_search',    name: 'Web Search',      category: 'web',    browserNative: false, description: 'Search the web via Brave Search API (requires BRAVE_API_KEY).' },
        { id: 'vector_search', name: 'Vector Search',   category: 'memory', browserNative: true,  description: 'In-memory semantic search using cosine similarity.' },
        { id: 'code_exec',     name: 'Code Executor',   category: 'code',   browserNative: false, description: 'Execute Python or JavaScript (requires ALLOW_SHELL=true).' },
        { id: 'db_query',      name: 'DB Query',        category: 'data',   browserNative: false, description: 'Execute SQL against the configured database (requires DATABASE_URL).' },
    ]);
});

// POST /api/tools/:id/invoke — invoke a tool directly
app.post('/api/tools/:id/invoke', async (req: Request, res: Response) => {
    const toolId = req.params['id'] ?? '';
    const body = req.body as Partial<ToolInvokeRequest>;

    const result = await invokeTool({
        toolId,
        input: body.input ?? {},
        agentId: body.agentId,
        runId: body.runId,
    });

    res.status(result.success ? 200 : 500).json(result);
});

// ─── MCP servers ─────────────────────────────────────────────────────────────

// GET /api/mcp/servers — list configured servers with live status
app.get('/api/mcp/servers', (_req, res) => {
    res.json(mcpManager.listServers());
});

// GET /api/mcp/servers/:id — single server state
app.get('/api/mcp/servers/:id', (req: Request, res: Response) => {
    const s = mcpManager.getServer(req.params['id'] ?? '');
    if (!s) { res.status(404).json({ error: 'Server not found' }); return; }
    res.json(s);
});

// POST /api/mcp/servers — add / upsert a server config
app.post('/api/mcp/servers', async (req: Request, res: Response) => {
    const body = req.body as { id?: string; name?: string; transport?: string; command?: string; endpoint?: string; env?: Record<string, string> };
    if (!body.id || !body.name || !body.transport) {
        res.status(400).json({ error: 'id, name, and transport are required' }); return;
    }
    await mcpManager.addServer({
        id: body.id, name: body.name,
        transport: body.transport as 'stdio' | 'sse' | 'websocket',
        command: body.command, endpoint: body.endpoint,
        env: body.env ?? {},
    });
    res.status(201).json(mcpManager.getServer(body.id));
});

// DELETE /api/mcp/servers/:id — remove server and disconnect
app.delete('/api/mcp/servers/:id', async (req: Request, res: Response) => {
    await mcpManager.removeServer(req.params['id'] ?? '');
    res.json({ removed: true });
});

// POST /api/mcp/servers/:id/connect — connect to an MCP server
app.post('/api/mcp/servers/:id/connect', async (req: Request, res: Response) => {
    try {
        const state = await mcpManager.connect(req.params['id'] ?? '');
        res.json(state);
    } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        res.status(500).json({ error: msg });
    }
});

// POST /api/mcp/servers/:id/disconnect — disconnect from an MCP server
app.post('/api/mcp/servers/:id/disconnect', async (req: Request, res: Response) => {
    await mcpManager.disconnect(req.params['id'] ?? '');
    res.json({ disconnected: true });
});

// GET /api/mcp/tools — all tools from all connected MCP servers
app.get('/api/mcp/tools', (_req, res) => {
    res.json(mcpManager.getAllTools());
});

// POST /api/mcp/tools/:serverId/:toolName/call — call a tool on a connected server
app.post('/api/mcp/tools/:serverId/:toolName/call', async (req: Request, res: Response) => {
    const { serverId = '', toolName = '' } = req.params;
    const args = (req.body as { args?: Record<string, unknown> }).args ?? req.body as Record<string, unknown>;
    try {
        const result = await mcpManager.callTool(serverId, toolName, args);
        res.json({ success: true, result });
    } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        res.status(500).json({ success: false, error: msg });
    }
});

// ─── Config/info ──────────────────────────────────────────────────────────────

app.get('/api/config', (_req, res) => {
    const mcpServers = mcpManager.listServers();
    res.json({
        workspaceRoot:   process.env.WORKSPACE_ROOT ?? process.cwd(),
        allowShell:      process.env.ALLOW_SHELL === 'true',
        hasBraveKey:     Boolean(process.env.BRAVE_API_KEY),
        hasOpenAiKey:    Boolean(process.env.OPENAI_API_KEY),
        hasAnthropicKey: Boolean(process.env.ANTHROPIC_API_KEY),
        hasDatabaseUrl:  Boolean(process.env.DATABASE_URL),
        port:            PORT,
        mcpServerCount:  mcpServers.length,
        mcpConnected:    mcpServers.filter(s => s.status === 'connected').length,
    });
});

// ─── 404 fallback ─────────────────────────────────────────────────────────────

app.use((_req: Request, res: Response) => {
    res.status(404).json({ error: 'Not found' });
});

// ─── Start ────────────────────────────────────────────────────────────────────

const server = http.createServer(app);
attachWebSocket(server);

server.listen(PORT, async () => {
    console.log(`Agent IDE backend listening on port ${PORT}`);
    console.log(`  WebSocket:  ws://localhost:${PORT}/ws`);
    console.log(`  Health:     http://localhost:${PORT}/health`);
    console.log(`  Runs API:   http://localhost:${PORT}/api/runs`);
    console.log(`  Tools API:  http://localhost:${PORT}/api/tools`);
    console.log(`  MCP API:    http://localhost:${PORT}/api/mcp/servers`);
    const apiKeySet = process.env.OPENAI_API_KEY || process.env.ANTHROPIC_API_KEY;
    console.log(`  Live LLM:   ${apiKeySet ? 'enabled' : 'offline/demo mode (no API key)'}`);
    console.log(`  Shell:      ${process.env.ALLOW_SHELL === 'true' ? 'enabled' : 'disabled'}`);

    // Load MCP server configs (reads .mcp.json, writes defaults if absent)
    await mcpManager.loadConfig();
    const mcpServers = mcpManager.listServers();
    console.log(`  MCP servers: ${mcpServers.length} configured`);
});

// Prune old runs every 30 minutes
setInterval(() => runStore.prune(), 30 * 60 * 1000);

export { app, server };
