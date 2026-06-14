import * as http from 'http';
import express, { Request, Response, NextFunction } from 'express';
import cors from 'cors';
import { v4 as uuidv4 } from 'uuid';
import { attachWebSocket } from './websocket';
import { startAgentRun } from './agent-loop';
import { invokeTool } from './tool-proxy';
import { runStore } from './run-store';
import { mcpManager } from './mcp-manager';
import { workspaceManager } from './workspace-manager';
import { requireAuth, authenticatePassword, issueToken, extractUser, AuthedRequest } from './auth';
import { knowledgeStore, ingestText, ingestUrl, SearchResult } from './knowledge-store';
import { policyEngine } from './policy-engine';
import { auditLog } from './audit-log';
import { approvalGate } from './approval-gate';
import { RunRequest, ToolInvokeRequest } from './types';
import { identityStore } from './identity-store';
import { orgManager } from './org-manager';
import { startOrchestration, getOrchestrationRun, listOrchestrationRuns } from './orchestrator';

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

// Attach tenant user to all requests
app.use(requireAuth);

// API key auth override — Bearer aik_… tokens use identityStore
app.use((req: Request, _res: Response, next: NextFunction) => {
    const header = req.headers.authorization;
    if (header?.startsWith('Bearer aik_')) {
        const user = identityStore.authenticateApiKey(header.slice(7));
        if (user) (req as AuthedRequest).user = { userId: user.userId, email: user.email, name: user.name };
    }
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
        { id: 'db_query',         name: 'DB Query',          category: 'data',   browserNative: false, description: 'Execute SQL against the configured database (requires DATABASE_URL).' },
        { id: 'openhands_task',   name: 'OpenHandS Task',    category: 'code',   browserNative: false, description: 'Run a software dev task via OpenHandS agent (requires OPENHANDS_URL).' },
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

// ─── Auth ─────────────────────────────────────────────────────────────────────

// GET /api/auth/me — current authenticated user
app.get('/api/auth/me', (req: Request, res: Response) => {
    const user = (req as AuthedRequest).user;
    res.json(user);
});

// POST /api/auth/login — demo password login, returns a JWT
app.post('/api/auth/login', (req: Request, res: Response) => {
    const { email, password } = req.body as { email?: string; password?: string };
    if (!email || !password) { res.status(400).json({ error: 'email and password required' }); return; }
    const user = authenticatePassword(email, password);
    if (!user) { res.status(401).json({ error: 'Invalid credentials' }); return; }
    const token = issueToken(user);
    res.json({ token, user });
});

// POST /api/auth/logout — stateless; client discards token
app.post('/api/auth/logout', (_req, res) => res.json({ ok: true }));

// ─── Workspaces ───────────────────────────────────────────────────────────────

// GET /api/workspaces — list caller's workspaces
app.get('/api/workspaces', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    res.json(workspaceManager.list(userId));
});

// POST /api/workspaces — create a workspace for the caller
app.post('/api/workspaces', async (req: Request, res: Response) => {
    const { userId, name: userName } = (req as AuthedRequest).user;
    const { name } = req.body as { name?: string };
    if (!name) { res.status(400).json({ error: 'name is required' }); return; }
    const workspace = await workspaceManager.create(userId, name);
    res.status(201).json(workspace);
});

// GET /api/workspaces/:id
app.get('/api/workspaces/:id', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const w = workspaceManager.get(req.params['id'] ?? '');
    if (!w || w.tenantId !== userId) { res.status(404).json({ error: 'Workspace not found' }); return; }
    res.json(w);
});

// PATCH /api/workspaces/:id — rename
app.patch('/api/workspaces/:id', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const w = workspaceManager.get(req.params['id'] ?? '');
    if (!w || w.tenantId !== userId) { res.status(404).json({ error: 'Workspace not found' }); return; }
    const { name } = req.body as { name?: string };
    if (!name) { res.status(400).json({ error: 'name is required' }); return; }
    const updated = await workspaceManager.rename(w.id, name);
    res.json(updated);
});

// DELETE /api/workspaces/:id
app.delete('/api/workspaces/:id', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const w = workspaceManager.get(req.params['id'] ?? '');
    if (!w || w.tenantId !== userId) { res.status(404).json({ error: 'Workspace not found' }); return; }
    await workspaceManager.delete(w.id);
    res.json({ deleted: true });
});

// POST /api/workspaces/:id/activate — mark as active (status field)
app.post('/api/workspaces/:id/activate', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const w = workspaceManager.get(req.params['id'] ?? '');
    if (!w || w.tenantId !== userId) { res.status(404).json({ error: 'Workspace not found' }); return; }
    // Deactivate siblings
    for (const sibling of workspaceManager.list(userId)) {
        if (sibling.id !== w.id && sibling.status === 'active') {
            await workspaceManager.setStatus(sibling.id, 'inactive');
        }
    }
    const updated = await workspaceManager.setStatus(w.id, 'active');
    res.json(updated);
});

// ─── Governance ───────────────────────────────────────────────────────────────

// GET /api/governance/policies
app.get('/api/governance/policies', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    res.json(policyEngine.list(userId));
});

// POST /api/governance/policies — create
app.post('/api/governance/policies', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { name, description, enabled = true, priority = 10, rules = [] } = req.body as { name?: string; description?: string; enabled?: boolean; priority?: number; rules?: unknown[] };
    if (!name) { res.status(400).json({ error: 'name is required' }); return; }
    const policy = await policyEngine.create(userId, { name, description: description ?? '', enabled, priority, rules: rules as never[] });
    res.status(201).json(policy);
});

// PUT /api/governance/policies/:id — update
app.put('/api/governance/policies/:id', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const policy = policyEngine.get(req.params['id'] ?? '');
    if (!policy || policy.tenantId !== userId) { res.status(404).json({ error: 'Policy not found' }); return; }
    const updated = await policyEngine.update(policy.id, req.body as never);
    res.json(updated);
});

// DELETE /api/governance/policies/:id
app.delete('/api/governance/policies/:id', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const policy = policyEngine.get(req.params['id'] ?? '');
    if (!policy || policy.tenantId !== userId) { res.status(404).json({ error: 'Policy not found' }); return; }
    await policyEngine.delete(policy.id);
    res.json({ deleted: true });
});

// GET /api/governance/audit — audit log (last N entries)
app.get('/api/governance/audit', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { event, toolId, runId, limit } = req.query as Record<string, string>;
    res.json(auditLog.list(userId, { event, toolId, runId, limit: limit ? Number(limit) : 100 }));
});

// GET /api/governance/approvals — pending and recent approvals
app.get('/api/governance/approvals', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    res.json(approvalGate.list(userId));
});

// POST /api/governance/approvals/:id/approve
app.post('/api/governance/approvals/:id/approve', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const approval = approvalGate.get(req.params['id'] ?? '');
    if (!approval || approval.tenantId !== userId) { res.status(404).json({ error: 'Approval not found' }); return; }
    const ok = approvalGate.resolve(approval.id, true, userId);
    auditLog.append({ tenantId: userId, event: 'tool:approved', agentId: approval.agentId, runId: approval.runId, toolId: approval.toolId, policyId: approval.policyId, metadata: { resolvedBy: userId } });
    res.json({ ok });
});

// POST /api/governance/approvals/:id/reject
app.post('/api/governance/approvals/:id/reject', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const approval = approvalGate.get(req.params['id'] ?? '');
    if (!approval || approval.tenantId !== userId) { res.status(404).json({ error: 'Approval not found' }); return; }
    const ok = approvalGate.resolve(approval.id, false, userId);
    auditLog.append({ tenantId: userId, event: 'tool:rejected', agentId: approval.agentId, runId: approval.runId, toolId: approval.toolId, policyId: approval.policyId, metadata: { resolvedBy: userId } });
    res.json({ ok });
});

// ─── Knowledge / vector store ─────────────────────────────────────────────────

// GET /api/knowledge — list chunks for the calling tenant
app.get('/api/knowledge', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    res.json(knowledgeStore.list(userId).map(c => ({ id: c.id, title: c.title, source: c.source, createdAt: c.createdAt, metadata: c.metadata, contentPreview: c.content.slice(0, 200) })));
});

// POST /api/knowledge/search — semantic search
app.post('/api/knowledge/search', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { query, topK = 5 } = req.body as { query?: string; topK?: number };
    if (!query) { res.status(400).json({ error: 'query is required' }); return; }
    try {
        const results: SearchResult[] = await knowledgeStore.search(userId, query, topK);
        res.json(results.map(r => ({ score: r.score, id: r.chunk.id, title: r.chunk.title, source: r.chunk.source, contentPreview: r.chunk.content.slice(0, 400), createdAt: r.chunk.createdAt })));
    } catch (err: unknown) {
        res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
    }
});

// POST /api/knowledge/ingest — ingest text or URL
app.post('/api/knowledge/ingest', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { type, title, content, url, metadata } = req.body as { type?: string; title?: string; content?: string; url?: string; metadata?: Record<string, unknown> };
    try {
        if (type === 'url' && url) {
            const chunks = await ingestUrl(userId, url);
            res.status(201).json({ chunks: chunks.length, ids: chunks.map(c => c.id) });
        } else if (content) {
            const chunks = await ingestText(userId, title ?? 'Untitled', content, 'manual', metadata ?? {});
            res.status(201).json({ chunks: chunks.length, ids: chunks.map(c => c.id) });
        } else {
            res.status(400).json({ error: 'Provide content (text) or type=url with url' });
        }
    } catch (err: unknown) {
        res.status(500).json({ error: err instanceof Error ? err.message : String(err) });
    }
});

// GET /api/knowledge/:id — single chunk with full content
app.get('/api/knowledge/:id', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const chunk = knowledgeStore.get(req.params['id'] ?? '');
    if (!chunk || chunk.tenantId !== userId) { res.status(404).json({ error: 'Chunk not found' }); return; }
    res.json({ ...chunk, embedding: undefined });
});

// DELETE /api/knowledge/:id
app.delete('/api/knowledge/:id', async (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const chunk = knowledgeStore.get(req.params['id'] ?? '');
    if (!chunk || chunk.tenantId !== userId) { res.status(404).json({ error: 'Chunk not found' }); return; }
    await knowledgeStore.delete(chunk.id);
    res.json({ deleted: true });
});

// ─── Config/info ──────────────────────────────────────────────────────────────

app.get('/api/config', (req, res) => {
    const mcpServers = mcpManager.listServers();
    const { userId } = (req as AuthedRequest).user;
    res.json({
        workspaceRoot:    process.env.WORKSPACE_ROOT ?? process.cwd(),
        allowShell:       process.env.ALLOW_SHELL === 'true',
        hasBraveKey:      Boolean(process.env.BRAVE_API_KEY),
        hasOpenAiKey:     Boolean(process.env.OPENAI_API_KEY),
        hasAnthropicKey:  Boolean(process.env.ANTHROPIC_API_KEY),
        hasDatabaseUrl:   Boolean(process.env.DATABASE_URL),
        hasOpenHandsUrl:  Boolean(process.env.OPENHANDS_URL),
        port:             PORT,
        mcpServerCount:   mcpServers.length,
        mcpConnected:     mcpServers.filter(s => s.status === 'connected').length,
        authEnabled:      process.env.AUTH_ENABLED === 'true',
        workspaceCount:   workspaceManager.list(userId).length,
        knowledgeChunks:  knowledgeStore.count(userId),
        hasEmbeddingKey:  Boolean(process.env.OPENAI_API_KEY),
    });
});

// ─── Identity & lifecycle ──────────────────────────────────────────────────────

// POST /api/identity/register — create a new user account
app.post('/api/identity/register', (req: Request, res: Response) => {
    const { email, name, password, role } = req.body as { email?: string; name?: string; password?: string; role?: string };
    if (!email || !name || !password) { res.status(400).json({ error: 'email, name, and password are required' }); return; }
    try {
        const user = identityStore.createUser(email, name, password, (role as 'admin' | 'developer' | 'viewer') ?? 'developer');
        const { passwordHash: _, ...safe } = user;
        res.status(201).json(safe);
    } catch (err: unknown) {
        res.status(409).json({ error: err instanceof Error ? err.message : 'Registration failed' });
    }
});

// GET /api/identity/me — current user profile (from identity store)
app.get('/api/identity/me', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const user = identityStore.getUser(userId);
    if (!user) { res.json({ userId, email: (req as AuthedRequest).user.email, name: (req as AuthedRequest).user.name, role: 'developer', mfaEnabled: false, status: 'active' }); return; }
    const { passwordHash: _, ...safe } = user;
    res.json(safe);
});

// PUT /api/identity/me — update profile
app.put('/api/identity/me', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const patch = req.body as { name?: string; avatarUrl?: string; mfaEnabled?: boolean };
    const updated = identityStore.updateUser(userId, patch);
    if (!updated) { res.status(404).json({ error: 'User not found in identity store' }); return; }
    const { passwordHash: _, ...safe } = updated;
    res.json(safe);
});

// PUT /api/identity/me/password — change password
app.put('/api/identity/me/password', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { password } = req.body as { password?: string };
    if (!password || password.length < 8) { res.status(400).json({ error: 'Password must be at least 8 characters' }); return; }
    identityStore.changePassword(userId, password);
    res.json({ ok: true });
});

// GET /api/identity/users — list all users (admin only)
app.get('/api/identity/users', (req: Request, res: Response) => {
    const user = identityStore.getUser((req as AuthedRequest).user.userId);
    if (user?.role !== 'admin') { res.status(403).json({ error: 'Admin only' }); return; }
    res.json(identityStore.listUsers().map(u => { const { passwordHash: _, ...safe } = u; return safe; }));
});

// ─── Agent Identities ─────────────────────────────────────────────────────────

app.get('/api/identity/agents', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    res.json(identityStore.listAgents(userId));
});

app.post('/api/identity/agents', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { name, description = '', model = 'claude-sonnet-4-6', capabilities = [], orgId } = req.body as { name?: string; description?: string; model?: string; capabilities?: string[]; orgId?: string };
    if (!name) { res.status(400).json({ error: 'name is required' }); return; }
    const agent = identityStore.createAgent({ name, description, ownerId: userId, model, status: 'active', capabilities, orgId });
    res.status(201).json(agent);
});

app.put('/api/identity/agents/:id', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const agent = identityStore.getAgent(req.params['id'] ?? '');
    if (!agent || agent.ownerId !== userId) { res.status(404).json({ error: 'Agent not found' }); return; }
    const updated = identityStore.updateAgent(agent.id, req.body as never);
    res.json(updated);
});

app.delete('/api/identity/agents/:id', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const agent = identityStore.getAgent(req.params['id'] ?? '');
    if (!agent || agent.ownerId !== userId) { res.status(404).json({ error: 'Agent not found' }); return; }
    identityStore.deleteAgent(agent.id);
    res.json({ deleted: true });
});

// ─── API Keys ─────────────────────────────────────────────────────────────────

app.get('/api/identity/api-keys', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    res.json(identityStore.listApiKeys(userId).map(k => ({ ...k, keyHash: undefined })));
});

app.post('/api/identity/api-keys', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { name, scopes = ['runs:write', 'tools:invoke'], agentId, expiresAt } = req.body as { name?: string; scopes?: string[]; agentId?: string; expiresAt?: string };
    if (!name) { res.status(400).json({ error: 'name is required' }); return; }
    const { key, raw } = identityStore.createApiKey(userId, name, scopes, agentId, expiresAt);
    res.status(201).json({ ...key, keyHash: undefined, raw }); // raw shown once only
});

app.delete('/api/identity/api-keys/:id', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const ok = identityStore.revokeApiKey(req.params['id'] ?? '', userId);
    if (!ok) { res.status(404).json({ error: 'API key not found' }); return; }
    res.json({ revoked: true });
});

// ─── Organizations ────────────────────────────────────────────────────────────

app.get('/api/identity/orgs', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    res.json(orgManager.listOrgs(userId));
});

app.post('/api/identity/orgs', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const { name, description } = req.body as { name?: string; description?: string };
    if (!name) { res.status(400).json({ error: 'name is required' }); return; }
    res.status(201).json(orgManager.createOrg(name, userId, description));
});

app.get('/api/identity/orgs/:id/members', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const org = orgManager.getOrg(req.params['id'] ?? '');
    if (!org) { res.status(404).json({ error: 'Org not found' }); return; }
    if (!orgManager.getMember(org.id, userId)) { res.status(403).json({ error: 'Not a member' }); return; }
    res.json(orgManager.listMembers(org.id));
});

app.post('/api/identity/orgs/:id/members', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const org = orgManager.getOrg(req.params['id'] ?? '');
    if (!org) { res.status(404).json({ error: 'Org not found' }); return; }
    const me = orgManager.getMember(org.id, userId);
    if (!me || (me.role !== 'owner' && me.role !== 'admin')) { res.status(403).json({ error: 'Admin required' }); return; }
    const { memberId, role = 'member' } = req.body as { memberId?: string; role?: string };
    if (!memberId) { res.status(400).json({ error: 'memberId is required' }); return; }
    res.status(201).json(orgManager.addMember(org.id, memberId, role as 'admin' | 'member' | 'viewer'));
});

app.delete('/api/identity/orgs/:id/members/:memberId', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const org = orgManager.getOrg(req.params['id'] ?? '');
    if (!org) { res.status(404).json({ error: 'Org not found' }); return; }
    const me = orgManager.getMember(org.id, userId);
    if (!me || (me.role !== 'owner' && me.role !== 'admin')) { res.status(403).json({ error: 'Admin required' }); return; }
    orgManager.removeMember(org.id, req.params['memberId'] ?? '');
    res.json({ removed: true });
});

app.get('/api/identity/orgs/:id/teams', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const org = orgManager.getOrg(req.params['id'] ?? '');
    if (!org) { res.status(404).json({ error: 'Org not found' }); return; }
    if (!orgManager.getMember(org.id, userId)) { res.status(403).json({ error: 'Not a member' }); return; }
    const teams = orgManager.listTeams(org.id);
    res.json(teams.map(t => ({ ...t, memberCount: orgManager.listTeamMembers(t.id).length })));
});

app.post('/api/identity/orgs/:id/teams', (req: Request, res: Response) => {
    const { userId } = (req as AuthedRequest).user;
    const org = orgManager.getOrg(req.params['id'] ?? '');
    if (!org) { res.status(404).json({ error: 'Org not found' }); return; }
    const me = orgManager.getMember(org.id, userId);
    if (!me || (me.role !== 'owner' && me.role !== 'admin')) { res.status(403).json({ error: 'Admin required' }); return; }
    const { name, description } = req.body as { name?: string; description?: string };
    if (!name) { res.status(400).json({ error: 'name is required' }); return; }
    res.status(201).json(orgManager.createTeam(org.id, name, description));
});

app.post('/api/identity/orgs/:id/teams/:teamId/members', (req: Request, res: Response) => {
    const org = orgManager.getOrg(req.params['id'] ?? '');
    const team = orgManager.getTeam(req.params['teamId'] ?? '');
    if (!org || !team || team.orgId !== org.id) { res.status(404).json({ error: 'Team not found' }); return; }
    const { memberId, role = 'member' } = req.body as { memberId?: string; role?: string };
    if (!memberId) { res.status(400).json({ error: 'memberId is required' }); return; }
    res.status(201).json(orgManager.addTeamMember(team.id, memberId, role as 'lead' | 'member'));
});

// ─── Orchestration ────────────────────────────────────────────────────────────

app.post('/api/orchestrate', async (req: Request, res: Response) => {
    const { goal, model = 'claude-sonnet-4-6', tools = [], apiKey } = req.body as { goal?: string; model?: string; tools?: string[]; apiKey?: string };
    if (!goal) { res.status(400).json({ error: 'goal is required' }); return; }
    const key = apiKey ?? process.env.ANTHROPIC_API_KEY ?? process.env.OPENAI_API_KEY ?? '';
    const id = await startOrchestration(goal, model, key, tools);
    res.status(202).json({ id, status: 'running', wsUrl: `/ws/${id}` });
});

app.get('/api/orchestrate', (_req: Request, res: Response) => {
    res.json(listOrchestrationRuns());
});

app.get('/api/orchestrate/:id', (req: Request, res: Response) => {
    const run = getOrchestrationRun(req.params['id'] ?? '');
    if (!run) { res.status(404).json({ error: 'Orchestration run not found' }); return; }
    res.json(run);
});

// ─── Headless render ─────────────────────────────────────────────────────────
// GET /api/runs/:id/render → PNG trace diagram (headless Fabric.js canvas)
// GET /api/peers/render    → PNG mesh topology diagram

app.get('/api/runs/:id/render', async (req: Request, res: Response) => {
    const run = runStore.get(req.params['id'] ?? '');
    if (!run) { res.status(404).json({ error: 'Run not found' }); return; }
    try {
        const { renderRunTrace } = await import('./headless-renderer');
        const png = await renderRunTrace(run as any);
        res.set('Content-Type', 'image/png').send(png);
    } catch (e: any) {
        res.status(501).json({ error: e?.message ?? 'Headless renderer not available' });
    }
});

app.get('/api/peers/render', async (_req: Request, res: Response) => {
    const peers = Array.from(peerStore.values());
    try {
        const { renderMesh } = await import('./headless-renderer');
        const png = await renderMesh(peers);
        res.set('Content-Type', 'image/png').send(png);
    } catch (e: any) {
        res.status(501).json({ error: e?.message ?? 'Headless renderer not available' });
    }
});

// ─── Peers ────────────────────────────────────────────────────────────────────
// In-memory peer store (survives only until restart; persistence is left to the data volume)

interface PeerRecord { id: string; name: string; url: string; status: 'online' | 'offline' | 'unknown'; lastSeen?: string; region?: string; }
const peerStore = new Map<string, PeerRecord>();

app.get('/api/peers', (_req: Request, res: Response) => {
    res.json(Array.from(peerStore.values()));
});

app.post('/api/peers', (req: Request, res: Response) => {
    const { name, url, region } = req.body as { name?: string; url?: string; region?: string };
    if (!name || !url) { res.status(400).json({ error: 'name and url are required' }); return; }
    const id = uuidv4();
    const peer: PeerRecord = { id, name, url: url.replace(/\/$/, ''), status: 'unknown', region };
    peerStore.set(id, peer);
    // Fire-and-forget ping to establish initial status
    fetch(`${peer.url}/health`, { signal: AbortSignal.timeout(5000) })
        .then(r => { if (r.ok) { peer.status = 'online'; peer.lastSeen = new Date().toISOString(); } else { peer.status = 'offline'; } })
        .catch(() => { peer.status = 'offline'; });
    res.status(201).json(peer);
});

app.delete('/api/peers/:id', (req: Request, res: Response) => {
    if (!peerStore.has(req.params['id'] ?? '')) { res.status(404).json({ error: 'Peer not found' }); return; }
    peerStore.delete(req.params['id'] ?? '');
    res.json({ deleted: true });
});

// ─── Transfer (agent teleportation) ──────────────────────────────────────────

// POST /api/transfer — push an agent to a peer (egress-only surface)
app.post('/api/transfer', async (req: Request, res: Response) => {
    const { agentId, peerId } = req.body as { agentId?: string; peerId?: string };
    if (!agentId || !peerId) { res.status(400).json({ error: 'agentId and peerId are required' }); return; }

    const peer = peerStore.get(peerId);
    if (!peer) { res.status(404).json({ error: 'Peer not found' }); return; }

    const agent = identityStore.getAgent(agentId);
    if (!agent) { res.status(404).json({ error: 'Agent not found' }); return; }

    const transferKey = process.env.TRANSFER_API_KEY;
    const headers: Record<string, string> = { 'Content-Type': 'application/json' };
    if (transferKey) headers['Authorization'] = `Bearer ${transferKey}`;

    try {
        const resp = await fetch(`${peer.url}/transfer/receive`, {
            method: 'POST',
            headers,
            body: JSON.stringify({ agent, sourceUrl: process.env.PUBLIC_URL ?? '' }),
            signal: AbortSignal.timeout(15000),
        });
        if (!resp.ok) {
            const txt = await resp.text().catch(() => resp.statusText);
            res.status(502).json({ ok: false, message: `Peer rejected: ${txt}` });
            return;
        }
        peer.status = 'online'; peer.lastSeen = new Date().toISOString();
        res.json({ ok: true, message: `Agent ${agent.name} transferred to ${peer.name}` });
    } catch (err: any) {
        peer.status = 'offline';
        res.status(502).json({ ok: false, message: `Transfer failed: ${err?.message ?? err}` });
    }
});

// POST /transfer/receive — receive an incoming agent from a peer (gated by TRANSFER_API_KEY)
app.post('/transfer/receive', (req: Request, res: Response) => {
    const transferKey = process.env.TRANSFER_API_KEY;
    if (transferKey) {
        const auth = req.headers.authorization;
        if (!auth || auth !== `Bearer ${transferKey}`) {
            res.status(401).json({ error: 'Unauthorized' }); return;
        }
    }
    const { agent, sourceUrl } = req.body as { agent?: any; sourceUrl?: string };
    if (!agent?.name) { res.status(400).json({ error: 'Invalid agent payload' }); return; }

    const imported = identityStore.createAgent({
        ownerId: 'user_demo',
        name: agent.name ?? 'Imported Agent',
        description: agent.description ?? `Teleported from ${sourceUrl ?? 'peer'}`,
        model: agent.model ?? 'claude-sonnet-4-6',
        status: 'idle',
        capabilities: agent.capabilities ?? [],
    });
    console.log(`[transfer] received agent "${imported.name}" (${imported.id}) from ${sourceUrl ?? 'unknown'}`);
    res.json({ ok: true, importedId: imported.id, name: imported.name });
});

// ─── Infra instances ──────────────────────────────────────────────────────────
// Static list from env; extend later with OCI API integration

app.get('/api/infra/instances', (_req: Request, res: Response) => {
    const raw = process.env.INFRA_INSTANCES ?? '';
    if (!raw) { res.json([]); return; }
    try {
        res.json(JSON.parse(raw));
    } catch {
        res.status(500).json({ error: 'INFRA_INSTANCES env is not valid JSON' });
    }
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

    // Seed demo identity (no-op if already exists)
    identityStore.ensureDemoUser();
    orgManager.ensureDemoOrg('user_demo');
    console.log(`  Identity:    ${identityStore.listUsers().length} users, ${identityStore.listAgents().length} agents`);

    // Load MCP server configs (reads .mcp.json, writes defaults if absent)
    await mcpManager.loadConfig();
    const mcpServers = mcpManager.listServers();
    console.log(`  MCP servers: ${mcpServers.length} configured`);

    // Load workspace store and seed demo workspace
    await workspaceManager.load();
    await workspaceManager.ensureDefaultWorkspace('user_demo', 'Default Workspace');
    console.log(`  Workspaces:  ${workspaceManager.listAll().length} loaded`);

    // Load knowledge store
    await knowledgeStore.load();
    console.log(`  Knowledge:   ${knowledgeStore.count()} chunks loaded`);

    // Load governance (policies, audit log)
    await policyEngine.load();
    await policyEngine.ensureDefaults('user_demo');
    await auditLog.load();
    console.log(`  Governance:  ${policyEngine.list('user_demo').length} policies, ${auditLog.count('user_demo')} audit entries`);

    // Prune approval gate every hour
    setInterval(() => approvalGate.prune(), 60 * 60 * 1000);
});

// Prune old runs every 30 minutes
setInterval(() => runStore.prune(), 30 * 60 * 1000);

export { app, server };
