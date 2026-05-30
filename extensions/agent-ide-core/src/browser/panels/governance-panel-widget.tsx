import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { GovernancePanelCommand } from '../agent-ide-commands';
import {
    isBackendReachable, listPolicies, updatePolicy, deletePolicy,
    listAuditLog, listApprovals, approveRequest, rejectRequest,
    LivePolicy, AuditEntry, PendingApproval, PolicyDecisionAction,
} from '../runtime/backend-client';
import { getSession } from '../runtime/session-store';

type GovTab = 'policies' | 'audit' | 'approvals';

// ─── Shared badges ────────────────────────────────────────────────────────────

const ACTION_META: Record<PolicyDecisionAction | string, { label: string; color: string; bg: string }> = {
    'allow':            { label: 'ALLOW',    color: '#40a040', bg: '#0a1a0a' },
    'deny':             { label: 'DENY',     color: '#c04040', bg: '#1a0a0a' },
    'require-approval': { label: 'APPROVE',  color: '#d0a030', bg: '#1a1400' },
};

function ActionBadge({ action }: { action: string }) {
    const m = ACTION_META[action] ?? { label: action.toUpperCase(), color: '#888', bg: '#111' };
    return <span style={{ background: m.bg, color: m.color, padding: '1px 6px', borderRadius: 3, fontSize: 10, fontWeight: 700, fontFamily: 'monospace', border: `1px solid ${m.color}22` }}>{m.label}</span>;
}

function DecisionDot({ decision }: { decision?: string }) {
    const colors: Record<string, string> = { allow: '#40a040', 'require-approval': '#d0a030', deny: '#c04040', approved: '#40a040', rejected: '#c04040', timeout: '#666' };
    const color = colors[decision ?? ''] ?? '#888';
    return <span style={{ color, fontSize: 12 }}>●</span>;
}

// ─── Policies tab ─────────────────────────────────────────────────────────────

function PoliciesTab({ liveBackend, token }: { liveBackend: boolean; token: string | null }) {
    const [policies, setPolicies] = React.useState<LivePolicy[]>([]);
    const [selected, setSelected] = React.useState<LivePolicy | null>(null);
    const [loading, setLoading]   = React.useState(true);

    React.useEffect(() => {
        if (!liveBackend) { setLoading(false); return; }
        listPolicies(token ?? undefined).then(p => { setPolicies(p); setLoading(false); }).catch(() => setLoading(false));
    }, [liveBackend]);

    async function toggleEnabled(p: LivePolicy) {
        const updated = await updatePolicy(p.id, { enabled: !p.enabled }, token ?? undefined);
        setPolicies(prev => prev.map(x => x.id === p.id ? updated : x));
        if (selected?.id === p.id) setSelected(updated);
    }

    async function handleDelete(p: LivePolicy) {
        await deletePolicy(p.id, token ?? undefined);
        setPolicies(prev => prev.filter(x => x.id !== p.id));
        if (selected?.id === p.id) setSelected(null);
    }

    return (
        <div style={{ display: 'flex', height: '100%', overflow: 'hidden' }}>
            <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
                {loading && <div style={{ color: '#555', fontSize: 12, padding: 12 }}>Loading…</div>}
                {!loading && policies.length === 0 && (
                    <div style={{ color: '#555', fontSize: 12, padding: 12, textAlign: 'center' }}>
                        No policies yet.{liveBackend ? '' : ' Start backend to load policies.'}
                    </div>
                )}
                {policies.map(p => (
                    <div key={p.id} onClick={() => setSelected(s => s?.id === p.id ? null : p)}
                        style={{ border: `1px solid ${selected?.id === p.id ? '#d0a030' : '#1e1e1e'}`, borderRadius: 6, padding: 12, marginBottom: 8, cursor: 'pointer', background: selected?.id === p.id ? '#1a1200' : '#0d0d0d' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                            <span style={{ fontSize: 12, fontWeight: 700, color: p.enabled ? '#e0e0e0' : '#555', flex: 1 }}>{p.name}</span>
                            <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>v{p.version}</span>
                            <span style={{ fontSize: 10, color: p.enabled ? '#40a040' : '#604040', fontFamily: 'monospace' }}>{p.enabled ? '● ON' : '○ OFF'}</span>
                        </div>
                        <div style={{ fontSize: 11, color: '#666', marginBottom: 6 }}>{p.description}</div>
                        <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
                            {p.rules.map(r => <ActionBadge key={r.id} action={r.action} />)}
                        </div>
                    </div>
                ))}
            </div>
            {selected && (
                <div style={{ width: 300, borderLeft: '1px solid #1e1e1e', overflow: 'auto', padding: 12, background: '#0d0d0d', display: 'flex', flexDirection: 'column', gap: 8 }}>
                    <div style={{ fontSize: 13, fontWeight: 700, color: '#e0e0e0' }}>{selected.name}</div>
                    <div style={{ fontSize: 11, color: '#666', lineHeight: 1.5 }}>{selected.description}</div>
                    <div style={{ fontSize: 10, color: '#555' }}>RULES ({selected.rules.length})</div>
                    {selected.rules.map(r => (
                        <div key={r.id} style={{ border: '1px solid #1a1a1a', borderRadius: 4, padding: 8, background: '#111' }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                                <ActionBadge action={r.action} />
                                <span style={{ fontSize: 10, color: '#888', fontFamily: 'monospace' }}>{r.tools.join(', ')}</span>
                            </div>
                            <div style={{ fontSize: 10, color: '#666', fontStyle: 'italic' }}>{r.reason}</div>
                        </div>
                    ))}
                    <div style={{ fontSize: 10, color: '#555', lineHeight: 1.8 }}>
                        <div>ID: {selected.id}</div>
                        <div>Priority: {selected.priority}</div>
                        <div>Created: {selected.createdAt.slice(0, 10)}</div>
                        <div>Updated: {selected.updatedAt.slice(0, 10)}</div>
                    </div>
                    {liveBackend && (
                        <div style={{ display: 'flex', gap: 6 }}>
                            <button onClick={() => toggleEnabled(selected)}
                                style={{ flex: 1, padding: '5px 0', background: selected.enabled ? '#1a0a0a' : '#0a1a0a', border: `1px solid ${selected.enabled ? '#7a3a3a' : '#3a7a3a'}`, color: selected.enabled ? '#d06060' : '#60d060', borderRadius: 3, cursor: 'pointer', fontSize: 11, fontWeight: 600 }}>
                                {selected.enabled ? 'Disable' : 'Enable'}
                            </button>
                            <button onClick={() => handleDelete(selected)}
                                style={{ padding: '5px 10px', background: '#1a0a0a', border: '1px solid #5a2a2a', color: '#d06060', borderRadius: 3, cursor: 'pointer', fontSize: 11 }}>
                                Delete
                            </button>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}

// ─── Audit tab ────────────────────────────────────────────────────────────────

function AuditTab({ liveBackend, token }: { liveBackend: boolean; token: string | null }) {
    const [entries, setEntries] = React.useState<AuditEntry[]>([]);
    const [filter, setFilter]   = React.useState('');
    const [loading, setLoading] = React.useState(true);

    React.useEffect(() => {
        if (!liveBackend) {
            setLoading(false);
            // Demo entries
            setEntries([
                { id: 'demo1', tenantId: 'user_demo', timestamp: new Date().toISOString(), event: 'tool:call', agentId: 'ResearchAgent', toolId: 'browser',    decision: 'allow',            policyId: 'builtin-tool-safety', metadata: {} },
                { id: 'demo2', tenantId: 'user_demo', timestamp: new Date().toISOString(), event: 'tool:call', agentId: 'CoderAgent',    toolId: 'shell',      decision: 'require-approval', policyId: 'builtin-tool-safety', metadata: {} },
                { id: 'demo3', tenantId: 'user_demo', timestamp: new Date().toISOString(), event: 'tool:call', agentId: 'CoderAgent',    toolId: 'file_rw',    decision: 'allow',            policyId: 'builtin-tool-safety', metadata: {} },
            ]);
            return;
        }
        listAuditLog({ limit: 100 }, token ?? undefined).then(e => { setEntries(e); setLoading(false); }).catch(() => setLoading(false));
    }, [liveBackend]);

    const filtered = filter
        ? entries.filter(e => e.event.includes(filter) || e.toolId?.includes(filter) || e.agentId?.includes(filter) || e.decision?.includes(filter))
        : entries;

    return (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            <div style={{ padding: '6px 12px', borderBottom: '1px solid #1a1a1a', background: '#0d0d0d', display: 'flex', gap: 8, alignItems: 'center' }}>
                <input value={filter} onChange={e => setFilter(e.target.value)} placeholder="Filter by event, tool, agent…"
                    style={{ flex: 1, background: '#111', border: '1px solid #2a2a2a', color: '#ddd', borderRadius: 3, padding: '4px 8px', fontSize: 11, outline: 'none' }} />
                <span style={{ fontSize: 10, color: '#555' }}>{filtered.length} entries</span>
                {liveBackend && (
                    <button onClick={() => listAuditLog({ limit: 100 }, token ?? undefined).then(setEntries).catch(() => {})}
                        style={{ padding: '3px 8px', background: 'none', border: '1px solid #2a2a2a', color: '#888', borderRadius: 3, cursor: 'pointer', fontSize: 10 }}>↻</button>
                )}
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {loading && <div style={{ padding: 12, color: '#555', fontSize: 12 }}>Loading…</div>}
                {filtered.map(e => (
                    <div key={e.id} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 12px', borderBottom: '1px solid #0f0f0f', fontSize: 11 }}>
                        <DecisionDot decision={e.decision} />
                        <span style={{ color: '#444', fontFamily: 'monospace', fontSize: 10, width: 56, flexShrink: 0 }}>{new Date(e.timestamp).toLocaleTimeString()}</span>
                        <span style={{ color: '#7ab4ff', width: 100, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flexShrink: 0 }}>{e.agentId ?? '—'}</span>
                        <span style={{ color: '#a0a0a0', fontFamily: 'monospace', fontSize: 10, width: 90, flexShrink: 0 }}>{e.event}</span>
                        <span style={{ color: '#d0d0a0', fontFamily: 'monospace', fontSize: 10, flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{e.toolId ?? ''}</span>
                        {e.decision && <ActionBadge action={e.decision} />}
                    </div>
                ))}
            </div>
        </div>
    );
}

// ─── Approvals tab ────────────────────────────────────────────────────────────

function ApprovalsTab({ liveBackend, token }: { liveBackend: boolean; token: string | null }) {
    const [approvals, setApprovals] = React.useState<PendingApproval[]>([]);
    const [loading, setLoading]     = React.useState(true);

    const load = () => {
        if (!liveBackend) {
            setLoading(false);
            setApprovals([{
                id: 'demo-appr', tenantId: 'user_demo', runId: 'run_demo', agentId: 'CoderAgent',
                toolId: 'shell', input: { command: 'ls -la /workspace' }, reason: 'Shell execution requires human approval.',
                requestedAt: new Date(Date.now() - 5 * 60000).toISOString(), status: 'pending',
            }]);
            return;
        }
        listApprovals(token ?? undefined).then(a => { setApprovals(a); setLoading(false); }).catch(() => setLoading(false));
    };

    React.useEffect(() => { load(); }, [liveBackend]);

    // Auto-poll every 5s when live
    React.useEffect(() => {
        if (!liveBackend) return;
        const t = setInterval(() => listApprovals(token ?? undefined).then(setApprovals).catch(() => {}), 5000);
        return () => clearInterval(t);
    }, [liveBackend]);

    async function handleApprove(id: string) {
        if (liveBackend) await approveRequest(id, token ?? undefined);
        setApprovals(prev => prev.map(a => a.id === id ? { ...a, status: 'approved' as const, resolvedAt: new Date().toISOString() } : a));
    }

    async function handleReject(id: string) {
        if (liveBackend) await rejectRequest(id, token ?? undefined);
        setApprovals(prev => prev.map(a => a.id === id ? { ...a, status: 'rejected' as const, resolvedAt: new Date().toISOString() } : a));
    }

    const pending  = approvals.filter(a => a.status === 'pending');
    const resolved = approvals.filter(a => a.status !== 'pending');

    return (
        <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
            {loading && <div style={{ color: '#555', fontSize: 12 }}>Loading…</div>}

            {pending.length === 0 && !loading && (
                <div style={{ textAlign: 'center', color: '#555', fontSize: 12, marginTop: 24, marginBottom: 16 }}>
                    ✓ No pending approvals
                </div>
            )}

            {pending.map(item => (
                <div key={item.id} style={{ border: '1px solid #3a2a0a', borderRadius: 6, padding: 14, marginBottom: 10, background: '#1a1400' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                        <span style={{ fontSize: 12, fontWeight: 700, color: '#d0a030' }}>{item.agentId}</span>
                        <span style={{ fontSize: 11, fontFamily: 'monospace', color: '#a0a0a0' }}>→ {item.toolId}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: '#666' }}>
                            {Math.round((Date.now() - new Date(item.requestedAt).getTime()) / 60000)}m ago
                        </span>
                    </div>
                    <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>Reason: {item.reason}</div>
                    {Object.keys(item.input).length > 0 && (
                        <pre style={{ fontSize: 10, color: '#a0c0a0', background: '#0d0d0d', padding: 6, borderRadius: 3, margin: '8px 0', overflow: 'auto', maxHeight: 80, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
                            {JSON.stringify(item.input, null, 2)}
                        </pre>
                    )}
                    <div style={{ display: 'flex', gap: 8 }}>
                        <button onClick={() => handleApprove(item.id)}
                            style={{ padding: '5px 16px', background: '#0a1a0a', border: '1px solid #3a7a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
                            ✓ Approve
                        </button>
                        <button onClick={() => handleReject(item.id)}
                            style={{ padding: '5px 16px', background: '#1a0a0a', border: '1px solid #7a3a3a', color: '#d06060', borderRadius: 4, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
                            ✗ Reject
                        </button>
                    </div>
                </div>
            ))}

            {resolved.length > 0 && (
                <>
                    <div style={{ fontSize: 10, color: '#555', marginTop: 12, marginBottom: 8, textTransform: 'uppercase', letterSpacing: 1 }}>Resolved ({resolved.length})</div>
                    {resolved.map(item => (
                        <div key={item.id} style={{ border: '1px solid #1e1e1e', borderRadius: 4, padding: 10, marginBottom: 6, background: '#0d0d0d', display: 'flex', alignItems: 'center', gap: 8 }}>
                            <DecisionDot decision={item.status} />
                            <span style={{ fontSize: 11, color: '#888', flex: 1 }}>{item.agentId} → {item.toolId}</span>
                            <span style={{ fontSize: 10, color: item.status === 'approved' ? '#40a040' : '#c04040', fontFamily: 'monospace' }}>{item.status.toUpperCase()}</span>
                            <span style={{ fontSize: 10, color: '#555' }}>{item.resolvedBy}</span>
                        </div>
                    ))}
                </>
            )}
        </div>
    );
}

// ─── Root view ────────────────────────────────────────────────────────────────

function GovernanceView() {
    const [tab, setTab]          = React.useState<GovTab>('policies');
    const [liveBackend, setLive] = React.useState(false);
    const [pendingCount, setPending] = React.useState(0);
    const token = getSession()?.token ?? null;

    React.useEffect(() => {
        isBackendReachable().then(r => {
            setLive(r);
            if (r) listApprovals(token ?? undefined).then(a => setPending(a.filter(x => x.status === 'pending').length)).catch(() => {});
        });
    }, []);

    // Keep pending badge updated
    React.useEffect(() => {
        if (!liveBackend) return;
        const t = setInterval(() => listApprovals(token ?? undefined).then(a => setPending(a.filter(x => x.status === 'pending').length)).catch(() => {}), 8000);
        return () => clearInterval(t);
    }, [liveBackend]);

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {(['policies', 'audit', 'approvals'] as GovTab[]).map(t => (
                    <button key={t} onClick={() => setTab(t)}
                        style={{ padding: '8px 14px', background: 'none', border: 'none', borderBottom: tab === t ? '2px solid #d0a030' : '2px solid transparent', color: tab === t ? '#d0a030' : '#666', cursor: 'pointer', fontSize: 12, fontWeight: tab === t ? 700 : 400, textTransform: 'capitalize', display: 'flex', alignItems: 'center', gap: 5 }}>
                        {t}
                        {t === 'approvals' && pendingCount > 0 && (
                            <span style={{ background: '#d0a030', color: '#000', borderRadius: 8, fontSize: 10, fontWeight: 700, padding: '0 5px', lineHeight: 1.6 }}>{pendingCount}</span>
                        )}
                    </button>
                ))}
                <span style={{ marginLeft: 'auto', padding: '8px 12px', fontSize: 10, color: liveBackend ? '#40a040' : '#555' }}>
                    {liveBackend ? '● live' : '○ demo'}
                </span>
            </div>
            <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                {tab === 'policies'  && <PoliciesTab  liveBackend={liveBackend} token={token} />}
                {tab === 'audit'     && <AuditTab     liveBackend={liveBackend} token={token} />}
                {tab === 'approvals' && <ApprovalsTab liveBackend={liveBackend} token={token} />}
            </div>
        </div>
    );
}

@injectable()
export class GovernancePanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:governance';
    static readonly LABEL = 'Governance';

    @postConstruct()
    protected init(): void {
        this.id = GovernancePanelWidget.ID;
        this.title.label = GovernancePanelWidget.LABEL;
        this.title.caption = 'Policies, rules, audit log, and approval gate';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-shield';
        this.update();
    }

    protected render(): React.ReactNode {
        return <GovernanceView />;
    }
}

@injectable()
export class GovernancePanelContribution extends AbstractViewContribution<GovernancePanelWidget> {
    constructor() {
        super({ widgetId: GovernancePanelWidget.ID, widgetName: GovernancePanelWidget.LABEL, defaultWidgetOptions: { area: 'right' }, toggleCommandId: GovernancePanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(GovernancePanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
