import * as http from 'http';
import express, { Request, Response, NextFunction } from 'express';
import cors from 'cors';
import { v4 as uuidv4 } from 'uuid';
import { attachWebSocket } from './websocket';
import { startAgentRun } from './agent-loop';
import { invokeTool } from './tool-proxy';
import { runStore } from './run-store';
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

// ─── Config/info ──────────────────────────────────────────────────────────────

app.get('/api/config', (_req, res) => {
    res.json({
        workspaceRoot: process.env.WORKSPACE_ROOT ?? process.cwd(),
        allowShell:    process.env.ALLOW_SHELL === 'true',
        hasBraveKey:   Boolean(process.env.BRAVE_API_KEY),
        hasOpenAiKey:  Boolean(process.env.OPENAI_API_KEY),
        hasAnthropicKey: Boolean(process.env.ANTHROPIC_API_KEY),
        hasDatabaseUrl: Boolean(process.env.DATABASE_URL),
        port: PORT,
    });
});

// ─── 404 fallback ─────────────────────────────────────────────────────────────

app.use((_req: Request, res: Response) => {
    res.status(404).json({ error: 'Not found' });
});

// ─── Start ────────────────────────────────────────────────────────────────────

const server = http.createServer(app);
attachWebSocket(server);

server.listen(PORT, () => {
    console.log(`Agent IDE backend listening on port ${PORT}`);
    console.log(`  WebSocket: ws://localhost:${PORT}/ws`);
    console.log(`  Health:    http://localhost:${PORT}/health`);
    console.log(`  Runs API:  http://localhost:${PORT}/api/runs`);
    console.log(`  Tools API: http://localhost:${PORT}/api/tools`);
    const apiKeySet = process.env.OPENAI_API_KEY || process.env.ANTHROPIC_API_KEY;
    console.log(`  Live LLM:  ${apiKeySet ? 'enabled' : 'offline/demo mode (no API key)'}`);
    console.log(`  Shell:     ${process.env.ALLOW_SHELL === 'true' ? 'enabled' : 'disabled'}`);
});

// Prune old runs every 30 minutes
setInterval(() => runStore.prune(), 30 * 60 * 1000);

export { app, server };
