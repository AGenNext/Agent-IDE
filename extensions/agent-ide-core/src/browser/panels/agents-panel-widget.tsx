import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { AgentsPanelCommand } from '../agent-ide-commands';

// ---------------------------------------------------------------------------
// Domain models (local to simulation — not yet persisted)
// ---------------------------------------------------------------------------

export interface AgentModel {
    id: string;
    name: string;
    description: string;
    model: LLMModel;
    apiKey: string;        // dummy key for simulation
    temperature: number;
    maxTokens: number;
    systemPrompt: string;
    skills: string[];
    tools: string[];
}

type LLMModel = 'gpt-4o' | 'claude-3-5-sonnet' | 'llama-3-1-70b' | 'mistral-large' | 'custom';

const MODEL_OPTIONS: { id: LLMModel; label: string; inputRate: number; outputRate: number }[] = [
    { id: 'gpt-4o',           label: 'GPT-4o',             inputRate: 5,  outputRate: 15  },
    { id: 'claude-3-5-sonnet', label: 'Claude 3.5 Sonnet', inputRate: 3,  outputRate: 15  },
    { id: 'llama-3-1-70b',    label: 'Llama 3.1 70B',     inputRate: 0,  outputRate: 0   },
    { id: 'mistral-large',    label: 'Mistral Large',      inputRate: 4,  outputRate: 12  },
    { id: 'custom',           label: 'Custom / Local',     inputRate: 0,  outputRate: 0   },
];

const AVAILABLE_TOOLS = [
    'web_search', 'file_read', 'file_write', 'code_exec',
    'sql_query', 'http_get', 'send_email', 'summarise',
];

function makeDummyKey(): string {
    const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
    const suffix = Array.from({ length: 24 }, () => chars[Math.floor(Math.random() * chars.length)]).join('');
    return `sk-dummy-${suffix}`;
}

function newAgent(): AgentModel {
    return {
        id: `agent-${Date.now().toString(36)}`,
        name: '',
        description: '',
        model: 'gpt-4o',
        apiKey: makeDummyKey(),
        temperature: 0.7,
        maxTokens: 1024,
        systemPrompt: 'You are a helpful AI assistant operating within Agent Workspace OS.',
        skills: [],
        tools: ['web_search', 'file_write'],
    };
}

// ---------------------------------------------------------------------------
// Dummy LLM simulation engine
// ---------------------------------------------------------------------------

export interface SimStep {
    sequence: number;
    type: 'thought' | 'action' | 'observation' | 'result' | 'error';
    content: string;
    toolName?: string;
    toolInput?: Record<string, string>;
    inputTokens: number;
    outputTokens: number;
    loopIndex?: number;   // set when step is part of a retry/refinement loop
}

export interface SimResult {
    agentId: string;
    agentName: string;
    model: string;
    steps: SimStep[];
    totalInputTokens: number;
    totalOutputTokens: number;
    estimatedCostUsd: number;
    durationMs: number;
}

function approxTokens(text: string): number {
    return Math.max(1, Math.ceil(text.length / 3.8));
}

const THOUGHT_LINES = [
    'Decomposing the task into subtasks: (1) gather data, (2) analyse, (3) produce artifact.',
    'I should check existing knowledge before making external calls.',
    'The previous observation was incomplete — I will refine the query and retry.',
    'All subtasks resolved. Synthesising into a final result now.',
    'Tool returned an error. Adjusting parameters and retrying with narrower scope.',
];
const ACTION_STEPS: { content: string; tool: string; input: Record<string, string> }[] = [
    { content: 'Searching the web for recent information.',           tool: 'web_search', input: { query: 'latest agent orchestration patterns 2025' } },
    { content: 'Reading the specification file.',                    tool: 'file_read',  input: { path: 'spec/requirements.md' } },
    { content: 'Executing analysis script.',                        tool: 'code_exec',  input: { script: 'analyse.py', args: '--verbose' } },
    { content: 'Writing summary artifact to output directory.',     tool: 'file_write', input: { path: 'output/summary.md', size: '~1.4 KB' } },
    { content: 'Retrying search with refined query after loop #2.', tool: 'web_search', input: { query: 'agent governance policy enforcement examples' } },
];
const OBS_LINES = [
    'Search returned 11 results. Extracting top 3 relevant items.',
    'File read successfully (320 lines). Key sections: Overview, Constraints, Acceptance Criteria.',
    'Script output: 42 records processed, 3 anomalies flagged.',
    'File written (1.4 KB). SHA256 checksum recorded.',
    'Refined search returned 6 results. Loop resolved — proceeding to synthesis.',
];

function runDummySimulation(agent: AgentModel): SimResult {
    const modelMeta = MODEL_OPTIONS.find(m => m.id === agent.model) ?? MODEL_OPTIONS[0];
    const sysPromptTokens = approxTokens(agent.systemPrompt);
    const steps: SimStep[] = [];
    let seq = 0;
    let loopCount = 0;
    let totalInput = 0;
    let totalOutput = 0;

    // Helper: add a step
    const addStep = (type: SimStep['type'], content: string, extra?: Partial<SimStep>, contextTokens = 0) => {
        seq++;
        const outTok = approxTokens(content);
        const inTok  = sysPromptTokens + contextTokens + Math.floor(Math.random() * 40) + 20;
        totalInput  += inTok;
        totalOutput += outTok;
        steps.push({ sequence: seq, type, content, inputTokens: inTok, outputTokens: outTok, ...extra });
    };

    // Step 1 — initial thought
    addStep('thought', THOUGHT_LINES[0], {}, 0);

    // Steps 2-3 — first action + observation
    const a1 = ACTION_STEPS[0];
    addStep('action', a1.content, { toolName: a1.tool, toolInput: a1.input }, approxTokens(THOUGHT_LINES[0]));
    addStep('observation', OBS_LINES[0], {}, approxTokens(a1.content));

    // Loop: simulate a refinement cycle if the agent has >=2 tools
    if (agent.tools.length >= 2) {
        loopCount = 1;
        addStep('thought', THOUGHT_LINES[2], { loopIndex: loopCount }, approxTokens(OBS_LINES[0]));
        const a2 = ACTION_STEPS[4];
        addStep('action', a2.content, { toolName: a2.tool, toolInput: a2.input, loopIndex: loopCount }, approxTokens(THOUGHT_LINES[2]));
        addStep('observation', OBS_LINES[4], { loopIndex: loopCount }, approxTokens(a2.content));
    }

    // Steps N-1 and N — synthesise + result
    addStep('thought', THOUGHT_LINES[3], {}, approxTokens(OBS_LINES[4] ?? OBS_LINES[0]));
    const a3 = ACTION_STEPS[3];
    addStep('action', a3.content, { toolName: a3.tool, toolInput: a3.input }, approxTokens(THOUGHT_LINES[3]));
    addStep('result', 'Task complete. Artifact saved to output/summary.md. Total loop iterations: ' + loopCount + '.', {}, approxTokens(a3.content));

    const costPer1M = modelMeta.inputRate * totalInput + modelMeta.outputRate * totalOutput;
    const estimatedCostUsd = costPer1M / 1_000_000;

    return {
        agentId: agent.id,
        agentName: agent.name,
        model: agent.model,
        steps,
        totalInputTokens: totalInput,
        totalOutputTokens: totalOutput,
        estimatedCostUsd,
        durationMs: 400 + steps.length * 220 + Math.floor(Math.random() * 300),
    };
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

@injectable()
export class AgentsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:agents';
    static readonly LABEL = 'Agents';

    @postConstruct()
    protected init(): void {
        this.id = AgentsPanelWidget.ID;
        this.title.label = AgentsPanelWidget.LABEL;
        this.title.caption = 'Model, configure, and simulate AI agents';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-robot';
        this.update();
    }

    protected render(): React.ReactNode {
        return <AgentModelingView />;
    }
}

@injectable()
export class AgentsPanelContribution extends AbstractViewContribution<AgentsPanelWidget> {
    constructor() {
        super({
            widgetId: AgentsPanelWidget.ID,
            widgetName: AgentsPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'left', rank: 100 },
            toggleCommandId: AgentsPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(AgentsPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}

// ---------------------------------------------------------------------------
// React: top-level view
// ---------------------------------------------------------------------------

const AgentModelingView: React.FC = () => {
    const [agents, setAgents]       = React.useState<AgentModel[]>([]);
    const [editing, setEditing]     = React.useState<AgentModel | null>(null);
    const [simResult, setSimResult] = React.useState<SimResult | null>(null);
    const [simulating, setSimulating] = React.useState(false);
    const [visibleSteps, setVisibleSteps] = React.useState(0);

    const startNew = () => { setEditing(newAgent()); setSimResult(null); };
    const selectAgent = (a: AgentModel) => { setEditing({ ...a }); setSimResult(null); };

    const save = (a: AgentModel) => {
        setAgents(prev => {
            const idx = prev.findIndex(x => x.id === a.id);
            return idx >= 0 ? prev.map(x => x.id === a.id ? a : x) : [...prev, a];
        });
        setEditing(a);
    };

    const simulate = (a: AgentModel) => {
        if (!a.name) return;
        save(a);
        setSimulating(true);
        setSimResult(null);
        setVisibleSteps(0);

        // Run synchronously but reveal steps with animation
        const result = runDummySimulation(a);
        setTimeout(() => {
            setSimResult(result);
            setSimulating(false);
            let i = 0;
            const iv = setInterval(() => {
                i++;
                setVisibleSteps(i);
                if (i >= result.steps.length) clearInterval(iv);
            }, 280);
        }, 600);
    };

    const deleteAgent = (id: string) => {
        setAgents(prev => prev.filter(a => a.id !== id));
        if (editing?.id === id) { setEditing(null); setSimResult(null); }
    };

    return (
        <div style={{ display: 'flex', height: '100%', overflow: 'hidden', fontFamily: 'system-ui, sans-serif', fontSize: 12, color: '#ccc', background: '#111' }}>

            {/* Sidebar — agent list */}
            <div style={{ width: 180, borderRight: '1px solid #2a2a2a', display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
                <div style={{ padding: '10px 8px 6px', fontWeight: 700, fontSize: 11, color: '#888', letterSpacing: 1, textTransform: 'uppercase' }}>Agents</div>
                <div style={{ flex: 1, overflowY: 'auto' }}>
                    {agents.length === 0 && (
                        <div style={{ padding: '8px 10px', color: '#444', fontSize: 11 }}>No agents yet.</div>
                    )}
                    {agents.map(a => (
                        <div
                            key={a.id}
                            onClick={() => selectAgent(a)}
                            style={agentCardStyle(editing?.id === a.id)}
                        >
                            <span className="codicon codicon-robot" style={{ marginRight: 6, color: '#7ab4ff' }} />
                            <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{a.name || '(unnamed)'}</span>
                            <button
                                onClick={e => { e.stopPropagation(); deleteAgent(a.id); }}
                                style={{ background: 'none', border: 'none', color: '#555', cursor: 'pointer', padding: 0 }}
                                title="Delete agent"
                            >
                                <span className="codicon codicon-trash" />
                            </button>
                        </div>
                    ))}
                </div>
                <button onClick={startNew} style={newAgentBtnStyle}>
                    <span className="codicon codicon-add" /> New Agent
                </button>
            </div>

            {/* Center — model form */}
            {editing ? (
                <AgentModelForm
                    key={editing.id}
                    initial={editing}
                    onSave={save}
                    onSimulate={simulate}
                    simulating={simulating}
                />
            ) : (
                <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#333' }}>
                    <div style={{ textAlign: 'center' }}>
                        <span className="codicon codicon-robot" style={{ fontSize: 40 }} />
                        <p>Select an agent or click <strong style={{ color: '#7ab4ff' }}>New Agent</strong></p>
                    </div>
                </div>
            )}

            {/* Right — simulation results */}
            {simResult && (
                <SimResultPanel result={simResult} visibleSteps={visibleSteps} />
            )}
            {simulating && !simResult && (
                <div style={{ width: 320, borderLeft: '1px solid #2a2a2a', display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#555' }}>
                    <span className="codicon codicon-loading codicon-modifier-spin" style={{ marginRight: 8 }} /> Simulating…
                </div>
            )}
        </div>
    );
};

// ---------------------------------------------------------------------------
// Agent model form
// ---------------------------------------------------------------------------

interface FormProps { initial: AgentModel; onSave: (a: AgentModel) => void; onSimulate: (a: AgentModel) => void; simulating: boolean; }

const AgentModelForm: React.FC<FormProps> = ({ initial, onSave, onSimulate, simulating }) => {
    const [form, setForm] = React.useState<AgentModel>(initial);
    const [newSkill, setNewSkill] = React.useState('');

    const set = <K extends keyof AgentModel>(k: K, v: AgentModel[K]) => setForm(f => ({ ...f, [k]: v }));

    const addSkill = () => {
        const s = newSkill.trim();
        if (s && !form.skills.includes(s)) set('skills', [...form.skills, s]);
        setNewSkill('');
    };
    const removeSkill = (s: string) => set('skills', form.skills.filter(x => x !== s));
    const toggleTool = (t: string) => set('tools', form.tools.includes(t) ? form.tools.filter(x => x !== t) : [...form.tools, t]);

    const modelMeta = MODEL_OPTIONS.find(m => m.id === form.model)!;
    const isLocal = modelMeta.inputRate === 0 && modelMeta.outputRate === 0;

    return (
        <div style={{ flex: 1, overflowY: 'auto', padding: 16, minWidth: 0 }}>
            <div style={{ fontWeight: 700, fontSize: 13, marginBottom: 12, color: '#fff' }}>Agent Model</div>

            <Field label="Name">
                <input style={inputStyle} value={form.name} onChange={e => set('name', e.target.value)} placeholder="e.g. Research Agent" />
            </Field>

            <Field label="Description">
                <input style={inputStyle} value={form.description} onChange={e => set('description', e.target.value)} placeholder="What does this agent do?" />
            </Field>

            <Field label="Model">
                <select style={inputStyle} value={form.model} onChange={e => set('model', e.target.value as LLMModel)}>
                    {MODEL_OPTIONS.map(m => <option key={m.id} value={m.id}>{m.label}</option>)}
                </select>
            </Field>

            <Field label="API Key">
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    <input
                        style={{ ...inputStyle, flex: 1, fontFamily: 'monospace', fontSize: 11, color: '#ffd06d' }}
                        value={form.apiKey}
                        onChange={e => set('apiKey', e.target.value)}
                    />
                    <button onClick={() => set('apiKey', makeDummyKey())} style={smallBtnStyle} title="Regenerate dummy key">
                        <span className="codicon codicon-refresh" />
                    </button>
                </div>
                <div style={{ fontSize: 10, color: '#555', marginTop: 2 }}>
                    {isLocal ? 'Local model — no API key required.' : 'Simulation uses this key label only. No real API calls are made.'}
                </div>
            </Field>

            <div style={{ display: 'flex', gap: 12 }}>
                <Field label={`Temperature (${form.temperature.toFixed(1)})`} style={{ flex: 1 }}>
                    <input type="range" min={0} max={2} step={0.1} value={form.temperature}
                        onChange={e => set('temperature', parseFloat(e.target.value))}
                        style={{ width: '100%', accentColor: '#7ab4ff' }}
                    />
                </Field>
                <Field label="Max Tokens" style={{ flex: 1 }}>
                    <input type="number" style={inputStyle} min={64} max={32768} step={64}
                        value={form.maxTokens} onChange={e => set('maxTokens', parseInt(e.target.value))} />
                </Field>
            </div>

            <Field label="System Prompt">
                <textarea
                    style={{ ...inputStyle, height: 64, resize: 'vertical', fontFamily: 'monospace', fontSize: 11 }}
                    value={form.systemPrompt}
                    onChange={e => set('systemPrompt', e.target.value)}
                />
            </Field>

            <Field label="Skills (capabilities this agent declares)">
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginBottom: 4 }}>
                    {form.skills.map(s => (
                        <span key={s} style={tagStyle}>
                            {s}
                            <button onClick={() => removeSkill(s)} style={{ background: 'none', border: 'none', color: '#888', cursor: 'pointer', padding: '0 0 0 4px' }}>×</button>
                        </span>
                    ))}
                </div>
                <div style={{ display: 'flex', gap: 4 }}>
                    <input style={{ ...inputStyle, flex: 1 }} value={newSkill}
                        onChange={e => setNewSkill(e.target.value)}
                        onKeyDown={e => e.key === 'Enter' && addSkill()}
                        placeholder="Add skill and press Enter"
                    />
                    <button onClick={addSkill} style={smallBtnStyle}><span className="codicon codicon-add" /></button>
                </div>
            </Field>

            <Field label="Tools">
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                    {AVAILABLE_TOOLS.map(t => (
                        <label key={t} style={{ display: 'flex', alignItems: 'center', gap: 4, cursor: 'pointer', fontSize: 11 }}>
                            <input type="checkbox" checked={form.tools.includes(t)} onChange={() => toggleTool(t)}
                                style={{ accentColor: '#6dffab' }} />
                            {t}
                        </label>
                    ))}
                </div>
            </Field>

            <div style={{ display: 'flex', gap: 8, marginTop: 16 }}>
                <button onClick={() => onSave(form)} style={btnStyle(false, false)}>Save</button>
                <button
                    onClick={() => onSimulate(form)}
                    disabled={simulating || !form.name}
                    style={btnStyle(simulating || !form.name, true)}
                >
                    {simulating
                        ? <><span className="codicon codicon-loading codicon-modifier-spin" /> Simulating…</>
                        : <><span className="codicon codicon-play" /> Simulate</>}
                </button>
            </div>
        </div>
    );
};

// ---------------------------------------------------------------------------
// Simulation results panel
// ---------------------------------------------------------------------------

const STEP_META: Record<string, { bg: string; fg: string; icon: string }> = {
    thought:     { bg: '#2d3a5c', fg: '#7ab4ff', icon: 'codicon-lightbulb' },
    action:      { bg: '#2d4a3a', fg: '#6dffab', icon: 'codicon-tools'     },
    observation: { bg: '#3a3020', fg: '#ffd06d', icon: 'codicon-eye'        },
    result:      { bg: '#3a2040', fg: '#d06dff', icon: 'codicon-pass-filled'},
    error:       { bg: '#4a2020', fg: '#ff6d6d', icon: 'codicon-error'      },
};

interface SimResultPanelProps { result: SimResult; visibleSteps: number; }
const SimResultPanel: React.FC<SimResultPanelProps> = ({ result, visibleSteps }) => {
    const displayedSteps = result.steps.slice(0, visibleSteps);
    const inputSoFar  = displayedSteps.reduce((s, x) => s + x.inputTokens,  0);
    const outputSoFar = displayedSteps.reduce((s, x) => s + x.outputTokens, 0);
    const done = visibleSteps >= result.steps.length;

    // Detect loops: steps with loopIndex set
    const loopSteps = result.steps.filter(s => s.loopIndex !== undefined && s.loopIndex > 0);
    const loopCount = loopSteps.length > 0 ? Math.max(...loopSteps.map(s => s.loopIndex ?? 0)) : 0;

    return (
        <div style={{ width: 340, borderLeft: '1px solid #2a2a2a', display: 'flex', flexDirection: 'column', flexShrink: 0, overflow: 'hidden' }}>

            {/* Header */}
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #2a2a2a', fontSize: 11 }}>
                <div style={{ fontWeight: 700, color: '#fff', marginBottom: 2 }}>
                    Simulation — {result.agentName}
                </div>
                <div style={{ color: '#555' }}>{result.model} &middot; {result.steps.length} steps{loopCount > 0 ? ` · ${loopCount} loop` : ''}</div>
            </div>

            {/* Token meter */}
            <TokenMeter
                inputTokens={inputSoFar}
                outputTokens={outputSoFar}
                maxTokens={result.totalInputTokens + result.totalOutputTokens}
                costUsd={done ? result.estimatedCostUsd : undefined}
            />

            {/* Steps */}
            <div style={{ flex: 1, overflowY: 'auto', padding: '4px 0' }}>
                {displayedSteps.map((step, i) => (
                    <SimStepRow key={i} step={step} />
                ))}
                {!done && (
                    <div style={{ padding: '6px 12px', color: '#444', fontSize: 11 }}>
                        <span className="codicon codicon-loading codicon-modifier-spin" /> running…
                    </div>
                )}
            </div>

            {/* Summary */}
            {done && (
                <div style={{ borderTop: '1px solid #2a2a2a', padding: '8px 12px', fontSize: 11, color: '#888' }}>
                    Total &nbsp;
                    <span style={{ color: '#7ab4ff' }}>{result.totalInputTokens.toLocaleString()} in</span>
                    {' + '}
                    <span style={{ color: '#6dffab' }}>{result.totalOutputTokens.toLocaleString()} out</span>
                    {' = '}
                    <span style={{ color: '#fff', fontWeight: 700 }}>{(result.totalInputTokens + result.totalOutputTokens).toLocaleString()} tokens</span>
                    {result.estimatedCostUsd > 0 && (
                        <span style={{ marginLeft: 8, color: '#ffd06d' }}>
                            ~${result.estimatedCostUsd.toFixed(4)}
                        </span>
                    )}
                    <br />
                    {loopCount > 0 && (
                        <span style={{ color: '#d06dff' }}>{loopCount} refinement loop(s) detected.</span>
                    )}
                </div>
            )}
        </div>
    );
};

const TokenMeter: React.FC<{ inputTokens: number; outputTokens: number; maxTokens: number; costUsd?: number }> = ({
    inputTokens, outputTokens, maxTokens, costUsd,
}) => {
    const total = inputTokens + outputTokens;
    const pct = maxTokens > 0 ? Math.min(100, (total / maxTokens) * 100) : 0;
    return (
        <div style={{ padding: '8px 12px', borderBottom: '1px solid #222' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: '#666', marginBottom: 4 }}>
                <span><span style={{ color: '#7ab4ff' }}>{inputTokens.toLocaleString()}</span> in / <span style={{ color: '#6dffab' }}>{outputTokens.toLocaleString()}</span> out</span>
                <span style={{ color: '#fff' }}>{total.toLocaleString()} tokens</span>
            </div>
            <div style={{ height: 4, background: '#222', borderRadius: 2, overflow: 'hidden' }}>
                <div style={{ height: '100%', width: `${pct}%`, background: 'linear-gradient(90deg, #1a5c8a, #6dffab)', borderRadius: 2, transition: 'width 0.3s' }} />
            </div>
            {costUsd !== undefined && costUsd > 0 && (
                <div style={{ fontSize: 10, color: '#ffd06d', marginTop: 3 }}>~${costUsd.toFixed(6)} estimated cost</div>
            )}
        </div>
    );
};

const SimStepRow: React.FC<{ step: SimStep }> = ({ step }) => {
    const meta = STEP_META[step.type] ?? STEP_META['thought'];
    return (
        <div style={{ padding: '6px 12px', borderBottom: '1px solid #1a1a1a' }}>
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: 6 }}>
                {step.loopIndex !== undefined && step.loopIndex > 0 && (
                    <span style={{ fontSize: 9, color: '#d06dff', border: '1px solid #d06dff', borderRadius: 3, padding: '0 4px', whiteSpace: 'nowrap', marginTop: 1 }}>
                        loop {step.loopIndex}
                    </span>
                )}
                <span style={{ background: meta.bg, color: meta.fg, fontSize: 10, fontWeight: 700, padding: '1px 6px', borderRadius: 4, whiteSpace: 'nowrap' }}>
                    {step.type}
                </span>
                <span style={{ fontSize: 11, color: '#ccc', lineHeight: 1.4, flex: 1 }}>{step.content}</span>
            </div>
            {step.toolName && (
                <div style={{ marginTop: 4, paddingLeft: step.loopIndex ? 52 : 46, fontSize: 10, color: '#888' }}>
                    <span className="codicon codicon-tools" style={{ marginRight: 4 }} />
                    <span style={{ color: '#6dffab' }}>{step.toolName}</span>
                    {step.toolInput && (
                        <span style={{ color: '#444', marginLeft: 6 }}>{JSON.stringify(step.toolInput).slice(0, 50)}</span>
                    )}
                </div>
            )}
            <div style={{ marginTop: 3, paddingLeft: step.loopIndex ? 52 : 46, fontSize: 10, color: '#444', display: 'flex', gap: 10 }}>
                <span><span style={{ color: '#7ab4ff' }}>{step.inputTokens}</span> in</span>
                <span><span style={{ color: '#6dffab' }}>{step.outputTokens}</span> out</span>
            </div>
        </div>
    );
};

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

const inputStyle: React.CSSProperties = {
    width: '100%', boxSizing: 'border-box',
    background: '#1a1a2e', border: '1px solid #2a2a4a',
    color: '#ccc', padding: '4px 8px', borderRadius: 4, fontSize: 12,
    fontFamily: 'inherit',
};

const tagStyle: React.CSSProperties = {
    background: '#2d3a5c', color: '#7ab4ff', padding: '2px 8px',
    borderRadius: 12, fontSize: 11, display: 'inline-flex', alignItems: 'center',
};

const smallBtnStyle: React.CSSProperties = {
    background: '#252535', border: '1px solid #333', color: '#aaa',
    borderRadius: 4, padding: '3px 8px', cursor: 'pointer',
};

const newAgentBtnStyle: React.CSSProperties = {
    margin: 8, padding: '5px 0', borderRadius: 5, border: '1px solid #2a4a2a',
    background: '#1a2a1a', color: '#6dffab', cursor: 'pointer', fontSize: 12,
    display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 4,
};

function agentCardStyle(active: boolean): React.CSSProperties {
    return {
        display: 'flex', alignItems: 'center', padding: '6px 10px', cursor: 'pointer',
        background: active ? '#1e2a3a' : 'transparent',
        borderLeft: active ? '2px solid #7ab4ff' : '2px solid transparent',
        fontSize: 12,
    };
}

function btnStyle(disabled: boolean, primary: boolean): React.CSSProperties {
    return {
        display: 'inline-flex', alignItems: 'center', gap: 6,
        padding: '5px 14px', borderRadius: 5, border: 'none',
        background: disabled ? '#2a2a2a' : primary ? '#1a5c8a' : '#252535',
        color: disabled ? '#444' : primary ? '#fff' : '#ccc',
        cursor: disabled ? 'not-allowed' : 'pointer', fontSize: 12, fontWeight: 600,
    };
}

const Field: React.FC<{ label: string; children: React.ReactNode; style?: React.CSSProperties }> = ({ label, children, style }) => (
    <div style={{ marginBottom: 10, ...style }}>
        <label style={{ display: 'block', fontSize: 10, color: '#666', marginBottom: 3, textTransform: 'uppercase', letterSpacing: 0.5 }}>{label}</label>
        {children}
    </div>
);
