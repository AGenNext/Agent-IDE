import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { McpPanelCommand } from '../agent-ide-commands';
import { getAllTools, getToolsByCategory, ToolDefinition, ToolCategory } from '../runtime/tool-registry';
import { isBackendReachable, listMcpServers, connectMcpServer, disconnectMcpServer, addMcpServer, removeMcpServer, McpServerState, McpTransport } from '../runtime/backend-client';

type McpTab = 'tools' | 'servers' | 'logs';

// ─── MCP server model ─────────────────────────────────────────────────────────

type ServerStatus = 'connected' | 'connecting' | 'disconnected' | 'error';
type Transport = 'stdio' | 'sse' | 'websocket';

interface McpServer {
    id: string;
    name: string;
    transport: Transport;
    endpoint: string;
    status: ServerStatus;
    toolCount: number;
    connectedAt?: string;
    error?: string;
}

function stateToServer(s: McpServerState): McpServer {
    return {
        id: s.id,
        name: s.name,
        transport: s.transport as Transport,
        endpoint: s.command ?? s.endpoint ?? '',
        status: s.status as ServerStatus,
        toolCount: s.toolCount,
        connectedAt: s.connectedAt,
        error: s.error,
    };
}

const DEMO_SERVERS: McpServer[] = [
    { id: 'filesystem',   name: 'filesystem',      transport: 'stdio',     endpoint: 'npx @modelcontextprotocol/server-filesystem .', status: 'disconnected', toolCount: 0 },
    { id: 'brave-search', name: 'brave-search',    transport: 'stdio',     endpoint: 'npx @modelcontextprotocol/server-brave-search', status: 'disconnected', toolCount: 0 },
    { id: 'postgres',     name: 'postgres',        transport: 'stdio',     endpoint: 'npx @modelcontextprotocol/server-postgres',     status: 'disconnected', toolCount: 0 },
    { id: 'puppeteer',    name: 'puppeteer',        transport: 'stdio',     endpoint: 'npx @modelcontextprotocol/server-puppeteer',    status: 'disconnected', toolCount: 0 },
];

// ─── Invocation log ──────────────────────────────────────────────────────────

interface InvocationLog {
    id: string;
    ts: string;
    tool: string;
    input: string;
    output: string;
    durationMs: number;
    success: boolean;
    agent: string;
}

const INITIAL_LOGS: InvocationLog[] = [
    { id: 'l1', ts: '14:22:08', tool: 'browser',      input: '{"url":"https://docs.example.com"}',     output: '{"title":"Example Docs","text":"..."}', durationMs: 820,  success: true,  agent: 'ResearchAgent' },
    { id: 'l2', ts: '14:08:31', tool: 'file_rw',      input: '{"operation":"read","path":"/workspace/src/index.ts"}', output: '{"content":"import ..."}', durationMs: 42,   success: true,  agent: 'CoderAgent' },
    { id: 'l3', ts: '13:55:12', tool: 'vector_search', input: '{"query":"agent evaluation metrics","topK":5}', output: '{"matches":[{"score":0.91,...}]}', durationMs: 18,   success: true,  agent: 'AnalystAgent' },
    { id: 'l4', ts: '13:30:44', tool: 'http_client',  input: '{"method":"GET","url":"https://api.github.com/repos/theia-ide/theia"}', output: '{"status":200,"body":"{...}"}', durationMs: 310, success: true,  agent: 'WriterAgent' },
    { id: 'l5', ts: '13:01:19', tool: 'web_search',   input: '{"query":"agent ide competitors 2025","limit":10}', output: '{"error":"API key not configured"}', durationMs: 55,  success: false, agent: 'ResearchAgent' },
    { id: 'l6', ts: '12:45:02', tool: 'shell',        input: '{"command":"ls -la /workspace"}',        output: '[BLOCKED by governance policy]',         durationMs: 0,    success: false, agent: 'CoderAgent' },
];

// ─── Sub-components ───────────────────────────────────────────────────────────

const CATEGORY_COLORS: Record<ToolCategory, string> = {
    web:    '#7ab4ff',
    code:   '#60d060',
    data:   '#f06040',
    file:   '#d0a030',
    api:    '#c080ff',
    memory: '#40c0c0',
};

const ALL_CATS: ToolCategory[] = ['web', 'code', 'data', 'file', 'api', 'memory'];

function CatBadge({ cat }: { cat: ToolCategory }) {
    return <span style={{ background: '#111', color: CATEGORY_COLORS[cat], border: `1px solid ${CATEGORY_COLORS[cat]}44`, padding: '1px 6px', borderRadius: 3, fontSize: 10, fontWeight: 700, fontFamily: 'monospace' }}>{cat.toUpperCase()}</span>;
}

function NativeBadge({ native }: { native: boolean }) {
    return native
        ? <span style={{ fontSize: 10, color: '#40c0c0', fontFamily: 'monospace' }}>browser-native</span>
        : <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>backend</span>;
}

function TestInvoke({ tool }: { tool: ToolDefinition }) {
    const [open, setOpen] = React.useState(false);
    const [running, setRunning] = React.useState(false);
    const [result, setResult] = React.useState<string | null>(null);
    const reqFields = Object.entries(tool.inputSchema).filter(([, v]) => v.required);
    const [values, setValues] = React.useState<Record<string, string>>(() =>
        Object.fromEntries(reqFields.map(([k]) => [k, '']))
    );

    async function invoke() {
        setRunning(true); setResult(null);
        try {
            const out = await tool.mockExecute(values as Record<string, unknown>);
            setResult(JSON.stringify(out, null, 2));
        } catch (e) {
            setResult(`Error: ${e}`);
        } finally {
            setRunning(false);
        }
    }

    if (!open) {
        return <button onClick={() => setOpen(true)} style={{ padding: '3px 8px', background: 'none', border: '1px solid #2a3a2a', color: '#60d060', borderRadius: 3, cursor: 'pointer', fontSize: 10 }}>▶ Test</button>;
    }

    return (
        <div style={{ marginTop: 8, border: '1px solid #1e2e1e', borderRadius: 4, padding: 10, background: '#0a110a' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                <span style={{ fontSize: 11, fontWeight: 700, color: '#60d060' }}>Test Invoke</span>
                <button onClick={() => { setOpen(false); setResult(null); }} style={{ marginLeft: 'auto', background: 'none', border: 'none', color: '#666', cursor: 'pointer', fontSize: 13 }}>×</button>
            </div>
            {reqFields.map(([k, v]) => (
                <div key={k} style={{ marginBottom: 6 }}>
                    <div style={{ fontSize: 10, color: '#555', marginBottom: 2 }}>{k} <span style={{ color: '#444' }}>({v.type})</span></div>
                    <input
                        value={values[k] ?? ''}
                        onChange={e => setValues(prev => ({ ...prev, [k]: e.target.value }))}
                        placeholder={v.description}
                        style={{ width: '100%', boxSizing: 'border-box', background: '#111', border: '1px solid #222', color: '#ddd', borderRadius: 3, padding: '4px 8px', fontSize: 11, outline: 'none' }}
                    />
                </div>
            ))}
            <button onClick={invoke} disabled={running}
                style={{ padding: '5px 12px', background: running ? '#111' : '#0a1a0a', border: '1px solid #3a6a3a', color: running ? '#444' : '#60d060', borderRadius: 3, cursor: running ? 'default' : 'pointer', fontSize: 11, marginTop: 4 }}>
                {running ? 'Running…' : '▶ Invoke'}
            </button>
            {result && (
                <pre style={{ margin: '8px 0 0', background: '#0d0d0d', color: '#a0d080', padding: 8, borderRadius: 3, fontSize: 10, overflow: 'auto', maxHeight: 160, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
                    {result}
                </pre>
            )}
        </div>
    );
}

function ToolCard({ tool, expanded, onToggle }: { tool: ToolDefinition; expanded: boolean; onToggle: () => void }) {
    return (
        <div style={{ border: '1px solid #1e1e1e', borderRadius: 6, marginBottom: 6, overflow: 'hidden' }}>
            <div
                onClick={onToggle}
                style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 12px', cursor: 'pointer', background: expanded ? '#111' : '#0d0d0d' }}
            >
                <CatBadge cat={tool.category} />
                <span style={{ fontSize: 12, fontWeight: 600, color: '#d0d0d0', flex: 1 }}>{tool.name}</span>
                <NativeBadge native={tool.browserNative} />
                <span style={{ color: '#555', fontSize: 12 }}>{expanded ? '▾' : '▸'}</span>
            </div>
            {expanded && (
                <div style={{ padding: '8px 12px', background: '#0a0a0a', borderTop: '1px solid #1a1a1a' }}>
                    <div style={{ fontSize: 11, color: '#888', marginBottom: 10, lineHeight: 1.5 }}>{tool.description}</div>
                    <div style={{ fontSize: 10, color: '#555', marginBottom: 6, fontWeight: 700 }}>INPUT SCHEMA</div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 10 }}>
                        {Object.entries(tool.inputSchema).map(([k, v]) => (
                            <div key={k} style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 11, fontFamily: 'monospace' }}>
                                <span style={{ color: v.required ? '#60d060' : '#7ab4ff' }}>{k}</span>
                                <span style={{ color: '#444' }}>: {v.type}</span>
                                {v.required && <span style={{ fontSize: 9, color: '#404040', fontFamily: 'sans-serif' }}>required</span>}
                                <span style={{ color: '#555', fontFamily: 'sans-serif', fontSize: 10, flex: 1 }}>{v.description}</span>
                            </div>
                        ))}
                    </div>
                    <TestInvoke tool={tool} />
                </div>
            )}
        </div>
    );
}

function ToolsTab() {
    const [catFilter, setCatFilter] = React.useState<ToolCategory | 'all'>('all');
    const [expanded, setExpanded] = React.useState<string | null>(null);
    const allTools = getAllTools();
    const filtered = catFilter === 'all' ? allTools : getToolsByCategory(catFilter);

    return (
        <div style={{ display: 'flex', height: '100%', overflow: 'hidden' }}>
            {/* Category sidebar */}
            <div style={{ width: 110, borderRight: '1px solid #1e1e1e', display: 'flex', flexDirection: 'column', background: '#0d0d0d' }}>
                <button onClick={() => setCatFilter('all')}
                    style={{ padding: '8px 10px', background: catFilter === 'all' ? '#111' : 'none', border: 'none', borderBottom: '1px solid #1a1a1a', color: catFilter === 'all' ? '#e0e0e0' : '#666', cursor: 'pointer', textAlign: 'left', fontSize: 11, fontWeight: catFilter === 'all' ? 700 : 400 }}>
                    All ({allTools.length})
                </button>
                {ALL_CATS.map(c => {
                    const count = getToolsByCategory(c).length;
                    if (!count) return null;
                    return (
                        <button key={c} onClick={() => setCatFilter(c)}
                            style={{ padding: '8px 10px', background: catFilter === c ? '#111' : 'none', border: 'none', borderBottom: '1px solid #1a1a1a', color: catFilter === c ? CATEGORY_COLORS[c] : '#666', cursor: 'pointer', textAlign: 'left', fontSize: 11, fontWeight: catFilter === c ? 700 : 400 }}>
                            {c} ({count})
                        </button>
                    );
                })}
            </div>

            {/* Tool list */}
            <div style={{ flex: 1, overflow: 'auto', padding: 10 }}>
                {filtered.map(t => (
                    <ToolCard
                        key={t.id} tool={t}
                        expanded={expanded === t.id}
                        onToggle={() => setExpanded(e => e === t.id ? null : t.id)}
                    />
                ))}
            </div>
        </div>
    );
}

const STATUS_META: Record<ServerStatus, { color: string; dot: string; label: string }> = {
    connected:    { color: '#40a040', dot: '●', label: 'Connected' },
    connecting:   { color: '#d0a030', dot: '◌', label: 'Connecting…' },
    disconnected: { color: '#555555', dot: '○', label: 'Disconnected' },
    error:        { color: '#c04040', dot: '✗', label: 'Error' },
};

const TRANSPORT_COLORS: Record<Transport, string> = {
    stdio:     '#7ab4ff',
    sse:       '#c080ff',
    websocket: '#40c0a0',
};

function ServerCard({ server, liveBackend, onStatusChange }: { server: McpServer; liveBackend: boolean; onStatusChange: (id: string, status: ServerStatus, toolCount?: number, error?: string) => void }) {
    const m = STATUS_META[server.status];

    async function toggle() {
        if (server.status === 'connected') {
            onStatusChange(server.id, 'disconnected');
            if (liveBackend) {
                try { await disconnectMcpServer(server.id); } catch { /* ignore */ }
            }
        } else if (server.status === 'disconnected' || server.status === 'error') {
            onStatusChange(server.id, 'connecting');
            if (liveBackend) {
                try {
                    const state = await connectMcpServer(server.id);
                    onStatusChange(server.id, state.status as ServerStatus, state.toolCount, state.error);
                } catch (e) {
                    onStatusChange(server.id, 'error', 0, String(e));
                }
            } else {
                setTimeout(() => onStatusChange(server.id, 'connected', 0), 1200);
            }
        }
    }

    return (
        <div style={{ border: '1px solid #1e1e1e', borderRadius: 6, padding: 12, marginBottom: 8, background: '#0d0d0d' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                <span style={{ color: m.color, fontSize: 14 }}>{m.dot}</span>
                <span style={{ fontSize: 12, fontWeight: 700, color: '#d0d0d0', flex: 1 }}>{server.name}</span>
                <span style={{ fontSize: 10, color: TRANSPORT_COLORS[server.transport], fontFamily: 'monospace' }}>{server.transport}</span>
                {server.toolCount > 0 && <span style={{ fontSize: 10, color: '#888' }}>{server.toolCount} tools</span>}
            </div>
            <div style={{ fontSize: 10, color: '#555', fontFamily: 'monospace', marginBottom: 6, wordBreak: 'break-all' }}>{server.endpoint}</div>
            {server.error && <div style={{ fontSize: 11, color: '#c04040', marginBottom: 6 }}>{server.error}</div>}
            {server.connectedAt && <div style={{ fontSize: 10, color: '#555', marginBottom: 6 }}>connected: {new Date(server.connectedAt).toLocaleTimeString()}</div>}
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <button onClick={toggle} disabled={server.status === 'connecting'}
                    style={{
                        padding: '4px 12px', fontSize: 11, borderRadius: 3,
                        cursor: server.status === 'connecting' ? 'default' : 'pointer', fontWeight: 600,
                        background: server.status === 'connected' ? '#1a0a0a' : '#0a1a0a',
                        border: `1px solid ${server.status === 'connected' ? '#7a3a3a' : '#3a7a3a'}`,
                        color: server.status === 'connected' ? '#d06060' : server.status === 'connecting' ? '#888' : '#60d060',
                    }}>
                    {server.status === 'connected' ? 'Disconnect' : server.status === 'connecting' ? 'Connecting…' : 'Connect'}
                </button>
                <span style={{ fontSize: 11, color: m.color }}>{m.label}</span>
            </div>
        </div>
    );
}

interface AddServerForm {
    id: string;
    name: string;
    transport: McpTransport;
    command: string;
}

function AddServerModal({ onAdd, onClose }: { onAdd: (form: AddServerForm) => Promise<void>; onClose: () => void }) {
    const [form, setForm] = React.useState<AddServerForm>({ id: '', name: '', transport: 'stdio', command: '' });
    const [saving, setSaving] = React.useState(false);
    const [err, setErr] = React.useState('');

    async function submit(e: React.FormEvent) {
        e.preventDefault();
        if (!form.id || !form.name || !form.command) { setErr('All fields are required'); return; }
        setSaving(true);
        try { await onAdd(form); onClose(); }
        catch (ex) { setErr(String(ex)); }
        finally { setSaving(false); }
    }

    const inputStyle: React.CSSProperties = { width: '100%', boxSizing: 'border-box', background: '#111', border: '1px solid #333', color: '#ddd', borderRadius: 3, padding: '5px 8px', fontSize: 11, outline: 'none' };

    return (
        <div style={{ position: 'fixed', inset: 0, background: '#000a', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }}>
            <div style={{ background: '#111', border: '1px solid #333', borderRadius: 8, padding: 20, width: 380, boxShadow: '0 8px 32px #0008' }}>
                <div style={{ display: 'flex', alignItems: 'center', marginBottom: 16 }}>
                    <span style={{ fontSize: 13, fontWeight: 700, color: '#7ab4ff' }}>Add MCP Server</span>
                    <button onClick={onClose} style={{ marginLeft: 'auto', background: 'none', border: 'none', color: '#666', cursor: 'pointer', fontSize: 16 }}>×</button>
                </div>
                <form onSubmit={submit}>
                    {[['id', 'Server ID (unique slug)'], ['name', 'Display name'], ['command', 'Shell command']] .map(([field, label]) => (
                        <div key={field} style={{ marginBottom: 10 }}>
                            <div style={{ fontSize: 10, color: '#777', marginBottom: 3 }}>{label}</div>
                            <input style={inputStyle} value={(form as Record<string, string>)[field]} onChange={e => setForm(f => ({ ...f, [field]: e.target.value }))} />
                        </div>
                    ))}
                    <div style={{ marginBottom: 12 }}>
                        <div style={{ fontSize: 10, color: '#777', marginBottom: 3 }}>Transport</div>
                        <select value={form.transport} onChange={e => setForm(f => ({ ...f, transport: e.target.value as McpTransport }))}
                            style={{ ...inputStyle, cursor: 'pointer' }}>
                            <option value="stdio">stdio</option>
                            <option value="sse">SSE</option>
                            <option value="websocket">WebSocket</option>
                        </select>
                    </div>
                    {err && <div style={{ fontSize: 11, color: '#c04040', marginBottom: 8 }}>{err}</div>}
                    <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                        <button type="button" onClick={onClose} style={{ padding: '6px 14px', background: 'none', border: '1px solid #333', color: '#888', borderRadius: 3, cursor: 'pointer', fontSize: 11 }}>Cancel</button>
                        <button type="submit" disabled={saving} style={{ padding: '6px 14px', background: '#0a1a0a', border: '1px solid #3a7a3a', color: saving ? '#555' : '#60d060', borderRadius: 3, cursor: saving ? 'default' : 'pointer', fontSize: 11, fontWeight: 700 }}>
                            {saving ? 'Adding…' : 'Add Server'}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    );
}

function ServersTab() {
    const [servers, setServers] = React.useState<McpServer[]>(DEMO_SERVERS);
    const [liveBackend, setLiveBackend] = React.useState(false);
    const [loading, setLoading] = React.useState(true);
    const [showAdd, setShowAdd] = React.useState(false);

    React.useEffect(() => {
        (async () => {
            const reachable = await isBackendReachable();
            setLiveBackend(reachable);
            if (reachable) {
                try {
                    const live = await listMcpServers();
                    setServers(live.map(stateToServer));
                } catch { /* stay with demo */ }
            }
            setLoading(false);
        })();
    }, []);

    function handleStatusChange(id: string, status: ServerStatus, toolCount?: number, error?: string) {
        setServers(prev => prev.map(s => s.id === id ? { ...s, status, toolCount: toolCount ?? s.toolCount, error } : s));
    }

    async function handleAdd(form: AddServerForm) {
        const state = await addMcpServer({ id: form.id, name: form.name, transport: form.transport, command: form.command });
        setServers(prev => [...prev, stateToServer(state)]);
    }

    async function handleRemove(id: string) {
        if (liveBackend) await removeMcpServer(id);
        setServers(prev => prev.filter(s => s.id !== id));
    }

    const connected = servers.filter(s => s.status === 'connected').length;

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #1a1a1a', fontSize: 11, background: '#0d0d0d', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ color: '#555' }}>
                    {loading ? 'Loading…' : `${connected} of ${servers.length} connected · ${servers.reduce((a, s) => a + s.toolCount, 0)} tools`}
                </span>
                {liveBackend
                    ? <span style={{ fontSize: 10, color: '#40a040', marginLeft: 'auto' }}>● live backend</span>
                    : <span style={{ fontSize: 10, color: '#555', marginLeft: 'auto' }}>○ demo mode</span>}
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: 10 }}>
                {servers.map(s => (
                    <div key={s.id} style={{ position: 'relative' }}>
                        <ServerCard server={s} liveBackend={liveBackend} onStatusChange={handleStatusChange} />
                        {liveBackend && (
                            <button onClick={() => handleRemove(s.id)}
                                title="Remove server"
                                style={{ position: 'absolute', top: 10, right: 10, background: 'none', border: 'none', color: '#444', cursor: 'pointer', fontSize: 12, padding: '0 4px' }}>
                                ✕
                            </button>
                        )}
                    </div>
                ))}
                <div style={{ marginTop: 8, padding: '8px 0', borderTop: '1px solid #1a1a1a' }}>
                    <button onClick={() => setShowAdd(true)} style={{ padding: '6px 14px', background: '#111', border: '1px solid #333', color: '#888', borderRadius: 4, cursor: 'pointer', fontSize: 11 }}>
                        + Add MCP Server
                    </button>
                </div>
            </div>
            {showAdd && <AddServerModal onAdd={handleAdd} onClose={() => setShowAdd(false)} />}
        </div>
    );
}

function LogsTab() {
    const [logs, setLogs] = React.useState(INITIAL_LOGS);
    const [expanded, setExpanded] = React.useState<string | null>(null);

    function clear() { setLogs([]); }

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
            <div style={{ display: 'flex', alignItems: 'center', padding: '6px 12px', borderBottom: '1px solid #1a1a1a', background: '#0d0d0d' }}>
                <span style={{ fontSize: 11, color: '#555', flex: 1 }}>{logs.length} recent invocations</span>
                <button onClick={clear} style={{ padding: '3px 8px', background: 'none', border: '1px solid #2a2a2a', color: '#666', borderRadius: 3, cursor: 'pointer', fontSize: 10 }}>Clear</button>
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {logs.length === 0 && (
                    <div style={{ padding: 24, textAlign: 'center', color: '#444', fontSize: 12 }}>No invocations recorded.</div>
                )}
                {logs.map(log => (
                    <div key={log.id} style={{ borderBottom: '1px solid #1a1a1a' }}>
                        <div
                            onClick={() => setExpanded(e => e === log.id ? null : log.id)}
                            style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '7px 12px', cursor: 'pointer', background: expanded === log.id ? '#0d110d' : 'transparent' }}
                        >
                            <span style={{ fontSize: 10, color: log.success ? '#40a040' : '#c04040', fontFamily: 'monospace', width: 12 }}>{log.success ? '✓' : '✗'}</span>
                            <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace', width: 52 }}>{log.ts}</span>
                            <span style={{ fontSize: 11, color: CATEGORY_COLORS[(getAllTools().find(t => t.id === log.tool)?.category) ?? 'api'], fontFamily: 'monospace', width: 110 }}>{log.tool}</span>
                            <span style={{ fontSize: 11, color: '#7ab4ff', width: 110 }}>{log.agent}</span>
                            <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace', marginLeft: 'auto' }}>{log.durationMs}ms</span>
                        </div>
                        {expanded === log.id && (
                            <div style={{ padding: '6px 12px 10px 34px', background: '#0a0a0a' }}>
                                <div style={{ fontSize: 10, color: '#555', marginBottom: 3 }}>INPUT</div>
                                <pre style={{ margin: '0 0 8px', fontSize: 10, color: '#a0c0a0', background: '#0d0d0d', padding: '5px 8px', borderRadius: 3, overflow: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{log.input}</pre>
                                <div style={{ fontSize: 10, color: '#555', marginBottom: 3 }}>OUTPUT</div>
                                <pre style={{ margin: 0, fontSize: 10, color: log.success ? '#a0d080' : '#d08080', background: '#0d0d0d', padding: '5px 8px', borderRadius: 3, overflow: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{log.output}</pre>
                            </div>
                        )}
                    </div>
                ))}
            </div>
        </div>
    );
}

function McpView() {
    const [tab, setTab] = React.useState<McpTab>('tools');
    const allTools = getAllTools();

    const tabBtn = (t: McpTab, label: string) => (
        <button onClick={() => setTab(t)}
            style={{ padding: '8px 14px', background: 'none', border: 'none', borderBottom: tab === t ? '2px solid #7ab4ff' : '2px solid transparent', color: tab === t ? '#7ab4ff' : '#666', cursor: 'pointer', fontSize: 12, fontWeight: tab === t ? 700 : 400 }}>
            {label}
        </button>
    );

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {tabBtn('tools', `Tools (${allTools.length})`)}
                {tabBtn('servers', 'MCP Servers')}
                {tabBtn('logs', `Logs (${INITIAL_LOGS.length})`)}
            </div>
            <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                {tab === 'tools'   && <ToolsTab />}
                {tab === 'servers' && <ServersTab />}
                {tab === 'logs'    && <LogsTab />}
            </div>
        </div>
    );
}

@injectable()
export class McpPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:mcp';
    static readonly LABEL = 'MCP / Tools';

    @postConstruct()
    protected init(): void {
        this.id = McpPanelWidget.ID;
        this.title.label = McpPanelWidget.LABEL;
        this.title.caption = 'MCP servers, tool registry, and invocation logs';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-plug';
        this.update();
    }

    protected render(): React.ReactNode {
        return <McpView />;
    }
}

@injectable()
export class McpPanelContribution extends AbstractViewContribution<McpPanelWidget> {
    constructor() {
        super({ widgetId: McpPanelWidget.ID, widgetName: McpPanelWidget.LABEL, defaultWidgetOptions: { area: 'left' }, toggleCommandId: McpPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(McpPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
