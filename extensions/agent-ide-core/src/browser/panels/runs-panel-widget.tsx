import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { AgentRun, TaskStatus, TraceStepType, TokenRecord, TokenFlowRecord } from '@agennext/agent-ide-types';
import { RunsPanelCommand } from '../agent-ide-commands';

type RunTab = 'trace' | 'tokenflow' | 'performance';

// ─── Dummy data helpers ───────────────────────────────────────────────────────

function approxTokens(text: string): number {
    return Math.ceil(text.length / 3.8);
}

function uid(): string {
    return Math.random().toString(36).slice(2, 10);
}

const STEP_META: Record<TraceStepType, { bg: string; fg: string }> = {
    thought:     { bg: '#1e2a4a', fg: '#7ab4ff' },
    action:      { bg: '#1a2e1a', fg: '#60d060' },
    observation: { bg: '#2e2a10', fg: '#d0a030' },
    result:      { bg: '#2a1a3a', fg: '#c080ff' },
    error:       { bg: '#3a1a1a', fg: '#ff6060' },
};

interface SimStep {
    sequence: number;
    type: TraceStepType;
    content: string;
    toolName?: string;
    toolInput?: Record<string, unknown>;
    durationMs: number;
    loopIndex?: number;
}

function makeSimTrace(runId: string): SimStep[] {
    return [
        { sequence: 1, type: 'thought',     content: 'Parsing task requirements and building execution plan.', durationMs: 180 },
        { sequence: 2, type: 'action',      content: 'Invoking browser tool to gather context.', toolName: 'browser', toolInput: { url: 'https://docs.example.com/api' }, durationMs: 820 },
        { sequence: 3, type: 'observation', content: 'Retrieved 4 relevant documentation sections. Processing…', durationMs: 310 },
        { sequence: 4, type: 'thought',     content: 'First pass yields incomplete result. Entering refinement loop.', durationMs: 140, loopIndex: 1 },
        { sequence: 5, type: 'action',      content: 'Re-querying with refined parameters.', toolName: 'http_client', toolInput: { method: 'GET', endpoint: '/api/v2/context' }, durationMs: 650, loopIndex: 1 },
        { sequence: 6, type: 'result',      content: 'Task completed. Artifact written to workspace.', durationMs: 95 },
    ];
}

function makeTokenFlow(agentId: string, taskId: string): TokenFlowRecord {
    const steps = makeSimTrace('dummy');
    const calls: TokenRecord[] = steps.map((s, i) => ({
        callIndex: i,
        stepSequence: s.sequence,
        stepType: s.type,
        model: 'claude-sonnet-4-6',
        inputTokens: approxTokens(s.content) + 80 + (i === 0 ? 320 : 40),
        outputTokens: approxTokens(s.content) + 20,
        cachedTokens: i > 0 ? Math.floor(approxTokens(s.content) * 0.4) : 0,
        durationMs: s.durationMs,
        loopIndex: s.loopIndex,
    }));
    const totalIn  = calls.reduce((s, c) => s + c.inputTokens,  0);
    const totalOut = calls.reduce((s, c) => s + c.outputTokens, 0);
    const totalCached = calls.reduce((s, c) => s + c.cachedTokens, 0);
    const now = new Date().toISOString();
    return {
        flowId: uid(),
        agentId, taskId,
        model: 'claude-sonnet-4-6',
        calls,
        totalInputTokens: totalIn,
        totalOutputTokens: totalOut,
        totalCachedTokens: totalCached,
        loopCount: calls.filter(c => c.loopIndex !== undefined && c.loopIndex > 0).length,
        totalCalls: calls.length,
        startedAt: now,
        completedAt: now,
        durationMs: calls.reduce((s, c) => s + c.durationMs, 0),
    };
}

function makeRun(agentName: string): AgentRun {
    const id = uid();
    return {
        id, agentId: uid(), taskId: uid(),
        status: 'completed' as TaskStatus,
        startedAt: new Date(Date.now() - Math.random() * 3600000).toISOString(),
        completedAt: new Date().toISOString(),
        trace: [], artifactIds: [], metadata: {},
    };
}

const INITIAL_RUNS = [makeRun('ResearchAgent'), makeRun('CoderAgent'), makeRun('AnalystAgent')];

// ─── Performance metric generators ───────────────────────────────────────────
// Metrics accurately represent the measurement frameworks from each source.
// Values are plausible dummy figures within realistic ranges for the framework.

function makePlatformMetrics() {
    const avgRtt = 120 + Math.random() * 80;
    return {
        avgRttMs:             parseFloat(avgRtt.toFixed(1)),
        minRttMs:             parseFloat((avgRtt * 0.4).toFixed(1)),
        maxRttMs:             parseFloat((avgRtt * 3.2).toFixed(1)),
        rttStdDevMs:          parseFloat((avgRtt * 0.25).toFixed(1)),
        throughputTokensPerSec: parseFloat((1800 + Math.random() * 800).toFixed(0)),
        messageOverheadPct:   parseFloat((4 + Math.random() * 6).toFixed(1)),
        stabilityScore:       parseFloat((0.82 + Math.random() * 0.15).toFixed(3)),
        connectionCostMs:     parseFloat((avgRtt * 1.4).toFixed(1)),
        securityScore:        parseFloat((0.88 + Math.random() * 0.1).toFixed(3)),
    };
}

function makePlanningMetrics() {
    return {
        taskCompletionRate:                parseFloat((0.78 + Math.random() * 0.18).toFixed(3)),
        onTimeRate:                        parseFloat((0.70 + Math.random() * 0.20).toFixed(3)),
        makespanMs:                        parseFloat((4200 + Math.random() * 3000).toFixed(0)),
        disruptionRecoveryRate:            parseFloat((0.65 + Math.random() * 0.25).toFixed(3)),
        constraintSatisfactionRate:        parseFloat((0.80 + Math.random() * 0.15).toFixed(3)),
        planningEfficiency:                parseFloat((0.72 + Math.random() * 0.20).toFixed(3)),
        interAgentDependencyResolutionRate: parseFloat((0.74 + Math.random() * 0.20).toFixed(3)),
    };
}

function makeQualityMetrics() {
    return {
        responseAccuracyRate:    parseFloat((0.82 + Math.random() * 0.14).toFixed(3)),
        toolExecutionSuccessRate: parseFloat((0.88 + Math.random() * 0.10).toFixed(3)),
        avgReasoningDepth:       parseFloat((3.2 + Math.random() * 2.4).toFixed(1)),
        coordinationOverheadMs:  parseFloat((180 + Math.random() * 120).toFixed(0)),
        hallucinationRate:       parseFloat((0.04 + Math.random() * 0.06).toFixed(3)),
    };
}

// AgentBench scores per environment — Liu et al. 2023
// OS: shell/file tasks, DB: database ops, KG: knowledge graph SPARQL,
// HH: household planning, WS: web shopping, ALF: AlfWorld nav,
// WB: WebArena browsing, LTP: lateral thinking puzzles
function makeAgentBenchScores() {
    const scores = {
        os:  parseFloat((0.40 + Math.random() * 0.40).toFixed(3)),
        db:  parseFloat((0.35 + Math.random() * 0.40).toFixed(3)),
        kg:  parseFloat((0.30 + Math.random() * 0.35).toFixed(3)),
        hh:  parseFloat((0.50 + Math.random() * 0.35).toFixed(3)),
        ws:  parseFloat((0.45 + Math.random() * 0.35).toFixed(3)),
        alf: parseFloat((0.25 + Math.random() * 0.40).toFixed(3)),
        wb:  parseFloat((0.20 + Math.random() * 0.35).toFixed(3)),
        ltp: parseFloat((0.30 + Math.random() * 0.45).toFixed(3)),
        overall: 0,
    };
    scores.overall = parseFloat(((scores.os + scores.db + scores.kg + scores.hh + scores.ws + scores.alf + scores.wb + scores.ltp) / 8).toFixed(3));
    return scores;
}

// ─── Sub-components ───────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: TaskStatus }) {
    const c = { completed: '#40a040', failed: '#a04040', in_progress: '#4080c0', pending: '#808040', cancelled: '#606060' }[status] ?? '#606060';
    return <span style={{ width: 8, height: 8, borderRadius: '50%', background: c, display: 'inline-block', flexShrink: 0 }} />;
}

function RunList({ runs, selected, onSelect }: { runs: AgentRun[]; selected: string | null; onSelect: (id: string) => void }) {
    return (
        <div style={{ flex: '0 0 180px', borderRight: '1px solid #1e1e1e', overflow: 'auto' }}>
            {runs.map(r => (
                <div key={r.id} onClick={() => onSelect(r.id)}
                    style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px', cursor: 'pointer', borderBottom: '1px solid #1a1a1a', background: selected === r.id ? '#1a2a3a' : 'transparent' }}>
                    <StatusBadge status={r.status} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ fontSize: 11, fontWeight: 600, color: '#ccc', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>run-{r.id.slice(0, 6)}</div>
                        <div style={{ fontSize: 10, color: '#555' }}>{new Date(r.startedAt).toLocaleTimeString()}</div>
                    </div>
                </div>
            ))}
        </div>
    );
}

function TraceStepRow({ step, visible }: { step: SimStep; visible: boolean }) {
    const c = STEP_META[step.type];
    if (!visible) return null;
    return (
        <div style={{ background: c.bg, borderRadius: 4, padding: '7px 10px', border: `1px solid ${c.fg}22` }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                <span style={{ background: `${c.fg}22`, color: c.fg, padding: '1px 6px', borderRadius: 2, fontSize: 10, fontWeight: 700 }}>{step.type.toUpperCase()}</span>
                {step.loopIndex !== undefined && step.loopIndex > 0 && <span style={{ background: '#3a1a5a', color: '#c080ff', padding: '1px 5px', borderRadius: 2, fontSize: 10 }}>loop {step.loopIndex}</span>}
                {step.toolName && <span style={{ color: '#70a0d0', fontSize: 10 }}>{step.toolName}</span>}
                <span style={{ marginLeft: 'auto', color: '#555', fontSize: 10 }}>{step.durationMs}ms</span>
            </div>
            <div style={{ fontSize: 12, color: c.fg, lineHeight: 1.4 }}>{step.content}</div>
            {step.toolInput && <pre style={{ margin: '6px 0 0', fontSize: 10, color: '#888', background: '#0a0a0a', padding: '4px 6px', borderRadius: 2, overflow: 'auto' }}>{JSON.stringify(step.toolInput, null, 2)}</pre>}
        </div>
    );
}

function TraceViewer({ runId }: { runId: string }) {
    const steps = React.useMemo(() => makeSimTrace(runId), [runId]);
    const [visible, setVisible] = React.useState(0);
    React.useEffect(() => {
        setVisible(0);
        const iv = setInterval(() => setVisible(v => { if (v >= steps.length) { clearInterval(iv); return v; } return v + 1; }), 220);
        return () => clearInterval(iv);
    }, [runId]);
    return (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, padding: 12 }}>
            {steps.map((s, i) => <TraceStepRow key={s.sequence} step={s} visible={i < visible} />)}
        </div>
    );
}

function TokenFlowViewer({ runId }: { runId: string }) {
    const flow = React.useMemo(() => makeTokenFlow(runId, 'task-' + runId), [runId]);
    const maxTokens = Math.max(...flow.calls.map(c => c.inputTokens + c.outputTokens));
    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ display: 'flex', gap: 16, fontSize: 11, color: '#888', paddingBottom: 8, borderBottom: '1px solid #1e1e1e' }}>
                <span>Total in: <b style={{ color: '#60b0ff' }}>{flow.totalInputTokens.toLocaleString()}</b></span>
                <span>Total out: <b style={{ color: '#60d060' }}>{flow.totalOutputTokens.toLocaleString()}</b></span>
                <span>Cached: <b style={{ color: '#d0a030' }}>{flow.totalCachedTokens.toLocaleString()}</b></span>
                <span>Calls: <b style={{ color: '#c0c0c0' }}>{flow.totalCalls}</b></span>
                <span>Loops: <b style={{ color: '#c080ff' }}>{flow.loopCount}</b></span>
            </div>
            {flow.calls.map(c => {
                const total = c.inputTokens + c.outputTokens;
                const inPct  = (c.inputTokens  / total) * 100;
                const outPct = (c.outputTokens / total) * 100;
                const barW   = Math.max(20, (total / maxTokens) * 100);
                const mc = STEP_META[c.stepType];
                return (
                    <div key={c.callIndex} style={{ background: '#141414', borderRadius: 4, padding: '8px 10px', border: '1px solid #1e1e1e' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 5 }}>
                            <span style={{ color: mc.fg, background: mc.bg, padding: '1px 6px', borderRadius: 2, fontSize: 10, fontWeight: 700 }}>{c.stepType.toUpperCase()}</span>
                            <span style={{ fontSize: 10, color: '#666' }}>call #{c.callIndex} · step {c.stepSequence}</span>
                            {c.loopIndex !== undefined && c.loopIndex > 0 && <span style={{ color: '#c080ff', fontSize: 10 }}>loop {c.loopIndex}</span>}
                            <span style={{ marginLeft: 'auto', fontSize: 10, color: '#555' }}>{c.durationMs}ms</span>
                        </div>
                        <div style={{ height: 14, background: '#0a0a0a', borderRadius: 2, overflow: 'hidden', position: 'relative' }}>
                            <div style={{ position: 'absolute', left: 0, top: 0, height: '100%', width: `${(inPct / 100) * barW}%`, background: '#2a4a7a' }} />
                            <div style={{ position: 'absolute', left: `${(inPct / 100) * barW}%`, top: 0, height: '100%', width: `${(outPct / 100) * barW}%`, background: '#2a5a2a' }} />
                        </div>
                        <div style={{ display: 'flex', gap: 10, fontSize: 10, color: '#666', marginTop: 3 }}>
                            <span style={{ color: '#4a80c0' }}>in {c.inputTokens}</span>
                            <span style={{ color: '#4a8a4a' }}>out {c.outputTokens}</span>
                            {c.cachedTokens > 0 && <span style={{ color: '#a07020' }}>cached {c.cachedTokens}</span>}
                        </div>
                    </div>
                );
            })}
        </div>
    );
}

// ─── Performance panel ────────────────────────────────────────────────────────

function MetricRow({ label, value, unit, color }: { label: string; value: number | string; unit?: string; color?: string }) {
    return (
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '4px 0', borderBottom: '1px solid #1a1a1a' }}>
            <span style={{ fontSize: 11, color: '#888' }}>{label}</span>
            <span style={{ fontSize: 12, fontWeight: 600, color: color ?? '#c0d0e0', fontFamily: 'monospace' }}>
                {typeof value === 'number' && value < 10 ? value.toFixed(3) : value}{unit && <span style={{ color: '#555', fontSize: 10 }}> {unit}</span>}
            </span>
        </div>
    );
}

function MetricGroup({ title, source, color, children }: { title: string; source: string; color: string; children: React.ReactNode }) {
    return (
        <div style={{ marginBottom: 16 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6 }}>
                <span style={{ fontSize: 12, fontWeight: 700, color }}>{title}</span>
                <span style={{ fontSize: 9, color: '#444', fontStyle: 'italic' }}>{source}</span>
            </div>
            <div style={{ background: '#141414', borderRadius: 4, padding: '4px 10px', border: `1px solid ${color}22` }}>{children}</div>
        </div>
    );
}

function AgentBenchBar({ env, score }: { env: string; score: number }) {
    const pct = Math.round(score * 100);
    const color = pct >= 60 ? '#60d060' : pct >= 40 ? '#d0a030' : '#d06060';
    return (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '3px 0' }}>
            <span style={{ width: 32, fontSize: 10, color: '#888', fontFamily: 'monospace', textTransform: 'uppercase' }}>{env}</span>
            <div style={{ flex: 1, height: 10, background: '#0a0a0a', borderRadius: 2 }}>
                <div style={{ width: `${pct}%`, height: '100%', background: color, borderRadius: 2, transition: 'width 0.5s' }} />
            </div>
            <span style={{ width: 36, fontSize: 10, color, fontFamily: 'monospace', textAlign: 'right' }}>{pct}%</span>
        </div>
    );
}

function PerformanceViewer({ runId }: { runId: string }) {
    const [running, setRunning] = React.useState(false);
    const [result, setResult] = React.useState<ReturnType<typeof makeAllMetrics> | null>(null);

    function makeAllMetrics() {
        return {
            platform:   makePlatformMetrics(),
            planning:   makePlanningMetrics(),
            quality:    makeQualityMetrics(),
            agentBench: makeAgentBenchScores(),
        };
    }

    const run = () => {
        setRunning(true);
        setResult(null);
        setTimeout(() => { setResult(makeAllMetrics()); setRunning(false); }, 600);
    };

    const m = result;
    return (
        <div style={{ padding: 12 }}>
            <div style={{ marginBottom: 14, display: 'flex', gap: 8, alignItems: 'center' }}>
                <button onClick={run} disabled={running} style={{ padding: '5px 14px', background: running ? '#1a1a1a' : '#1e2a3e', border: '1px solid #2a3a5a', color: running ? '#444' : '#60b0ff', borderRadius: 4, cursor: running ? 'not-allowed' : 'pointer', fontSize: 12, fontWeight: 600 }}>
                    {running ? 'Running…' : '▶ Run Evaluation'}
                </button>
                {!m && !running && <span style={{ fontSize: 11, color: '#444' }}>No results yet. Run evaluation to generate metrics.</span>}
            </div>

            {m && (
                <div>
                    <MetricGroup title="Platform" source="Jurasovic 2006 · Król 2008" color="#60b0ff">
                        <MetricRow label="Avg RTT"               value={m.platform.avgRttMs}             unit="ms" />
                        <MetricRow label="RTT StdDev"            value={m.platform.rttStdDevMs}          unit="ms" />
                        <MetricRow label="Throughput"            value={m.platform.throughputTokensPerSec} unit="tok/s" color="#80d080" />
                        <MetricRow label="Msg Overhead"          value={m.platform.messageOverheadPct}   unit="%" />
                        <MetricRow label="Stability istabt(h)"  value={m.platform.stabilityScore}       color="#a0c0e0" />
                        <MetricRow label="Connect cost Lt(h)"   value={m.platform.connectionCostMs}     unit="ms" />
                        <MetricRow label="Security isect(h)"    value={m.platform.securityScore}        color="#a0c0e0" />
                    </MetricGroup>

                    <MetricGroup title="Planning" source="REALM-Bench · Geng & Chang 2025" color="#d0a030">
                        <MetricRow label="Task Completion"           value={m.planning.taskCompletionRate}                color="#d0c060" />
                        <MetricRow label="On-Time Rate"              value={m.planning.onTimeRate}                        color="#d0c060" />
                        <MetricRow label="Makespan"                  value={m.planning.makespanMs}                        unit="ms" />
                        <MetricRow label="Disruption Recovery"       value={m.planning.disruptionRecoveryRate}            color="#d0c060" />
                        <MetricRow label="Constraint Satisfaction"   value={m.planning.constraintSatisfactionRate}       color="#d0c060" />
                        <MetricRow label="Planning Efficiency"       value={m.planning.planningEfficiency}               color="#d0c060" />
                        <MetricRow label="Dep. Resolution"           value={m.planning.interAgentDependencyResolutionRate} color="#d0c060" />
                    </MetricGroup>

                    <MetricGroup title="LLM Quality" source="Härer 2025" color="#c080ff">
                        <MetricRow label="Response Accuracy"        value={m.quality.responseAccuracyRate}    color="#c0a0ff" />
                        <MetricRow label="Tool Exec Success"        value={m.quality.toolExecutionSuccessRate} color="#c0a0ff" />
                        <MetricRow label="Avg Reasoning Depth"      value={m.quality.avgReasoningDepth}       color="#c0a0ff" />
                        <MetricRow label="Coordination Overhead"    value={m.quality.coordinationOverheadMs}  unit="ms" />
                        <MetricRow label="Hallucination Rate"       value={m.quality.hallucinationRate}       color="#ff8080" />
                    </MetricGroup>

                    <MetricGroup title="AgentBench" source="Liu et al. 2023" color="#60d0a0">
                        <div style={{ paddingTop: 4 }}>
                            {(['os','db','kg','hh','ws','alf','wb','ltp'] as const).map(env => (
                                <AgentBenchBar key={env} env={env} score={(m.agentBench as any)[env]} />
                            ))}
                            <div style={{ borderTop: '1px solid #1e1e1e', marginTop: 6, paddingTop: 6 }}>
                                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                                    <span style={{ fontSize: 11, color: '#888' }}>Overall</span>
                                    <span style={{ fontSize: 13, fontWeight: 700, color: '#60d0a0', fontFamily: 'monospace' }}>{(m.agentBench.overall * 100).toFixed(1)}%</span>
                                </div>
                            </div>
                        </div>
                    </MetricGroup>
                </div>
            )}
        </div>
    );
}

// ─── Main view ────────────────────────────────────────────────────────────────

function RunsView() {
    const [runs, setRuns] = React.useState<AgentRun[]>(INITIAL_RUNS);
    const [selected, setSelected] = React.useState<string | null>(INITIAL_RUNS[0].id);
    const [tab, setTab] = React.useState<RunTab>('trace');
    const [simulating, setSimulating] = React.useState(false);

    const simulate = () => {
        setSimulating(true);
        setTimeout(() => {
            const r = makeRun('SimAgent');
            setRuns(rs => [r, ...rs]);
            setSelected(r.id);
            setTab('trace');
            setSimulating(false);
        }, 500);
    };

    const TABS: { id: RunTab; label: string }[] = [
        { id: 'trace',       label: 'Trace' },
        { id: 'tokenflow',   label: 'Token Flow' },
        { id: 'performance', label: 'Performance' },
    ];

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 10 }}>
                <span style={{ fontWeight: 700, fontSize: 13 }}>Agent Runs</span>
                <button onClick={simulate} disabled={simulating} style={{ marginLeft: 'auto', padding: '4px 12px', background: simulating ? '#1a1a1a' : '#1e2e1e', border: '1px solid #3a5a3a', color: simulating ? '#444' : '#60d060', borderRadius: 4, cursor: simulating ? 'not-allowed' : 'pointer', fontSize: 11, fontWeight: 600 }}>
                    {simulating ? 'Simulating…' : '▶ Simulate Run'}
                </button>
            </div>
            <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
                <RunList runs={runs} selected={selected} onSelect={id => { setSelected(id); setTab('trace'); }} />
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                    {selected && (
                        <>
                            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                                {TABS.map(t => (
                                    <button key={t.id} onClick={() => setTab(t.id)} style={{
                                        padding: '6px 14px', border: 'none', background: 'transparent',
                                        color: tab === t.id ? '#60b0ff' : '#666',
                                        borderBottom: tab === t.id ? '2px solid #60b0ff' : '2px solid transparent',
                                        cursor: 'pointer', fontSize: 11, fontWeight: tab === t.id ? 700 : 400
                                    }}>{t.label}</button>
                                ))}
                            </div>
                            <div style={{ flex: 1, overflow: 'auto' }}>
                                {tab === 'trace'       && <TraceViewer       runId={selected} />}
                                {tab === 'tokenflow'   && <TokenFlowViewer   runId={selected} />}
                                {tab === 'performance' && <PerformanceViewer runId={selected} />}
                            </div>
                        </>
                    )}
                    {!selected && <div style={{ padding: 24, color: '#444', fontSize: 12 }}>Select a run from the list.</div>}
                </div>
            </div>
        </div>
    );
}

@injectable()
export class RunsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:runs';
    static readonly LABEL = 'Runs';

    @postConstruct()
    protected init(): void {
        this.id = RunsPanelWidget.ID;
        this.title.label = RunsPanelWidget.LABEL;
        this.title.caption = RunsPanelWidget.LABEL;
        this.title.closable = true;
        this.title.iconClass = 'fa fa-play-circle';
        this.update();
    }

    protected render(): React.ReactNode {
        return <RunsView />;
    }
}

@injectable()
export class RunsPanelContribution extends AbstractViewContribution<RunsPanelWidget> {
    constructor() {
        super({ widgetId: RunsPanelWidget.ID, widgetName: RunsPanelWidget.LABEL, defaultWidgetOptions: { area: 'bottom' }, toggleCommandId: RunsPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(RunsPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
