import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { TraceStepType } from '@agennext/agent-ide-types';
import { ReplayPanelCommand } from '../agent-ide-commands';
import { listRuns, getRun, RunSummary } from '../runtime/backend-client';

interface ReplayStep {
    sequence:    number;
    type:        TraceStepType;
    content:     string;
    toolName?:   string;
    toolInput?:  Record<string, unknown>;
    toolOutput?: string;
    durationMs:  number;
    inputTokens: number;
    outputTokens: number;
    loopIndex?:  number;
    timestamp:   string;
}

interface ReplayRun {
    id:         string;
    agentName:  string;
    taskTitle:  string;
    model:      string;
    startedAt:  string;
    durationMs: number;
    status:     string;
    steps:      ReplayStep[];
}

interface BackendRunRecord {
    runId:        string;
    agentName:    string;
    task:         string;
    model:        string;
    startedAt:    string;
    completedAt?: string;
    status:       string;
    steps: Array<{
        sequence: number; type: TraceStepType; content: string;
        toolName?: string; toolInput?: Record<string, unknown>; toolOutput?: unknown;
        durationMs: number; inputTokens: number; outputTokens: number;
        loopIndex?: number; timestamp: string;
    }>;
}

const STEP_META: Record<TraceStepType, { bg: string; fg: string; icon: string; label: string }> = {
    thought:     { bg: '#1e2a4a', fg: '#7ab4ff', icon: '💭', label: 'Thought' },
    action:      { bg: '#1a2e1a', fg: '#60d060', icon: '⚙', label: 'Action' },
    observation: { bg: '#2e2a10', fg: '#d0a030', icon: '👁', label: 'Observation' },
    result:      { bg: '#2a1a3a', fg: '#c080ff', icon: '✓', label: 'Result' },
    error:       { bg: '#3a1a1a', fg: '#ff6060', icon: '✗', label: 'Error' },
};

function mapBackendRun(raw: BackendRunRecord): ReplayRun {
    const start = new Date(raw.startedAt).getTime();
    const end   = raw.completedAt ? new Date(raw.completedAt).getTime() : start;
    return {
        id:         raw.runId,
        agentName:  raw.agentName,
        taskTitle:  raw.task,
        model:      raw.model,
        startedAt:  raw.startedAt,
        durationMs: end - start,
        status:     raw.status,
        steps: raw.steps.map(s => ({
            ...s,
            toolOutput: s.toolOutput !== undefined
                ? (typeof s.toolOutput === 'string' ? s.toolOutput : JSON.stringify(s.toolOutput, null, 2).slice(0, 1000))
                : undefined,
        })),
    };
}

function StepCard({ step, active, visible }: { step: ReplayStep; active: boolean; visible: boolean }) {
    const m = STEP_META[step.type];
    return (
        <div style={{
            border: `1px solid ${active ? m.fg : '#1e1e1e'}`,
            borderRadius: 6, marginBottom: 6, overflow: 'hidden',
            opacity: visible ? 1 : 0.25,
            transition: 'opacity 0.3s, border-color 0.2s',
            background: active ? m.bg : '#0d0d0d',
        }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px', borderBottom: '1px solid #1a1a1a' }}>
                <span style={{ fontSize: 13 }}>{m.icon}</span>
                <span style={{ fontSize: 11, fontWeight: 700, color: m.fg, width: 80 }}>{m.label}</span>
                <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>#{step.sequence}</span>
                {step.loopIndex && <span style={{ fontSize: 10, color: '#d060d0', fontFamily: 'monospace' }}>loop {step.loopIndex}</span>}
                <span style={{ marginLeft: 'auto', fontSize: 10, color: '#555', fontFamily: 'monospace' }}>{step.timestamp.slice(11, 23)}</span>
                <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>{step.durationMs}ms</span>
            </div>
            <div style={{ padding: '8px 10px', fontSize: 12, color: '#c8c8c8', lineHeight: 1.5 }}>{step.content}</div>
            {step.toolName && (
                <div style={{ padding: '4px 10px 8px', fontSize: 11, fontFamily: 'monospace' }}>
                    <span style={{ color: '#555' }}>tool: </span>
                    <span style={{ color: '#60d060' }}>{step.toolName}</span>
                    {step.toolOutput && (
                        <div style={{ marginTop: 4, color: '#888', background: '#111', padding: '4px 8px', borderRadius: 3, fontSize: 10, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{step.toolOutput}</div>
                    )}
                </div>
            )}
            <div style={{ display: 'flex', gap: 16, padding: '4px 10px 8px', fontSize: 10, color: '#555', fontFamily: 'monospace' }}>
                <span>in: {step.inputTokens}</span>
                <span>out: {step.outputTokens}</span>
            </div>
        </div>
    );
}

const STATUS_COLOR: Record<string, string> = { completed: '#60d060', running: '#7ab4ff', failed: '#ff6060', cancelled: '#888' };

function ReplayView() {
    const [summaries, setSummaries]   = React.useState<RunSummary[]>([]);
    const [selected,  setSelected]    = React.useState<ReplayRun | null>(null);
    const [loading,   setLoading]     = React.useState(true);
    const [loadingRun, setLoadingRun] = React.useState(false);
    const [error,     setError]       = React.useState<string | null>(null);
    const [cursor,    setCursor]      = React.useState(0);
    const [playing,   setPlaying]     = React.useState(false);
    const timerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);

    // Fetch run list on mount
    React.useEffect(() => {
        listRuns()
            .then(r => { setSummaries(r); setLoading(false); })
            .catch(() => { setError('Backend not reachable — start the backend to view real runs.'); setLoading(false); });
    }, []);

    // Auto-select first run when list loads
    React.useEffect(() => {
        if (summaries.length > 0 && !selected && !loadingRun) loadRun(summaries[0]!.runId);
    }, [summaries]);

    async function loadRun(runId: string) {
        setLoadingRun(true);
        setCursor(0);
        setPlaying(false);
        try {
            const raw = await getRun(runId) as BackendRunRecord;
            setSelected(mapBackendRun(raw));
        } catch {
            setError(`Could not load run ${runId}`);
        } finally {
            setLoadingRun(false);
        }
    }

    const totalSteps = selected?.steps.length ?? 0;

    React.useEffect(() => {
        if (playing) {
            if (cursor >= totalSteps) { setPlaying(false); return; }
            timerRef.current = setTimeout(() => setCursor(c => c + 1), 600);
        }
        return () => { if (timerRef.current) clearTimeout(timerRef.current); };
    }, [playing, cursor, totalSteps]);

    function togglePlay() {
        if (cursor >= totalSteps) { setCursor(0); setPlaying(true); return; }
        setPlaying(p => !p);
    }

    const totalIn  = selected?.steps.slice(0, cursor).reduce((s, x) => s + x.inputTokens, 0) ?? 0;
    const totalOut = selected?.steps.slice(0, cursor).reduce((s, x) => s + x.outputTokens, 0) ?? 0;

    return (
        <div style={{ display: 'flex', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            {/* Run list sidebar */}
            <div style={{ width: 220, borderRight: '1px solid #1e1e1e', display: 'flex', flexDirection: 'column' }}>
                <div style={{ padding: '8px 12px', borderBottom: '1px solid #1e1e1e', fontSize: 11, fontWeight: 700, color: '#555' }}>
                    RUNS {loading ? '…' : `(${summaries.length})`}
                </div>
                {error && <div style={{ padding: '10px 12px', fontSize: 11, color: '#d06060' }}>{error}</div>}
                {!loading && summaries.length === 0 && !error && (
                    <div style={{ padding: '10px 12px', fontSize: 11, color: '#555' }}>No runs yet. Submit a task in the Runs panel.</div>
                )}
                {summaries.map(r => (
                    <div key={r.runId} onClick={() => loadRun(r.runId)}
                        style={{ padding: '8px 12px', cursor: 'pointer', borderBottom: '1px solid #1a1a1a', background: selected?.id === r.runId ? '#111a2a' : 'transparent' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
                            <span style={{ fontSize: 12, color: selected?.id === r.runId ? '#7ab4ff' : '#aaa', fontWeight: selected?.id === r.runId ? 700 : 400, flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.agentName}</span>
                            <span style={{ fontSize: 9, color: STATUS_COLOR[r.status] ?? '#888' }}>{r.status}</span>
                        </div>
                        <div style={{ fontSize: 11, color: '#555', marginTop: 2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.task}</div>
                        <div style={{ fontSize: 10, color: '#444', marginTop: 2 }}>{r.stepCount} steps</div>
                    </div>
                ))}
            </div>

            {/* Replay area */}
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                {!selected ? (
                    <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#444', fontSize: 12 }}>
                        {loadingRun ? 'Loading run…' : 'Select a run from the sidebar.'}
                    </div>
                ) : (
                    <>
                        <div style={{ padding: '8px 14px', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                            <div style={{ fontSize: 12, fontWeight: 700, color: '#ccc' }}>{selected.agentName} — {selected.taskTitle}</div>
                            <div style={{ fontSize: 11, color: '#555', marginTop: 2 }}>Model: {selected.model} · {selected.steps.length} steps · {selected.durationMs}ms · <span style={{ color: STATUS_COLOR[selected.status] ?? '#888' }}>{selected.status}</span></div>
                        </div>

                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 14px', borderBottom: '1px solid #1a1a1a', background: '#0d0d0d' }}>
                            <button onClick={() => { setCursor(0); setPlaying(false); }} style={btnStyle}>⏮</button>
                            <button onClick={() => { if (cursor > 0) { setCursor(c => c - 1); setPlaying(false); } }} style={btnStyle}>◀</button>
                            <button onClick={togglePlay} style={{ ...btnStyle, background: playing ? '#1a3a1a' : '#1a2a3a', color: playing ? '#60d060' : '#7ab4ff', minWidth: 64 }}>
                                {playing ? '⏸ Pause' : cursor >= totalSteps ? '↺ Restart' : '▶ Play'}
                            </button>
                            <button onClick={() => { if (cursor < totalSteps) { setCursor(c => c + 1); setPlaying(false); } }} style={btnStyle}>▶</button>
                            <button onClick={() => { setCursor(totalSteps); setPlaying(false); }} style={btnStyle}>⏭</button>
                            <div style={{ marginLeft: 8, flex: 1 }}>
                                <input type="range" min={0} max={totalSteps} value={cursor}
                                    onChange={e => { setCursor(Number(e.target.value)); setPlaying(false); }}
                                    style={{ width: '100%', accentColor: '#7ab4ff' }}
                                />
                            </div>
                            <span style={{ fontSize: 11, color: '#7ab4ff', fontFamily: 'monospace', minWidth: 56 }}>{cursor}/{totalSteps}</span>
                            <div style={{ fontSize: 10, color: '#555', fontFamily: 'monospace', textAlign: 'right', lineHeight: 1.4 }}>
                                <div>in: {totalIn.toLocaleString()}</div>
                                <div>out: {totalOut.toLocaleString()}</div>
                            </div>
                        </div>

                        <div style={{ flex: 1, overflow: 'auto', padding: '10px 14px' }}>
                            {selected.steps.map((step, i) => (
                                <StepCard key={step.sequence} step={step} active={i === cursor - 1} visible={i < cursor} />
                            ))}
                            {cursor === 0 && <div style={{ color: '#444', fontSize: 12, textAlign: 'center', marginTop: 40 }}>Press ▶ Play or ▶ to step through the trace.</div>}
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}

const btnStyle: React.CSSProperties = {
    padding: '5px 10px', background: '#111', border: '1px solid #2a2a2a',
    color: '#aaa', borderRadius: 4, cursor: 'pointer', fontSize: 13,
};

@injectable()
export class ReplayPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:replay';
    static readonly LABEL = 'Replay';

    @postConstruct()
    protected init(): void {
        this.id = ReplayPanelWidget.ID;
        this.title.label = ReplayPanelWidget.LABEL;
        this.title.caption = 'Step-through agent trace replay';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-debug-rerun';
        this.update();
    }

    protected render(): React.ReactNode { return <ReplayView />; }
}

@injectable()
export class ReplayPanelContribution extends AbstractViewContribution<ReplayPanelWidget> {
    constructor() {
        super({ widgetId: ReplayPanelWidget.ID, widgetName: ReplayPanelWidget.LABEL, defaultWidgetOptions: { area: 'bottom' }, toggleCommandId: ReplayPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(ReplayPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
