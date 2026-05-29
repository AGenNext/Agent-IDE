import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { RunsPanelCommand } from '../agent-ide-commands';
import { TraceStep, AgentRun } from '@agennext/agent-ide-types';

// ---------------------------------------------------------------------------
// Simulated run data — replace with real agent runtime in Phase 2.
// ---------------------------------------------------------------------------

const SIM_AGENTS = ['Planner', 'CodeWriter', 'Researcher'];

function makeSimTrace(runId: string): TraceStep[] {
    const now = Date.now();
    return [
        { id: `${runId}-1`, runId, sequence: 1, type: 'thought',     content: 'Analysing the task requirements and decomposing into subtasks.', timestamp: new Date(now).toISOString(), durationMs: 120 },
        { id: `${runId}-2`, runId, sequence: 2, type: 'action',      content: 'search_web', toolName: 'web_search', toolInput: { query: 'latest agent frameworks 2025' }, timestamp: new Date(now + 200).toISOString(), durationMs: 800 },
        { id: `${runId}-3`, runId, sequence: 3, type: 'observation',  content: 'Found 12 results. Top result: "LangGraph 0.3 released with improved state management."', timestamp: new Date(now + 1000).toISOString(), durationMs: 50 },
        { id: `${runId}-4`, runId, sequence: 4, type: 'thought',     content: 'Synthesising findings. Will write a summary report as the final artifact.', timestamp: new Date(now + 1100).toISOString(), durationMs: 90 },
        { id: `${runId}-5`, runId, sequence: 5, type: 'action',      content: 'write_file', toolName: 'file_write', toolInput: { path: 'output/summary.md' }, timestamp: new Date(now + 1200).toISOString(), durationMs: 60 },
        { id: `${runId}-6`, runId, sequence: 6, type: 'result',      content: 'Task complete. Artifact written to output/summary.md (1.2 KB).', timestamp: new Date(now + 1300).toISOString(), durationMs: 10 },
    ];
}

function makeRun(agentName: string): AgentRun {
    const id = `run-${Date.now().toString(36)}`;
    return {
        id,
        agentId: agentName.toLowerCase(),
        taskId: `task-${Math.floor(Math.random() * 1000)}`,
        status: 'completed',
        startedAt: new Date().toISOString(),
        completedAt: new Date().toISOString(),
        trace: makeSimTrace(id),
        artifactIds: ['artifact-summary'],
        metadata: {},
    };
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

@injectable()
export class RunsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:runs';
    static readonly LABEL = 'Runs';

    @postConstruct()
    protected init(): void {
        this.id = RunsPanelWidget.ID;
        this.title.label = RunsPanelWidget.LABEL;
        this.title.caption = 'Agent execution run history';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-play-circle';
        this.update();
    }

    protected render(): React.ReactNode {
        return <RunsView />;
    }
}

// ---------------------------------------------------------------------------
// React component — manages run list and simulation state
// ---------------------------------------------------------------------------

const RunsView: React.FC = () => {
    const [runs, setRuns] = React.useState<AgentRun[]>([]);
    const [selected, setSelected] = React.useState<AgentRun | null>(null);
    const [simulating, setSimulating] = React.useState(false);

    const startSimulation = () => {
        if (simulating) return;
        setSimulating(true);
        const agentName = SIM_AGENTS[runs.length % SIM_AGENTS.length];
        const run: AgentRun = { ...makeRun(agentName), status: 'in_progress', completedAt: undefined };
        setRuns(prev => [run, ...prev]);
        setSelected(run);

        // Simulate completion after ~1.5 s
        setTimeout(() => {
            const completed: AgentRun = { ...run, status: 'completed', completedAt: new Date().toISOString() };
            setRuns(prev => prev.map(r => r.id === run.id ? completed : r));
            setSelected(completed);
            setSimulating(false);
        }, 1600);
    };

    return (
        <div className="agent-runs">
            <div className="agent-runs__toolbar">
                <h2 className="agent-runs__title">Runs</h2>
                <button
                    className="agent-runs__btn-simulate"
                    onClick={startSimulation}
                    disabled={simulating}
                    style={btnStyle(simulating)}
                >
                    {simulating
                        ? <><span className="codicon codicon-loading codicon-modifier-spin" /> Simulating…</>
                        : <><span className="codicon codicon-play" /> Simulate Agent Run</>
                    }
                </button>
            </div>

            {runs.length === 0 ? (
                <div className="agent-runs__empty">
                    <p>No runs yet.</p>
                    <p style={{ color: '#888', fontSize: 12 }}>
                        Click <strong>Simulate Agent Run</strong> to see a demo run with full trace.
                    </p>
                </div>
            ) : (
                <div className="agent-runs__layout">
                    <RunList runs={runs} selected={selected} onSelect={setSelected} />
                    {selected && <TraceViewer run={selected} />}
                </div>
            )}
        </div>
    );
};

// ---------------------------------------------------------------------------
// Run list
// ---------------------------------------------------------------------------

interface RunListProps { runs: AgentRun[]; selected: AgentRun | null; onSelect: (r: AgentRun) => void; }
const RunList: React.FC<RunListProps> = ({ runs, selected, onSelect }) => (
    <ul className="agent-runs__list">
        {runs.map(run => (
            <li
                key={run.id}
                className={`agent-runs__run-item ${selected?.id === run.id ? 'agent-runs__run-item--active' : ''}`}
                onClick={() => onSelect(run)}
                style={runItemStyle(run, selected)}
            >
                <span className={`codicon ${statusIcon(run.status)}`} style={{ color: statusColor(run.status), marginRight: 6 }} />
                <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontWeight: 600, fontSize: 12, textTransform: 'capitalize' }}>
                        {run.agentId}
                    </div>
                    <div style={{ color: '#888', fontSize: 11 }}>
                        {run.taskId} &middot; {run.trace.length} steps
                    </div>
                </div>
                <span style={{ fontSize: 10, color: statusColor(run.status), whiteSpace: 'nowrap' }}>
                    {run.status}
                </span>
            </li>
        ))}
    </ul>
);

// ---------------------------------------------------------------------------
// Trace viewer — animates steps in on first render
// ---------------------------------------------------------------------------

interface TraceViewerProps { run: AgentRun; }
const TraceViewer: React.FC<TraceViewerProps> = ({ run }) => {
    const [visible, setVisible] = React.useState(0);

    React.useEffect(() => {
        setVisible(0);
        if (run.trace.length === 0) return;
        let i = 0;
        const interval = setInterval(() => {
            i += 1;
            setVisible(i);
            if (i >= run.trace.length) clearInterval(interval);
        }, 220);
        return () => clearInterval(interval);
    }, [run.id]);

    return (
        <div className="agent-trace" style={traceWrapStyle}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #333', fontSize: 11, color: '#aaa' }}>
                Trace &middot; {run.agentId} &middot; {run.trace.length} steps
            </div>
            <div style={{ padding: '8px 0', overflowY: 'auto', flex: 1 }}>
                {run.trace.slice(0, visible).map(step => (
                    <TraceStepRow key={step.id} step={step} />
                ))}
                {run.status === 'in_progress' && visible < run.trace.length && (
                    <div style={{ padding: '6px 12px', color: '#555', fontSize: 11 }}>
                        <span className="codicon codicon-loading codicon-modifier-spin" /> running…
                    </div>
                )}
            </div>
        </div>
    );
};

interface TraceStepRowProps { step: TraceStep; }
const TraceStepRow: React.FC<TraceStepRowProps> = ({ step }) => {
    const meta = STEP_META[step.type] ?? STEP_META['thought'];
    return (
        <div style={stepRowStyle}>
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
                <span
                    style={{
                        display: 'inline-block',
                        padding: '1px 6px',
                        borderRadius: 4,
                        fontSize: 10,
                        fontWeight: 700,
                        background: meta.bg,
                        color: meta.fg,
                        whiteSpace: 'nowrap',
                        minWidth: 72,
                        textAlign: 'center',
                    }}
                >
                    {step.type}
                </span>
                <span style={{ fontSize: 12, color: '#ddd', lineHeight: 1.4 }}>{step.content}</span>
            </div>
            {step.toolName && (
                <div style={{ marginTop: 4, marginLeft: 80, fontSize: 11, color: '#888' }}>
                    <span className="codicon codicon-tools" style={{ marginRight: 4 }} />
                    {step.toolName}
                    {step.toolInput && (
                        <span style={{ marginLeft: 6, color: '#555' }}>
                            {JSON.stringify(step.toolInput).slice(0, 60)}
                        </span>
                    )}
                </div>
            )}
            {step.durationMs !== undefined && (
                <div style={{ marginTop: 2, marginLeft: 80, fontSize: 10, color: '#555' }}>
                    {step.durationMs}ms
                </div>
            )}
        </div>
    );
};

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

const STEP_META: Record<string, { bg: string; fg: string }> = {
    thought:     { bg: '#2d3a5c', fg: '#7ab4ff' },
    action:      { bg: '#2d4a3a', fg: '#6dffab' },
    observation: { bg: '#3a3020', fg: '#ffd06d' },
    result:      { bg: '#3a2040', fg: '#d06dff' },
    error:       { bg: '#4a2020', fg: '#ff6d6d' },
};

function statusIcon(s: string) {
    switch (s) {
        case 'completed':  return 'codicon-pass-filled';
        case 'in_progress': return 'codicon-loading codicon-modifier-spin';
        case 'failed':     return 'codicon-error';
        default:           return 'codicon-circle-outline';
    }
}

function statusColor(s: string) {
    switch (s) {
        case 'completed':   return '#6dffab';
        case 'in_progress': return '#7ab4ff';
        case 'failed':      return '#ff6d6d';
        default:            return '#888';
    }
}

function btnStyle(disabled: boolean): React.CSSProperties {
    return {
        display: 'inline-flex', alignItems: 'center', gap: 6,
        padding: '5px 12px', borderRadius: 5, border: 'none', cursor: disabled ? 'not-allowed' : 'pointer',
        background: disabled ? '#333' : '#1a5c8a', color: disabled ? '#666' : '#fff',
        fontSize: 12, fontWeight: 600,
    };
}

function runItemStyle(run: AgentRun, selected: AgentRun | null): React.CSSProperties {
    return {
        display: 'flex', alignItems: 'center', gap: 8,
        padding: '7px 12px', cursor: 'pointer', borderBottom: '1px solid #222',
        background: selected?.id === run.id ? '#1e2a3a' : 'transparent',
    };
}

const stepRowStyle: React.CSSProperties = {
    padding: '6px 12px',
    borderBottom: '1px solid #1a1a1a',
};

const traceWrapStyle: React.CSSProperties = {
    display: 'flex', flexDirection: 'column',
    background: '#111', borderLeft: '1px solid #333',
    minWidth: 320, flex: 1,
};
