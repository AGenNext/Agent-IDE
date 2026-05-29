import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { ReplayPanelCommand } from '../agent-ide-commands';
import { TraceStep } from '@agennext/agent-ide-types';

// Demo trace used when Replay is opened standalone (no run selected via Runs panel).
// TODO: in Phase 2, Replay receives a real AgentRun injected from the backend.
const DEMO_TRACE: TraceStep[] = [
    { id: 's1', runId: 'demo', sequence: 1, type: 'thought',     content: 'I need to research the topic, then write a structured report.',              timestamp: '2026-01-01T10:00:00Z', durationMs: 85  },
    { id: 's2', runId: 'demo', sequence: 2, type: 'action',      content: 'search_web',    toolName: 'web_search',  toolInput: { query: 'eclipse theia architecture' }, timestamp: '2026-01-01T10:00:01Z', durationMs: 720 },
    { id: 's3', runId: 'demo', sequence: 3, type: 'observation',  content: 'Theia is a cloud and desktop IDE platform built on Monaco + LSP + DI.',    timestamp: '2026-01-01T10:00:02Z', durationMs: 40  },
    { id: 's4', runId: 'demo', sequence: 4, type: 'thought',     content: 'Good. Now I will write a concise summary and save it as an artifact.',      timestamp: '2026-01-01T10:00:02Z', durationMs: 65  },
    { id: 's5', runId: 'demo', sequence: 5, type: 'action',      content: 'write_file',    toolName: 'file_write',  toolInput: { path: 'output/theia-summary.md' }, timestamp: '2026-01-01T10:00:03Z', durationMs: 55  },
    { id: 's6', runId: 'demo', sequence: 6, type: 'result',      content: 'Report saved. Task complete (1 artifact produced).',                        timestamp: '2026-01-01T10:00:03Z', durationMs: 8   },
];

@injectable()
export class ReplayPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:replay';
    static readonly LABEL = 'Replay';

    @postConstruct()
    protected init(): void {
        this.id = ReplayPanelWidget.ID;
        this.title.label = ReplayPanelWidget.LABEL;
        this.title.caption = 'Step through past agent runs';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-history';
        this.update();
    }

    protected render(): React.ReactNode {
        return <ReplayView trace={DEMO_TRACE} runLabel="demo-run" />;
    }
}

@injectable()
export class ReplayPanelContribution extends AbstractViewContribution<ReplayPanelWidget> {
    constructor() {
        super({
            widgetId: ReplayPanelWidget.ID,
            widgetName: ReplayPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'bottom', rank: 200 },
            toggleCommandId: ReplayPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(ReplayPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}

// ---------------------------------------------------------------------------
// ReplayView — manual step-through with play/pause and scrubber
// ---------------------------------------------------------------------------

interface ReplayViewProps { trace: TraceStep[]; runLabel: string; }

const STEP_META: Record<string, { bg: string; fg: string; icon: string }> = {
    thought:     { bg: '#2d3a5c', fg: '#7ab4ff', icon: 'codicon-lightbulb'     },
    action:      { bg: '#2d4a3a', fg: '#6dffab', icon: 'codicon-tools'          },
    observation: { bg: '#3a3020', fg: '#ffd06d', icon: 'codicon-eye'            },
    result:      { bg: '#3a2040', fg: '#d06dff', icon: 'codicon-pass-filled'    },
    error:       { bg: '#4a2020', fg: '#ff6d6d', icon: 'codicon-error'          },
};

const ReplayView: React.FC<ReplayViewProps> = ({ trace, runLabel }) => {
    const [cursor, setCursor] = React.useState(0);   // 0 = before step 1
    const [playing, setPlaying] = React.useState(false);
    const intervalRef = React.useRef<ReturnType<typeof setInterval> | null>(null);

    const total = trace.length;
    const currentStep = cursor > 0 ? trace[cursor - 1] : null;

    // Auto-play: advance one step per 900 ms
    React.useEffect(() => {
        if (playing) {
            intervalRef.current = setInterval(() => {
                setCursor(c => {
                    if (c >= total) {
                        setPlaying(false);
                        return c;
                    }
                    return c + 1;
                });
            }, 900);
        } else if (intervalRef.current) {
            clearInterval(intervalRef.current);
        }
        return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
    }, [playing, total]);

    const reset = () => { setPlaying(false); setCursor(0); };
    const stepBack = () => { setPlaying(false); setCursor(c => Math.max(0, c - 1)); };
    const stepFwd  = () => { setPlaying(false); setCursor(c => Math.min(total, c + 1)); };
    const togglePlay = () => {
        if (cursor >= total) { setCursor(0); setPlaying(true); }
        else setPlaying(p => !p);
    };

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'system-ui, sans-serif' }}>

            {/* Header */}
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #2a2a2a', display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                <span className="codicon codicon-history" style={{ color: '#7ab4ff' }} />
                <span style={{ fontWeight: 700, fontSize: 13 }}>Replay</span>
                <span style={{ color: '#555', fontSize: 11 }}>{runLabel}</span>
                <span style={{ marginLeft: 'auto', fontSize: 11, color: '#888' }}>
                    Step {cursor} / {total}
                </span>
            </div>

            {/* Controls */}
            <div style={{ padding: '8px 12px', display: 'flex', alignItems: 'center', gap: 8, borderBottom: '1px solid #2a2a2a' }}>
                <CtrlBtn icon="codicon-debug-restart" title="Reset" onClick={reset} />
                <CtrlBtn icon="codicon-debug-step-back" title="Step back" onClick={stepBack} disabled={cursor === 0} />
                <CtrlBtn
                    icon={playing ? 'codicon-debug-pause' : 'codicon-debug-start'}
                    title={playing ? 'Pause' : 'Play'}
                    onClick={togglePlay}
                    primary
                />
                <CtrlBtn icon="codicon-debug-step-over" title="Step forward" onClick={stepFwd} disabled={cursor >= total} />

                {/* Scrubber */}
                <input
                    type="range"
                    min={0}
                    max={total}
                    value={cursor}
                    onChange={e => { setPlaying(false); setCursor(Number(e.target.value)); }}
                    style={{ flex: 1, accentColor: '#7ab4ff', cursor: 'pointer' }}
                />
            </div>

            {/* Step breadcrumb */}
            <div style={{ padding: '6px 12px', display: 'flex', gap: 4, flexWrap: 'wrap', borderBottom: '1px solid #222' }}>
                {trace.map((s, i) => {
                    const meta = STEP_META[s.type] ?? STEP_META['thought'];
                    const active = i < cursor;
                    const current = i === cursor - 1;
                    return (
                        <button
                            key={s.id}
                            title={`Step ${i + 1}: ${s.type}`}
                            onClick={() => { setPlaying(false); setCursor(i + 1); }}
                            style={{
                                width: 22, height: 22, borderRadius: '50%', border: current ? `2px solid ${meta.fg}` : '2px solid transparent',
                                background: active ? meta.bg : '#1e1e1e', color: active ? meta.fg : '#444',
                                cursor: 'pointer', fontSize: 10, fontWeight: 700, padding: 0,
                            }}
                        >
                            {i + 1}
                        </button>
                    );
                })}
            </div>

            {/* Active step detail */}
            <div style={{ flex: 1, padding: 12, overflowY: 'auto' }}>
                {currentStep ? (
                    <StepDetail step={currentStep} />
                ) : (
                    <div style={{ color: '#555', fontSize: 12, paddingTop: 12 }}>
                        Press <strong style={{ color: '#7ab4ff' }}>Play</strong> or click a step circle to begin replay.
                    </div>
                )}
            </div>

            {/* Trace list — scrollable history of visited steps */}
            {cursor > 0 && (
                <div style={{ borderTop: '1px solid #222', maxHeight: 160, overflowY: 'auto', padding: '4px 0' }}>
                    {trace.slice(0, cursor).map(s => (
                        <MiniStepRow key={s.id} step={s} />
                    ))}
                </div>
            )}
        </div>
    );
};

const StepDetail: React.FC<{ step: TraceStep }> = ({ step }) => {
    const meta = STEP_META[step.type] ?? STEP_META['thought'];
    return (
        <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 10 }}>
                <span className={`codicon ${meta.icon}`} style={{ color: meta.fg, fontSize: 18 }} />
                <span style={{ background: meta.bg, color: meta.fg, padding: '2px 10px', borderRadius: 5, fontSize: 12, fontWeight: 700 }}>
                    {step.type}
                </span>
                <span style={{ color: '#555', fontSize: 11 }}>step {step.sequence}</span>
                {step.durationMs !== undefined && (
                    <span style={{ color: '#555', fontSize: 11, marginLeft: 'auto' }}>{step.durationMs}ms</span>
                )}
            </div>
            <p style={{ fontSize: 13, lineHeight: 1.6, color: '#ddd', margin: '0 0 10px' }}>{step.content}</p>
            {step.toolName && (
                <div style={{ background: '#1a1a1a', borderRadius: 6, padding: '8px 12px', fontSize: 12 }}>
                    <div style={{ color: '#6dffab', marginBottom: 4 }}>
                        <span className="codicon codicon-tools" style={{ marginRight: 6 }} />
                        {step.toolName}
                    </div>
                    {step.toolInput && (
                        <pre style={{ margin: 0, color: '#aaa', fontSize: 11, overflowX: 'auto' }}>
                            {JSON.stringify(step.toolInput, null, 2)}
                        </pre>
                    )}
                    {step.toolOutput !== undefined && (
                        <pre style={{ margin: '6px 0 0', color: '#ffd06d', fontSize: 11, overflowX: 'auto' }}>
                            {JSON.stringify(step.toolOutput, null, 2)}
                        </pre>
                    )}
                </div>
            )}
        </div>
    );
};

const MiniStepRow: React.FC<{ step: TraceStep }> = ({ step }) => {
    const meta = STEP_META[step.type] ?? STEP_META['thought'];
    return (
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8, padding: '3px 12px' }}>
            <span style={{ color: meta.fg, fontSize: 10, minWidth: 60, paddingTop: 1 }}>{step.type}</span>
            <span style={{ color: '#888', fontSize: 11, lineHeight: 1.4 }}>{step.content.slice(0, 80)}{step.content.length > 80 ? '…' : ''}</span>
        </div>
    );
};

interface CtrlBtnProps { icon: string; title: string; onClick: () => void; disabled?: boolean; primary?: boolean; }
const CtrlBtn: React.FC<CtrlBtnProps> = ({ icon, title, onClick, disabled, primary }) => (
    <button
        title={title}
        onClick={onClick}
        disabled={disabled}
        style={{
            width: 28, height: 28, borderRadius: 5, border: 'none',
            background: primary ? '#1a5c8a' : '#252525',
            color: disabled ? '#444' : primary ? '#fff' : '#ccc',
            cursor: disabled ? 'not-allowed' : 'pointer',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}
    >
        <span className={`codicon ${icon}`} />
    </button>
);
