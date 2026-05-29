import * as fs from 'fs/promises';
import * as path from 'path';
import { execFile } from 'child_process';
import { promisify } from 'util';
import { ToolInvokeRequest, ToolInvokeResult } from './types';
import { policyEngine } from './policy-engine';
import { approvalGate } from './approval-gate';
import { auditLog } from './audit-log';

const execFileAsync = promisify(execFile);

const WORKSPACE_ROOT = process.env.WORKSPACE_ROOT ?? process.cwd();
const ALLOW_SHELL = process.env.ALLOW_SHELL === 'true';

// Resolve a path safely within WORKSPACE_ROOT; throws if path escapes.
function safePath(rel: string): string {
    const resolved = path.resolve(WORKSPACE_ROOT, rel);
    if (!resolved.startsWith(path.resolve(WORKSPACE_ROOT))) {
        throw new Error(`Path escape attempt blocked: ${rel}`);
    }
    return resolved;
}

const TOOL_HANDLERS: Record<string, (input: Record<string, unknown>) => Promise<unknown>> = {

    // ── File R/W ──────────────────────────────────────────────────────────────
    file_rw: async (input) => {
        const op   = String(input['operation'] ?? 'read');
        const rel  = String(input['path'] ?? '');
        const abs  = safePath(rel);

        if (op === 'read') {
            const content = await fs.readFile(abs, 'utf-8');
            return { content, path: rel, bytes: content.length };
        }
        if (op === 'write') {
            const content = String(input['content'] ?? '');
            await fs.mkdir(path.dirname(abs), { recursive: true });
            await fs.writeFile(abs, content, 'utf-8');
            return { success: true, path: rel, bytes: content.length };
        }
        if (op === 'list') {
            const entries = await fs.readdir(abs, { withFileTypes: true });
            return { entries: entries.map(e => ({ name: e.name, type: e.isDirectory() ? 'dir' : 'file' })) };
        }
        if (op === 'delete') {
            await fs.rm(abs, { recursive: true });
            return { success: true, path: rel };
        }
        throw new Error(`Unknown operation: ${op}`);
    },

    // ── Shell (opt-in via env) ────────────────────────────────────────────────
    shell: async (input) => {
        if (!ALLOW_SHELL) {
            return { blocked: true, reason: 'Shell execution disabled. Set ALLOW_SHELL=true to enable.' };
        }
        const cmd = String(input['command'] ?? '');
        if (!cmd) throw new Error('command is required');
        const timeout = Number(input['timeout'] ?? 10000);
        const { stdout, stderr } = await execFileAsync('sh', ['-c', cmd], {
            cwd: WORKSPACE_ROOT,
            timeout,
            maxBuffer: 256 * 1024,
        });
        return { stdout, stderr, exitCode: 0 };
    },

    // ── HTTP Client ───────────────────────────────────────────────────────────
    http_client: async (input) => {
        const method  = String(input['method'] ?? 'GET').toUpperCase();
        const url     = String(input['url'] ?? '');
        const body    = input['body'] ? String(input['body']) : undefined;
        const rawHdrs = input['headers'];
        const headers: Record<string, string> = {
            'User-Agent': 'AgentIDE/0.1',
            ...(rawHdrs && typeof rawHdrs === 'object' ? rawHdrs as Record<string, string> : {}),
        };
        const init: RequestInit = { method, headers };
        if (body && method !== 'GET' && method !== 'HEAD') init.body = body;
        const res = await fetch(url, init);
        const text = await res.text();
        return { status: res.status, headers: Object.fromEntries(res.headers.entries()), body: text.slice(0, 4096) };
    },

    // ── Vector Search (in-memory cosine sim) ─────────────────────────────────
    vector_search: async (input) => {
        const query = String(input['query'] ?? '');
        const topK  = Number(input['topK'] ?? 3);
        const CHUNKS = [
            { id: 'c1', text: 'AgentBench evaluates LLMs as agents across 8 interactive environments.', vec: [0.8,0.6,0.2,0.1,0.9,0.3,0.4,0.7] },
            { id: 'c2', text: 'ReAct interleaves reasoning and acting for multi-step problem solving.',    vec: [0.7,0.5,0.3,0.2,0.8,0.4,0.6,0.5] },
            { id: 'c3', text: 'Prompt caching reduces repeated context tokens to 10% of base cost.',       vec: [0.3,0.2,0.9,0.8,0.1,0.7,0.2,0.4] },
            { id: 'c4', text: 'Tool use enables agents to call external APIs and execute code.',          vec: [0.5,0.8,0.4,0.3,0.6,0.9,0.7,0.2] },
            { id: 'c5', text: 'Eclipse Theia provides a VS Code-like workbench for building browser IDEs.', vec: [0.4,0.3,0.5,0.9,0.2,0.6,0.8,0.3] },
        ];
        const h = Array.from(query).reduce((s, c) => s + c.charCodeAt(0), 0);
        const qv = Array.from({ length: 8 }, (_, i) => Math.abs(Math.sin(h * (i + 1))));
        const dot = (a: number[], b: number[]) => a.reduce((s, v, i) => s + v * b[i], 0);
        const norm = (a: number[]) => Math.sqrt(a.reduce((s, v) => s + v * v, 0));
        const sim = (a: number[], b: number[]) => dot(a, b) / (norm(a) * norm(b) || 1);
        const scored = CHUNKS.map(c => ({ ...c, score: sim(qv, c.vec) }));
        scored.sort((a, b) => b.score - a.score);
        return { matches: scored.slice(0, topK).map(c => ({ id: c.id, score: parseFloat(c.score.toFixed(3)), content: c.text })) };
    },

    // ── Browser / Web Search (requires live API keys) ─────────────────────────
    browser: async (input) => {
        const url = String(input['url'] ?? '');
        const res = await fetch(url, { headers: { 'User-Agent': 'AgentIDE/0.1' } });
        const html = await res.text();
        // Minimal HTML → text extraction (strip tags)
        const text = html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 4000);
        return { url, status: res.status, text, charCount: text.length };
    },

    web_search: async (input) => {
        const apiKey = process.env.BRAVE_API_KEY;
        if (!apiKey) return { error: 'BRAVE_API_KEY not configured', results: [] };
        const q   = encodeURIComponent(String(input['query'] ?? ''));
        const cnt = Number(input['limit'] ?? 5);
        const res = await fetch(`https://api.search.brave.com/res/v1/web/search?q=${q}&count=${cnt}`, {
            headers: { 'Accept': 'application/json', 'X-Subscription-Token': apiKey },
        });
        const data = await res.json() as { web?: { results: Array<{ title: string; url: string; description: string }> } };
        return { results: (data.web?.results ?? []).map(r => ({ title: r.title, url: r.url, snippet: r.description })) };
    },

    db_query: async (input) => {
        // Requires DATABASE_URL; returns mock when not configured
        if (!process.env.DATABASE_URL) {
            return { mock: true, rows: [{ id: 1, result: `[mock] ${input['query']}` }], rowCount: 1 };
        }
        return { error: 'Live DB driver not yet wired. Set DATABASE_URL and add pg package.' };
    },

    code_exec: async (input) => {
        if (!ALLOW_SHELL) {
            return { blocked: true, reason: 'Code execution requires ALLOW_SHELL=true' };
        }
        const lang = String(input['language'] ?? 'python');
        const code = String(input['code'] ?? '');
        if (lang === 'python') {
            const { stdout, stderr } = await execFileAsync('python3', ['-c', code], { timeout: 10000, maxBuffer: 128 * 1024 });
            return { stdout, stderr, exitCode: 0 };
        }
        if (lang === 'javascript' || lang === 'js') {
            const { stdout, stderr } = await execFileAsync('node', ['--eval', code], { timeout: 10000, maxBuffer: 128 * 1024 });
            return { stdout, stderr, exitCode: 0 };
        }
        return { error: `Unsupported language: ${lang}` };
    },
};

export async function invokeTool(req: ToolInvokeRequest): Promise<ToolInvokeResult> {
    const handler = TOOL_HANDLERS[req.toolId];
    const start = Date.now();
    const tenantId = req.agentId ?? 'user_demo';

    if (!handler) {
        return { toolId: req.toolId, output: null, durationMs: 0, success: false, error: `Unknown tool: ${req.toolId}` };
    }

    // ── Policy check ──────────────────────────────────────────────────────────
    const decision = policyEngine.evaluate({ toolId: req.toolId, agentId: req.agentId ?? '', runId: req.runId ?? '', tenantId, input: req.input });

    auditLog.append({ tenantId, event: 'tool:call', agentId: req.agentId, runId: req.runId, toolId: req.toolId, decision: decision.action, policyId: decision.policyId, metadata: { reason: decision.reason } });

    if (decision.action === 'deny') {
        return { toolId: req.toolId, output: null, durationMs: Date.now() - start, success: false, error: `[GOVERNANCE BLOCK] ${decision.reason}` };
    }

    if (decision.action === 'require-approval') {
        const approved = await approvalGate.request({ tenantId, runId: req.runId ?? '', agentId: req.agentId ?? '', toolId: req.toolId, input: req.input, policyId: decision.policyId, reason: decision.reason });
        auditLog.append({ tenantId, event: approved ? 'tool:approved' : 'tool:rejected', agentId: req.agentId, runId: req.runId, toolId: req.toolId, policyId: decision.policyId, metadata: {} });
        if (!approved) {
            return { toolId: req.toolId, output: null, durationMs: Date.now() - start, success: false, error: `[GOVERNANCE REJECTED] Tool call rejected or timed out.` };
        }
    }

    // ── Execute ───────────────────────────────────────────────────────────────
    try {
        const output = await handler(req.input);
        return { toolId: req.toolId, output, durationMs: Date.now() - start, success: true };
    } catch (err: unknown) {
        const error = err instanceof Error ? err.message : String(err);
        return { toolId: req.toolId, output: null, durationMs: Date.now() - start, success: false, error };
    }
}
