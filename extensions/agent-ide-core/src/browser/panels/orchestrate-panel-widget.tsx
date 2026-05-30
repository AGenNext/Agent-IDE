import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { OrchestratePanelCommand } from '../agent-ide-commands';
import {
    startOrchestration, listOrchestrationRuns, getOrchestrationRun,
    type OrchestrationRun, type OrchestrationTask,
} from '../runtime/backend-client';

type OrchestrateTab = 'compose' | 'graph' | 'history';

const GRAPH_NODES: Array<{ id: string; label: string; icon: string }> = [
    { id: 'start',   label: 'Start',   icon: '◉' },
    { id: 'plan',    label: 'Plan',    icon: '⊞' },
    { id: 'execute', label: 'Execute', icon: '⚙' },
    { id: 'review',  label: 'Review',  icon: '◈' },
    { id: 'deliver', label: 'Deliver', icon: '⟹' },
    { id: 'done',    label: 'Done',    icon: '✓' },
];

const NODE_ORDER = GRAPH_NODES.map(n => n.id);

function nodeColor(current: string, nodeId: string): { bg: string; fg: string; border: string } {
    const ci = NODE_ORDER.indexOf(current);
    const ni = NODE_ORDER.indexOf(nodeId);
    if (current === 'error' && nodeId === current) return { bg: '#3a1a1a', fg: '#ff6060', border: '#6a2a2a' };
    if (ni < ci || current === 'done') return { bg: '#0d1a0d', fg: '#60d060', border: '#2a4a2a' }; // done
    if (ni === ci) return { bg: '#1a2a3a', fg: '#7ab4ff', border: '#2a4a6a' };                      // active
    return { bg: '#0d0d0d', fg: '#444', border: '#1e1e1e' };                                         // pending
}

function GraphView({ run }: { run: OrchestrationRun }) {
    const current = run.node;
    return (
        <div>
            {/* Node row */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 4, padding: '12px 0', overflowX: 'auto' }}>
                {GRAPH_NODES.map((n, i) => {
                    const c = nodeColor(current, n.id);
                    return (
                        <React.Fragment key={n.id}>
                            <div style={{ flexShrink: 0, padding: '8px 14px', borderRadius: 6, border: `1px solid ${c.border}`, background: c.bg, textAlign: 'center', minWidth: 72 }}>
                                <div style={{ fontSize: 16, color: c.fg }}>{n.icon}</div>
                                <div style={{ fontSize: 10, color: c.fg, fontWeight: 700, marginTop: 3 }}>{n.label}</div>
                            </div>
                            {i < GRAPH_NODES.length - 1 && (
                                <div style={{ color: '#333', fontSize: 18, flexShrink: 0 }}>→</div>
                            )}
                        </React.Fragment>
                    );
                })}
            </div>

            {/* Plan summary */}
            {run.plan && (
                <div style={{ padding: '8px 10px', background: '#0d1a0d', border: '1px solid #1a3a1a', borderRadius: 4, fontSize: 11, color: '#80c080', marginBottom: 10 }}>
                    <span style={{ color: '#555', marginRight: 6 }}>Plan:</span>{run.plan}
                </div>
            )}

            {/* Tasks */}
            {run.tasks.length > 0 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                    {run.tasks.map(t => <TaskCard key={t.id} task={t} />)}
                </div>
            )}

            {/* Review verdict */}
            {run.review && (
                <div style={{ marginTop: 10, padding: '8px 10px', background: run.review.startsWith('APPROVED') ? '#0d1a0d' : '#1a1a0d', border: `1px solid ${run.review.startsWith('APPROVED') ? '#2a4a2a' : '#4a3a1a'}`, borderRadius: 4 }}>
                    <span style={{ fontSize: 11, color: '#888', marginRight: 6 }}>Review:</span>
                    <span style={{ fontSize: 11, color: run.review.startsWith('APPROVED') ? '#60d060' : '#d0a030' }}>{run.review}</span>
                </div>
            )}

            {/* Result */}
            {run.result && (
                <div style={{ marginTop: 12, padding: '10px 12px', background: '#111', border: '1px solid #2a2a3a', borderRadius: 6 }}>
                    <div style={{ fontSize: 11, fontWeight: 700, color: '#888', marginBottom: 6 }}>RESULT</div>
                    <div style={{ fontSize: 12, color: '#c8c8c8', lineHeight: 1.6, whiteSpace: 'pre-wrap' }}>{run.result}</div>
                </div>
            )}
        </div>
    );
}

const TASK_STATUS_COLOR: Record<string, string> = {
    pending: '#555',
    running: '#7ab4ff',
    done:    '#60d060',
    failed:  '#ff6060',
};

const TASK_STATUS_ICON: Record<string, string> = {
    pending: '○',
    running: '●',
    done:    '✓',
    failed:  '✗',
};

function TaskCard({ task }: { task: OrchestrationTask }) {
    const [expanded, setExpanded] = React.useState(false);
    const c = TASK_STATUS_COLOR[task.status] ?? '#555';
    return (
        <div style={{ border: '1px solid #1e1e1e', borderRadius: 4, overflow: 'hidden' }}>
            <div onClick={() => setExpanded(e => !e)} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px', cursor: 'pointer', background: '#0d0d0d' }}>
                <span style={{ color: c, fontSize: 14 }}>{TASK_STATUS_ICON[task.status] ?? '○'}</span>
                <span style={{ fontSize: 12, color: '#ccc', flex: 1, fontWeight: 600 }}>{task.title}</span>
                <span style={{ fontSize: 10, color: '#555', border: '1px solid #2a2a2a', borderRadius: 3, padding: '1px 5px' }}>{task.agentRole}</span>
                <span style={{ fontSize: 10, color: c, border: `1px solid ${c}30`, borderRadius: 3, padding: '1px 5px' }}>{task.status}</span>
                <span style={{ color: '#444', fontSize: 12 }}>{expanded ? '▲' : '▼'}</span>
            </div>
            {expanded && (
                <div style={{ padding: '8px 10px', background: '#080808', borderTop: '1px solid #1a1a1a' }}>
                    <div style={{ fontSize: 11, color: '#666', marginBottom: 6 }}>{task.description}</div>
                    {task.tools.length > 0 && (
                        <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', marginBottom: 6 }}>
                            {task.tools.map(t => <span key={t} style={{ fontSize: 10, color: '#60b0ff', border: '1px solid #1a3a4a', borderRadius: 3, padding: '1px 4px' }}>{t}</span>)}
                        </div>
                    )}
                    {task.runId && <div style={{ fontSize: 10, color: '#444', fontFamily: 'monospace' }}>run: {task.runId}</div>}
                    {task.output && (
                        <div style={{ marginTop: 6, padding: '6px 8px', background: '#111', borderRadius: 3, fontSize: 11, color: '#a0c0a0', whiteSpace: 'pre-wrap', maxHeight: 200, overflow: 'auto' }}>
                            {task.output}
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}

const MODELS = [
    'claude-sonnet-4-6', 'claude-opus-4-8', 'claude-haiku-4-5',
    'gpt-4o', 'gpt-4o-mini', 'gemini-1.5-pro',
];

const ALL_TOOLS = ['web_search', 'http_client', 'browser', 'file_rw', 'vector_search', 'code_exec', 'shell', 'db_query'];

function ComposeTab({ onSubmit }: { onSubmit: (id: string) => void }) {
    const [goal, setGoal]       = React.useState('');
    const [model, setModel]     = React.useState('claude-sonnet-4-6');
    const [tools, setTools]     = React.useState<string[]>(['web_search', 'http_client', 'vector_search']);
    const [apiKey, setApiKey]   = React.useState('');
    const [loading, setLoading] = React.useState(false);
    const [error, setError]     = React.useState('');

    function toggleTool(t: string) {
        setTools(ts => ts.includes(t) ? ts.filter(x => x !== t) : [...ts, t]);
    }

    async function submit() {
        if (!goal.trim()) { setError('Goal is required'); return; }
        setLoading(true); setError('');
        try {
            const { id } = await startOrchestration(goal.trim(), model, tools, apiKey || undefined);
            onSubmit(id);
        } catch (err: unknown) {
            setError(err instanceof Error ? err.message : 'Submit failed');
        } finally {
            setLoading(false);
        }
    }

    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div style={{ fontSize: 11, color: '#555', fontStyle: 'italic', paddingBottom: 6, borderBottom: '1px solid #1a1a1a' }}>
                PM agent decomposes your goal → parallel sub-agents execute tasks → reviewer validates → synthesizer delivers
            </div>

            {error && <div style={{ padding: '6px 10px', background: '#2a1a1a', borderRadius: 4, fontSize: 11, color: '#d06060' }}>{error}</div>}

            <div>
                <label style={{ fontSize: 11, color: '#888' }}>Goal</label>
                <textarea
                    value={goal}
                    onChange={e => setGoal(e.target.value)}
                    placeholder="Describe what you want the multi-agent system to accomplish…"
                    rows={4}
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '6px 8px', borderRadius: 4, fontSize: 12, resize: 'vertical', fontFamily: 'inherit', boxSizing: 'border-box' }}
                />
            </div>

            <div>
                <label style={{ fontSize: 11, color: '#888' }}>Orchestrator Model</label>
                <select value={model} onChange={e => setModel(e.target.value)}
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 13 }}>
                    {MODELS.map(m => <option key={m} value={m}>{m}</option>)}
                </select>
            </div>

            <div>
                <label style={{ fontSize: 11, color: '#888' }}>Default Tools for Sub-Agents</label>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 6 }}>
                    {ALL_TOOLS.map(t => (
                        <button key={t} onClick={() => toggleTool(t)} style={{
                            padding: '3px 9px', fontSize: 11, borderRadius: 3, cursor: 'pointer',
                            background: tools.includes(t) ? '#1a2a3a' : '#111',
                            border: tools.includes(t) ? '1px solid #2a4a6a' : '1px solid #2a2a2a',
                            color: tools.includes(t) ? '#7ab4ff' : '#555',
                        }}>{t}</button>
                    ))}
                </div>
            </div>

            <div>
                <label style={{ fontSize: 11, color: '#888' }}>API Key <span style={{ color: '#444' }}>(optional — uses server env if omitted)</span></label>
                <input type="password" value={apiKey} onChange={e => setApiKey(e.target.value)}
                    placeholder="sk-ant-… / sk-… (not stored)"
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 12, boxSizing: 'border-box' }}
                />
            </div>

            <button onClick={submit} disabled={loading}
                style={{ padding: '8px 20px', background: loading ? '#111' : '#1a2a1a', border: '1px solid #2a4a2a', color: '#60d060', borderRadius: 4, cursor: loading ? 'default' : 'pointer', fontSize: 13, fontWeight: 700, alignSelf: 'flex-start' }}>
                {loading ? '⟳ Starting…' : '▶ Orchestrate'}
            </button>
        </div>
    );
}

function GraphTab({ runId }: { runId: string | null }) {
    const [run, setRun] = React.useState<OrchestrationRun | null>(null);
    const [err, setErr] = React.useState('');

    React.useEffect(() => {
        if (!runId) return;
        let cancelled = false;

        async function poll() {
            try {
                const r = await getOrchestrationRun(runId!);
                if (!cancelled) {
                    setRun(r);
                    if (r.status === 'running') setTimeout(poll, 2000);
                }
            } catch { if (!cancelled) setErr('Could not load run'); }
        }
        poll();
        return () => { cancelled = true; };
    }, [runId]);

    if (!runId) return <div style={{ padding: 20, color: '#444', fontSize: 12 }}>Submit a goal in the Compose tab to start orchestration.</div>;
    if (err) return <div style={{ padding: 20, color: '#d06060', fontSize: 12 }}>{err}</div>;
    if (!run) return <div style={{ padding: 20, color: '#555', fontSize: 12 }}>Loading…</div>;

    return (
        <div style={{ padding: 12 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 12 }}>
                <span style={{ fontSize: 12, color: '#ccc', fontWeight: 700, flex: 1 }}>{run.goal.slice(0, 80)}{run.goal.length > 80 ? '…' : ''}</span>
                <span style={{
                    fontSize: 11, fontWeight: 700, border: '1px solid',
                    borderRadius: 3, padding: '2px 6px',
                    color: run.status === 'completed' ? '#60d060' : run.status === 'failed' ? '#ff6060' : '#7ab4ff',
                    borderColor: run.status === 'completed' ? '#2a4a2a' : run.status === 'failed' ? '#4a1a1a' : '#1a2a4a',
                }}>{run.status === 'running' ? `● ${run.node}…` : run.status}</span>
            </div>
            <GraphView run={run} />
        </div>
    );
}

function HistoryTab({ onSelect }: { onSelect: (id: string) => void }) {
    const [runs, setRuns] = React.useState<OrchestrationRun[]>([]);

    React.useEffect(() => {
        listOrchestrationRuns().then(setRuns).catch(() => setRuns([]));
        const iv = setInterval(() => listOrchestrationRuns().then(setRuns).catch(() => {}), 5000);
        return () => clearInterval(iv);
    }, []);

    if (runs.length === 0) return <div style={{ padding: 20, color: '#444', fontSize: 12 }}>No orchestration runs yet.</div>;

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 6 }}>
            {runs.map(r => (
                <div key={r.id} onClick={() => onSelect(r.id)}
                    style={{ padding: '8px 10px', background: '#0d0d0d', border: '1px solid #1e1e1e', borderRadius: 4, cursor: 'pointer' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                        <span style={{ fontSize: 12, color: '#ccc', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.goal.slice(0, 80)}</span>
                        <span style={{ fontSize: 10, color: r.status === 'completed' ? '#60d060' : r.status === 'failed' ? '#ff6060' : '#7ab4ff', fontWeight: 700 }}>{r.status}</span>
                    </div>
                    <div style={{ fontSize: 10, color: '#444', fontFamily: 'monospace' }}>
                        {r.tasks.length} tasks · node: {r.node} · {r.startedAt.slice(0, 19).replace('T', ' ')}
                    </div>
                    {r.result && <div style={{ fontSize: 11, color: '#666', marginTop: 4, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.result.slice(0, 100)}</div>}
                </div>
            ))}
        </div>
    );
}

const TABS: { id: OrchestrateTab; label: string }[] = [
    { id: 'compose', label: 'Compose' },
    { id: 'graph',   label: 'Graph'   },
    { id: 'history', label: 'History' },
];

function OrchestrateView() {
    const [tab, setTab]       = React.useState<OrchestrateTab>('compose');
    const [activeId, setActiveId] = React.useState<string | null>(null);

    function handleSubmit(id: string) {
        setActiveId(id);
        setTab('graph');
    }

    function handleSelectHistory(id: string) {
        setActiveId(id);
        setTab('graph');
    }

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0d0d0d', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #1e1e1e', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontWeight: 700, fontSize: 13, color: '#60d0a0' }}>⊞ Orchestrate</span>
                <span style={{ fontSize: 10, color: '#333' }}>PM → Execute → Review → Deliver</span>
            </div>
            <div style={{ display: 'flex', borderBottom: '1px solid #1a1a1a', background: '#0a0a0a' }}>
                {TABS.map(t => (
                    <button key={t.id} onClick={() => setTab(t.id)} style={{
                        padding: '6px 16px', border: 'none', background: 'transparent',
                        color: tab === t.id ? '#60d0a0' : '#555',
                        borderBottom: tab === t.id ? '2px solid #60d0a0' : '2px solid transparent',
                        cursor: 'pointer', fontSize: 12, fontWeight: tab === t.id ? 700 : 400,
                    }}>{t.label}</button>
                ))}
                {activeId && (
                    <span style={{ marginLeft: 'auto', padding: '6px 12px', fontSize: 10, color: '#444', fontFamily: 'monospace', alignSelf: 'center' }}>
                        {activeId.slice(0, 8)}…
                    </span>
                )}
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {tab === 'compose' && <ComposeTab onSubmit={handleSubmit} />}
                {tab === 'graph'   && <GraphTab runId={activeId} />}
                {tab === 'history' && <HistoryTab onSelect={handleSelectHistory} />}
            </div>
        </div>
    );
}

@injectable()
export class OrchestratePanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:orchestrate';
    static readonly LABEL = 'Orchestrate';

    @postConstruct()
    protected init(): void {
        this.id = OrchestratePanelWidget.ID;
        this.title.label = OrchestratePanelWidget.LABEL;
        this.title.caption = 'Multi-agent graph orchestrator';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-type-hierarchy-sub';
        this.update();
    }

    protected render(): React.ReactNode { return <OrchestrateView />; }
}

@injectable()
export class OrchestratePanelContribution extends AbstractViewContribution<OrchestratePanelWidget> {
    constructor() {
        super({ widgetId: OrchestratePanelWidget.ID, widgetName: OrchestratePanelWidget.LABEL, defaultWidgetOptions: { area: 'main' }, toggleCommandId: OrchestratePanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(OrchestratePanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
