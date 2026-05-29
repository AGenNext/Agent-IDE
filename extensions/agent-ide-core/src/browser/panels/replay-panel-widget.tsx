import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { TraceStepType } from '@agennext/agent-ide-types';
import { ReplayPanelCommand } from '../agent-ide-commands';

interface ReplayStep {
    sequence: number;
    type: TraceStepType;
    content: string;
    toolName?: string;
    toolInput?: Record<string, unknown>;
    toolOutput?: string;
    durationMs: number;
    inputTokens: number;
    outputTokens: number;
    loopIndex?: number;
    timestamp: string;
}

interface ReplayRun {
    id: string;
    agentName: string;
    taskTitle: string;
    model: string;
    startedAt: string;
    durationMs: number;
    steps: ReplayStep[];
}

const STEP_META: Record<TraceStepType, { bg: string; fg: string; icon: string; label: string }> = {
    thought:     { bg: '#1e2a4a', fg: '#7ab4ff', icon: '💭', label: 'Thought' },
    action:      { bg: '#1a2e1a', fg: '#60d060', icon: '⚙', label: 'Action' },
    observation: { bg: '#2e2a10', fg: '#d0a030', icon: '👁', label: 'Observation' },
    result:      { bg: '#2a1a3a', fg: '#c080ff', icon: '✓', label: 'Result' },
    error:       { bg: '#3a1a1a', fg: '#ff6060', icon: '✗', label: 'Error' },
};

function makeTimestamp(base: number, offsetMs: number): string {
    return new Date(base + offsetMs).toISOString().slice(11, 23);
}

function makeReplayRun(id: string, agentName: string, taskTitle: string): ReplayRun {
    const base = Date.now() - 7200000;
    const steps: ReplayStep[] = [
        { sequence: 1, type: 'thought',     content: `Analyzing task: "${taskTitle}". Decomposing into sub-goals and selecting tool strategy.`, durationMs: 180, inputTokens: 420, outputTokens: 68, timestamp: makeTimestamp(base, 0) },
        { sequence: 2, type: 'action',      content: 'Querying knowledge base for relevant context.', toolName: 'vector_search', toolInput: { query: taskTitle, topK: 5 }, toolOutput: 'Found 5 relevant chunks (scores: 0.91, 0.88, 0.84, 0.79, 0.74)', durationMs: 340, inputTokens: 260, outputTokens: 42, timestamp: makeTimestamp(base, 180) },
        { sequence: 3, type: 'observation', content: 'Retrieved 5 knowledge chunks. Highest relevance: 0.91. Context window updated with 2,400 tokens of background material.', durationMs: 120, inputTokens: 310, outputTokens: 88, timestamp: makeTimestamp(base, 520) },
        { sequence: 4, type: 'action',      content: 'Invoking HTTP client to fetch live data.', toolName: 'http_client', toolInput: { method: 'GET', url: 'https://api.example.com/data', headers: { Authorization: 'Bearer sk-…' } }, toolOutput: '{"status":200,"data":{"count":142,"items":[…]}}', durationMs: 820, inputTokens: 190, outputTokens: 35, timestamp: makeTimestamp(base, 640) },
        { sequence: 5, type: 'thought',     content: 'Data retrieved. Identifying gaps — 3 items require deeper analysis. Initiating refinement loop.', durationMs: 140, inputTokens: 380, outputTokens: 72, loopIndex: 1, timestamp: makeTimestamp(base, 1460) },
        { sequence: 6, type: 'action',      content: 'Re-querying with refined parameters for gap items.', toolName: 'http_client', toolInput: { method: 'POST', url: 'https://api.example.com/analyze', body: { itemIds: [12, 47, 98] } }, toolOutput: '{"analyses":[{"id":12,"score":0.88},{"id":47,"score":0.72},{"id":98,"score":0.91}]}', durationMs: 650, inputTokens: 220, outputTokens: 48, loopIndex: 1, timestamp: makeTimestamp(base, 1600) },
        { sequence: 7, type: 'observation', content: 'Refinement complete. All gap items resolved. Aggregate confidence: 0.84.', durationMs: 95, inputTokens: 280, outputTokens: 60, loopIndex: 1, timestamp: makeTimestamp(base, 2250) },
        { sequence: 8, type: 'result',      content: `Task "${taskTitle}" completed successfully. Produced structured artifact. Total: ${(420+260+310+190+380+220+280).toLocaleString()} input tokens, ${(68+42+88+35+72+48+60).toLocaleString()} output tokens.`, durationMs: 210, inputTokens: 520, outputTokens: 195, timestamp: makeTimestamp(base, 2345) },
    ];
    const totalMs = steps.reduce((s, x) => s + x.durationMs, 0);
    return { id, agentName, taskTitle, model: 'claude-sonnet-4-6', startedAt: new Date(base).toISOString(), durationMs: totalMs, steps };
}

const REPLAY_RUNS: ReplayRun[] = [
    makeReplayRun('r001', 'ResearchAgent',  'Market analysis Q2 2025'),
    makeReplayRun('r002', 'CoderAgent',     'Refactor orchestrator module'),
    makeReplayRun('r003', 'AnalystAgent',   'Generate evaluation report'),
];

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
                <span style={{ marginLeft: 'auto', fontSize: 10, color: '#555', fontFamily: 'monospace' }}>{step.timestamp}</span>
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

function ReplayView() {
    const [runIdx, setRunIdx] = React.useState(0);
    const [cursor, setCursor] = React.useState(0);
    const [playing, setPlaying] = React.useState(false);
    const timerRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);

    const run = REPLAY_RUNS[runIdx];
    const totalSteps = run.steps.length;

    React.useEffect(() => {
        if (playing) {
            if (cursor >= totalSteps) { setPlaying(false); return; }
            timerRef.current = setTimeout(() => setCursor(c => c + 1), 600);
        }
        return () => { if (timerRef.current) clearTimeout(timerRef.current); };
    }, [playing, cursor, totalSteps]);

    function selectRun(idx: number) {
        setRunIdx(idx);
        setCursor(0);
        setPlaying(false);
    }

    function togglePlay() {
        if (cursor >= totalSteps) { setCursor(0); setPlaying(true); return; }
        setPlaying(p => !p);
    }

    function stepForward() { if (cursor < totalSteps) { setCursor(c => c + 1); setPlaying(false); } }
    function stepBack()    { if (cursor > 0) { setCursor(c => c - 1); setPlaying(false); } }
    function jumpEnd()     { setCursor(totalSteps); setPlaying(false); }
    function jumpStart()   { setCursor(0); setPlaying(false); }

    const totalIn  = run.steps.slice(0, cursor).reduce((s, x) => s + x.inputTokens, 0);
    const totalOut = run.steps.slice(0, cursor).reduce((s, x) => s + x.outputTokens, 0);

    return (
        <div style={{ display: 'flex', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            {/* Run list sidebar */}
            <div style={{ width: 220, borderRight: '1px solid #1e1e1e', display: 'flex', flexDirection: 'column' }}>
                <div style={{ padding: '8px 12px', borderBottom: '1px solid #1e1e1e', fontSize: 11, fontWeight: 700, color: '#555' }}>RUNS</div>
                {REPLAY_RUNS.map((r, i) => (
                    <div key={r.id} onClick={() => selectRun(i)}
                        style={{ padding: '8px 12px', cursor: 'pointer', borderBottom: '1px solid #1a1a1a', background: runIdx === i ? '#111a2a' : 'transparent' }}>
                        <div style={{ fontSize: 12, color: runIdx === i ? '#7ab4ff' : '#aaa', fontWeight: runIdx === i ? 700 : 400 }}>{r.agentName}</div>
                        <div style={{ fontSize: 11, color: '#555', marginTop: 2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.taskTitle}</div>
                        <div style={{ fontSize: 10, color: '#444', marginTop: 2 }}>{r.steps.length} steps · {r.durationMs}ms</div>
                    </div>
                ))}
            </div>

            {/* Replay area */}
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                {/* Header */}
                <div style={{ padding: '8px 14px', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                    <div style={{ fontSize: 12, fontWeight: 700, color: '#ccc' }}>{run.agentName} — {run.taskTitle}</div>
                    <div style={{ fontSize: 11, color: '#555', marginTop: 2 }}>Model: {run.model} · {run.steps.length} steps · {run.durationMs}ms total</div>
                </div>

                {/* Controls */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 14px', borderBottom: '1px solid #1a1a1a', background: '#0d0d0d' }}>
                    <button onClick={jumpStart} title="Jump to start" style={btnStyle}>⏮</button>
                    <button onClick={stepBack}  title="Step back"     style={btnStyle}>◀</button>
                    <button onClick={togglePlay} style={{ ...btnStyle, background: playing ? '#1a3a1a' : '#1a2a3a', color: playing ? '#60d060' : '#7ab4ff', minWidth: 64 }}>
                        {playing ? '⏸ Pause' : cursor >= totalSteps ? '↺ Restart' : '▶ Play'}
                    </button>
                    <button onClick={stepForward} title="Step forward" style={btnStyle}>▶</button>
                    <button onClick={jumpEnd}     title="Jump to end"  style={btnStyle}>⏭</button>
                    <div style={{ marginLeft: 8, flex: 1 }}>
                        <input type="range" min={0} max={totalSteps} value={cursor}
                            onChange={e => { setCursor(Number(e.target.value)); setPlaying(false); }}
                            style={{ width: '100%', accentColor: '#7ab4ff' }}
                        />
                    </div>
                    <span style={{ fontSize: 11, color: '#7ab4ff', fontFamily: 'monospace', minWidth: 56 }}>
                        {cursor}/{totalSteps}
                    </span>
                    <div style={{ fontSize: 10, color: '#555', fontFamily: 'monospace', textAlign: 'right', lineHeight: 1.4 }}>
                        <div>in: {totalIn.toLocaleString()}</div>
                        <div>out: {totalOut.toLocaleString()}</div>
                    </div>
                </div>

                {/* Steps */}
                <div style={{ flex: 1, overflow: 'auto', padding: '10px 14px' }}>
                    {run.steps.map((step, i) => (
                        <StepCard
                            key={step.sequence}
                            step={step}
                            active={i === cursor - 1}
                            visible={i < cursor}
                        />
                    ))}
                    {cursor === 0 && (
                        <div style={{ color: '#444', fontSize: 12, textAlign: 'center', marginTop: 40 }}>
                            Press ▶ Play or click ▶ to step through the trace.
                        </div>
                    )}
                </div>
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

    protected render(): React.ReactNode {
        return <ReplayView />;
    }
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
