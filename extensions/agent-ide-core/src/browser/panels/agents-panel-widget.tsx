import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { AgentsPanelCommand } from '../agent-ide-commands';
import {
    listAgentIdentities, createAgentIdentity, updateAgentIdentity, deleteAgentIdentity,
    AgentIdentity, submitRun, streamRun, RunRequest,
} from '../runtime/backend-client';

type LLMModel = 'claude-opus-4-8' | 'claude-sonnet-4-6' | 'claude-haiku-4-5' | 'gpt-4o' | 'gpt-4o-mini' | 'gemini-1.5-pro';
type FormSection = 'overview' | 'model' | 'prompt' | 'tools' | 'skills' | 'agents';

const MODEL_OPTIONS: { value: LLMModel; label: string; tokens: number; costPer1k: number }[] = [
    { value: 'claude-opus-4-8',   label: 'Claude Opus 4.8',    tokens: 200000, costPer1k: 0.015 },
    { value: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6',  tokens: 200000, costPer1k: 0.003 },
    { value: 'claude-haiku-4-5',  label: 'Claude Haiku 4.5',   tokens: 200000, costPer1k: 0.00025 },
    { value: 'gpt-4o',            label: 'GPT-4o',              tokens: 128000, costPer1k: 0.005 },
    { value: 'gpt-4o-mini',       label: 'GPT-4o Mini',         tokens: 128000, costPer1k: 0.00015 },
    { value: 'gemini-1.5-pro',    label: 'Gemini 1.5 Pro',      tokens: 1000000, costPer1k: 0.00125 },
];

const AVAILABLE_TOOLS = [
    { id: 'browser',     name: 'Browser',       desc: 'Web navigation and scraping' },
    { id: 'code_exec',   name: 'Code Executor',  desc: 'Run Python/JS in sandbox' },
    { id: 'file_rw',     name: 'File R/W',       desc: 'Read and write workspace files' },
    { id: 'pdf_reader',  name: 'PDF Reader',     desc: 'Extract text from PDFs' },
    { id: 'http_client', name: 'HTTP Client',    desc: 'Make outbound API calls' },
    { id: 'db_query',    name: 'DB Query',       desc: 'Query structured databases' },
    { id: 'vector_search', name: 'Vector Search', desc: 'Semantic search over embeddings' },
    { id: 'shell',       name: 'Shell',          desc: 'Execute shell commands (sandboxed)' },
];


interface AgentModel {
    id: string;
    name: string;
    description: string;
    model: LLMModel;
    apiKey: string;
    temperature: number;
    maxTokens: number;
    systemPrompt: string;
    fewShotExamples: string;
    skills: string[];
    tools: string[];
    subAgentIds: string[];
}

interface SimStep {
    sequence: number;
    type: 'thought' | 'action' | 'observation' | 'result' | 'error';
    content: string;
    toolName?: string;
    toolInput?: Record<string, unknown>;
    inputTokens: number;
    outputTokens: number;
    loopIndex?: number;
}

interface SimResult {
    agentId: string;
    agentName: string;
    model: string;
    steps: SimStep[];
    totalInputTokens: number;
    totalOutputTokens: number;
    estimatedCostUsd: number;
    durationMs: number;
}

function makeApiKey(): string {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    return 'sk-' + Array.from({ length: 48 }, () => chars[Math.floor(Math.random() * chars.length)]).join('');
}

function makeDefaultAgent(): AgentModel {
    return {
        id: 'agent-' + Date.now(),
        name: 'New Agent',
        description: 'A general-purpose agent',
        model: 'claude-sonnet-4-6',
        apiKey: makeApiKey(),
        temperature: 0.7,
        maxTokens: 4096,
        systemPrompt: 'You are a helpful assistant. Break tasks into steps, use available tools, and produce clear outputs.',
        fewShotExamples: '',
        skills: [],
        tools: [],
        subAgentIds: [],
    };
}

// ─── Sub-components ───────────────────────────────────────────────────────────

const SECTION_TABS: { id: FormSection; label: string; icon: string }[] = [
    { id: 'overview', label: 'Overview', icon: '◎' },
    { id: 'model',    label: 'Model',    icon: '⬡' },
    { id: 'prompt',   label: 'Prompt',   icon: '✎' },
    { id: 'tools',    label: 'Tools',    icon: '⚙' },
    { id: 'skills',   label: 'Skills',   icon: '★' },
    { id: 'agents',   label: 'Agents',   icon: '⇆' },
];

function SectionTabs({ active, onChange }: { active: FormSection; onChange: (s: FormSection) => void }) {
    return (
        <div style={{ display: 'flex', borderBottom: '1px solid #2a2a2a', background: '#0f0f0f' }}>
            {SECTION_TABS.map(t => (
                <button key={t.id} onClick={() => onChange(t.id)} style={{
                    padding: '7px 12px', border: 'none', background: 'transparent',
                    color: active === t.id ? '#60b0ff' : '#666',
                    borderBottom: active === t.id ? '2px solid #60b0ff' : '2px solid transparent',
                    cursor: 'pointer', fontSize: 12, fontWeight: active === t.id ? 700 : 400,
                    display: 'flex', alignItems: 'center', gap: 5
                }}>
                    <span style={{ fontSize: 11 }}>{t.icon}</span>{t.label}
                </button>
            ))}
        </div>
    );
}

function OverviewSection({ agent, onChange }: { agent: AgentModel; onChange: (a: AgentModel) => void }) {
    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
            <label style={{ fontSize: 11, color: '#888' }}>Name
                <input value={agent.name} onChange={e => onChange({ ...agent, name: e.target.value })}
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 13, boxSizing: 'border-box' }} />
            </label>
            <label style={{ fontSize: 11, color: '#888' }}>Description
                <textarea value={agent.description} onChange={e => onChange({ ...agent, description: e.target.value })} rows={3}
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 12, resize: 'vertical', boxSizing: 'border-box' }} />
            </label>
            <div style={{ background: '#1a1a1a', borderRadius: 4, padding: 10, fontSize: 11, color: '#555', border: '1px solid #222' }}>
                <div style={{ color: '#888', marginBottom: 4 }}>Agent ID</div>
                <span style={{ fontFamily: 'monospace', color: '#70a0d0' }}>{agent.id}</span>
            </div>
        </div>
    );
}

function ModelSection({ agent, onChange }: { agent: AgentModel; onChange: (a: AgentModel) => void }) {
    const modelInfo = MODEL_OPTIONS.find(m => m.value === agent.model)!;
    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 14 }}>
            <label style={{ fontSize: 11, color: '#888' }}>Model
                <select value={agent.model} onChange={e => onChange({ ...agent, model: e.target.value as LLMModel })}
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 13 }}>
                    {MODEL_OPTIONS.map(m => <option key={m.value} value={m.value}>{m.label}</option>)}
                </select>
            </label>
            <div style={{ display: 'flex', gap: 10, fontSize: 11, color: '#666' }}>
                <span>Context: <b style={{ color: '#a0c0e0' }}>{(modelInfo.tokens / 1000).toFixed(0)}K</b></span>
                <span>Cost/1K tok: <b style={{ color: '#a0c0e0' }}>${modelInfo.costPer1k}</b></span>
            </div>
            <label style={{ fontSize: 11, color: '#888' }}>API Key
                <div style={{ display: 'flex', gap: 6, marginTop: 4 }}>
                    <input type="password" value={agent.apiKey} readOnly
                        style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 12, fontFamily: 'monospace' }} />
                    <button onClick={() => onChange({ ...agent, apiKey: makeApiKey() })}
                        style={{ padding: '4px 10px', background: '#1e2e1e', border: '1px solid #3a4a3a', color: '#70d070', borderRadius: 4, cursor: 'pointer', fontSize: 11 }}>Regen</button>
                </div>
            </label>
            <label style={{ fontSize: 11, color: '#888' }}>
                Temperature: <b style={{ color: '#ccc' }}>{agent.temperature.toFixed(2)}</b>
                <input type="range" min={0} max={1} step={0.01} value={agent.temperature}
                    onChange={e => onChange({ ...agent, temperature: parseFloat(e.target.value) })}
                    style={{ display: 'block', width: '100%', marginTop: 4 }} />
            </label>
            <label style={{ fontSize: 11, color: '#888' }}>Max Output Tokens
                <input type="number" min={256} max={32768} step={256} value={agent.maxTokens}
                    onChange={e => onChange({ ...agent, maxTokens: parseInt(e.target.value) })}
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 13, boxSizing: 'border-box' }} />
            </label>
        </div>
    );
}

function PromptSection({ agent, onChange }: { agent: AgentModel; onChange: (a: AgentModel) => void }) {
    const sysTok = Math.ceil(agent.systemPrompt.length / 3.8);
    const fewTok = Math.ceil(agent.fewShotExamples.length / 3.8);
    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 14 }}>
            <label style={{ fontSize: 11, color: '#888' }}>
                System Prompt
                <span style={{ float: 'right', color: '#556', fontSize: 10 }}>~{sysTok} tokens</span>
                <textarea value={agent.systemPrompt} onChange={e => onChange({ ...agent, systemPrompt: e.target.value })} rows={8}
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '8px', borderRadius: 4, fontSize: 12, fontFamily: 'monospace', resize: 'vertical', lineHeight: 1.5, boxSizing: 'border-box' }} />
            </label>
            <label style={{ fontSize: 11, color: '#888' }}>
                Few-Shot Examples
                <span style={{ float: 'right', color: '#556', fontSize: 10 }}>~{fewTok} tokens</span>
                <textarea value={agent.fewShotExamples} onChange={e => onChange({ ...agent, fewShotExamples: e.target.value })} rows={5}
                    placeholder="User: ...\nAssistant: ..."
                    style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '8px', borderRadius: 4, fontSize: 12, fontFamily: 'monospace', resize: 'vertical', lineHeight: 1.5, boxSizing: 'border-box' }} />
            </label>
            <div style={{ fontSize: 11, color: '#555' }}>Total prompt overhead: <b style={{ color: '#a0b0c0' }}>~{sysTok + fewTok}</b> tokens per call</div>
        </div>
    );
}

function ToolsSection({ agent, onChange }: { agent: AgentModel; onChange: (a: AgentModel) => void }) {
    const toggle = (id: string) => {
        const next = agent.tools.includes(id) ? agent.tools.filter(t => t !== id) : [...agent.tools, id];
        onChange({ ...agent, tools: next });
    };
    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ fontSize: 11, color: '#666', marginBottom: 4 }}>Select tools this agent can invoke:</div>
            {AVAILABLE_TOOLS.map(t => (
                <label key={t.id} style={{ display: 'flex', alignItems: 'flex-start', gap: 10, cursor: 'pointer', padding: '8px 10px', background: agent.tools.includes(t.id) ? '#1a2a1a' : '#141414', borderRadius: 4, border: `1px solid ${agent.tools.includes(t.id) ? '#2a4a2a' : '#222'}` }}>
                    <input type="checkbox" checked={agent.tools.includes(t.id)} onChange={() => toggle(t.id)} style={{ marginTop: 2 }} />
                    <div>
                        <div style={{ fontSize: 12, fontWeight: 600, color: agent.tools.includes(t.id) ? '#70d070' : '#ccc' }}>{t.name}</div>
                        <div style={{ fontSize: 11, color: '#666' }}>{t.desc}</div>
                    </div>
                </label>
            ))}
            <div style={{ fontSize: 11, color: '#555', marginTop: 4 }}>{agent.tools.length} tool(s) selected</div>
        </div>
    );
}

function SkillsSection({ agent, onChange }: { agent: AgentModel; onChange: (a: AgentModel) => void }) {
    const [input, setInput] = React.useState('');
    const add = () => {
        const v = input.trim();
        if (v && !agent.skills.includes(v)) { onChange({ ...agent, skills: [...agent.skills, v] }); }
        setInput('');
    };
    const remove = (s: string) => onChange({ ...agent, skills: agent.skills.filter(x => x !== s) });
    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div style={{ fontSize: 11, color: '#666' }}>Skills describe abstract capabilities (e.g. "summarize", "plan", "critique").</div>
            <div style={{ display: 'flex', gap: 6 }}>
                <input value={input} onChange={e => setInput(e.target.value)} onKeyDown={e => e.key === 'Enter' && add()}
                    placeholder="Add skill..."
                    style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 13 }} />
                <button onClick={add} style={{ padding: '4px 12px', background: '#1a2a3a', border: '1px solid #2a3a4a', color: '#70b0ff', borderRadius: 4, cursor: 'pointer', fontSize: 12 }}>Add</button>
            </div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                {agent.skills.map(s => (
                    <span key={s} style={{ background: '#1a2a3a', color: '#80b0e0', padding: '3px 10px', borderRadius: 12, fontSize: 12, display: 'flex', alignItems: 'center', gap: 6 }}>
                        {s}
                        <button onClick={() => remove(s)} style={{ background: 'none', border: 'none', color: '#a05050', cursor: 'pointer', padding: 0, fontSize: 13, lineHeight: 1 }}>×</button>
                    </span>
                ))}
                {agent.skills.length === 0 && <span style={{ color: '#444', fontSize: 12 }}>No skills added yet.</span>}
            </div>
        </div>
    );
}

function AgentsSection({ agent, onChange }: { agent: AgentModel; onChange: (a: AgentModel) => void }) {
    const [peers, setPeers] = React.useState<AgentIdentity[]>([]);
    React.useEffect(() => { listAgentIdentities().then(setPeers).catch(() => {}); }, []);
    const available = peers.filter(p => p.id !== agent.id);
    const toggle = (id: string) => {
        const next = agent.subAgentIds.includes(id)
            ? agent.subAgentIds.filter(x => x !== id)
            : [...agent.subAgentIds, id];
        onChange({ ...agent, subAgentIds: next });
    };
    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ fontSize: 11, color: '#666', marginBottom: 4 }}>Select sub-agents this agent can delegate to:</div>
            {available.length === 0 && <div style={{ fontSize: 12, color: '#444' }}>No other agents yet.</div>}
            {available.map(a => (
                <label key={a.id} style={{ display: 'flex', alignItems: 'flex-start', gap: 10, cursor: 'pointer', padding: '8px 10px', background: agent.subAgentIds.includes(a.id) ? '#1a1a2a' : '#141414', borderRadius: 4, border: `1px solid ${agent.subAgentIds.includes(a.id) ? '#2a2a4a' : '#222'}` }}>
                    <input type="checkbox" checked={agent.subAgentIds.includes(a.id)} onChange={() => toggle(a.id)} style={{ marginTop: 2 }} />
                    <div>
                        <div style={{ fontSize: 12, fontWeight: 600, color: agent.subAgentIds.includes(a.id) ? '#a070ff' : '#ccc' }}>{a.name}</div>
                        <div style={{ fontSize: 11, color: '#666' }}>{a.description}</div>
                    </div>
                </label>
            ))}
            <div style={{ fontSize: 11, color: '#555', marginTop: 4 }}>{agent.subAgentIds.length} sub-agent(s) wired</div>
        </div>
    );
}

function TokenMeter({ label, value, max, color }: { label: string; value: number; max: number; color: string }) {
    const pct = Math.min(100, (value / max) * 100);
    return (
        <div style={{ marginBottom: 6 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: '#888', marginBottom: 2 }}>
                <span>{label}</span><span>{value.toLocaleString()}</span>
            </div>
            <div style={{ height: 4, background: '#1e1e1e', borderRadius: 2 }}>
                <div style={{ height: '100%', width: `${pct}%`, background: color, borderRadius: 2, transition: 'width 0.3s' }} />
            </div>
        </div>
    );
}

const STEP_COLORS: Record<string, { bg: string; fg: string }> = {
    thought:     { bg: '#1e2a4a', fg: '#7ab4ff' },
    action:      { bg: '#1a2e1a', fg: '#60d060' },
    observation: { bg: '#2e2a10', fg: '#d0a030' },
    result:      { bg: '#2a1a3a', fg: '#c080ff' },
    error:       { bg: '#3a1a1a', fg: '#ff6060' },
};

function SimResultPanel({ result }: { result: SimResult }) {
    const [revealed, setRevealed] = React.useState(0);
    const loopSteps = result.steps.filter(s => s.loopIndex !== undefined && s.loopIndex > 0);
    const loopCount = loopSteps.length > 0 ? Math.max(...loopSteps.map(s => s.loopIndex!)) : 0;
    const modelInfo = MODEL_OPTIONS.find(m => m.value === result.model);

    React.useEffect(() => {
        setRevealed(0);
        const iv = setInterval(() => setRevealed(r => { if (r >= result.steps.length) { clearInterval(iv); return r; } return r + 1; }), 280);
        return () => clearInterval(iv);
    }, [result]);

    return (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: 12 }}>
            <div style={{ fontSize: 12, fontWeight: 700, color: '#ccc', marginBottom: 4 }}>{result.agentName} · {modelInfo?.label ?? result.model}</div>
            {result.steps.slice(0, revealed).map(step => {
                const c = STEP_COLORS[step.type] ?? STEP_COLORS.thought;
                return (
                    <div key={step.sequence} style={{ background: c.bg, border: `1px solid ${c.fg}22`, borderRadius: 4, padding: '7px 10px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                            <span style={{ background: `${c.fg}22`, color: c.fg, padding: '1px 6px', borderRadius: 2, fontSize: 10, fontWeight: 700 }}>{step.type.toUpperCase()}</span>
                            {step.loopIndex !== undefined && step.loopIndex > 0 && <span style={{ background: '#3a1a5a', color: '#c080ff', padding: '1px 6px', borderRadius: 2, fontSize: 10 }}>loop {step.loopIndex}</span>}
                            <span style={{ marginLeft: 'auto', fontSize: 10, color: '#555' }}>in:{step.inputTokens} out:{step.outputTokens}</span>
                        </div>
                        <div style={{ fontSize: 12, color: c.fg, lineHeight: 1.4 }}>{step.content}</div>
                    </div>
                );
            })}
            {revealed >= result.steps.length && (
                <div style={{ background: '#0f1f0f', border: '1px solid #2a4a2a', borderRadius: 4, padding: 10, marginTop: 4 }}>
                    <TokenMeter label="Input tokens" value={result.totalInputTokens} max={result.totalInputTokens + result.totalOutputTokens} color="#4a90d9" />
                    <TokenMeter label="Output tokens" value={result.totalOutputTokens} max={result.totalInputTokens + result.totalOutputTokens} color="#60d060" />
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: '#888', marginTop: 6 }}>
                        <span>Est. cost: <b style={{ color: '#a0d0a0' }}>${result.estimatedCostUsd.toFixed(6)}</b></span>
                        <span>Duration: <b style={{ color: '#a0c0e0' }}>{result.durationMs.toFixed(0)}ms</b></span>
                    </div>
                    {loopCount > 0 && <div style={{ marginTop: 6, fontSize: 11, color: '#a070d0' }}>{loopCount} refinement loop(s) detected</div>}
                </div>
            )}
        </div>
    );
}

function AgentSidebar({ agents, selected, onSelect, onNew }: { agents: AgentModel[]; selected: string; onSelect: (id: string) => void; onNew: () => void }) {
    return (
        <div style={{ width: 160, borderRight: '1px solid #1e1e1e', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            <div style={{ padding: '8px 10px', borderBottom: '1px solid #1e1e1e', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontSize: 11, color: '#888', fontWeight: 600 }}>AGENTS</span>
                <button onClick={onNew} style={{ background: 'none', border: 'none', color: '#60b0ff', fontSize: 16, cursor: 'pointer', lineHeight: 1 }}>+</button>
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {agents.map(a => (
                    <div key={a.id} onClick={() => onSelect(a.id)}
                        style={{ padding: '8px 10px', cursor: 'pointer', borderBottom: '1px solid #1a1a1a', background: selected === a.id ? '#1a2a3a' : 'transparent' }}>
                        <div style={{ fontSize: 12, fontWeight: 600, color: selected === a.id ? '#60b0ff' : '#ccc', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{a.name}</div>
                        <div style={{ fontSize: 10, color: '#555', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{a.model}</div>
                    </div>
                ))}
            </div>
        </div>
    );
}

function identityToModel(a: AgentIdentity): AgentModel {
    return {
        id: a.id, name: a.name, description: a.description,
        model: (a.model as LLMModel) || 'claude-sonnet-4-6',
        apiKey: '', temperature: 0.7, maxTokens: 4096,
        systemPrompt: 'You are a helpful assistant.',
        fewShotExamples: '',
        skills: a.capabilities ?? [],
        tools: [],
        subAgentIds: [],
    };
}

function AgentModelingView() {
    const [agents, setAgents] = React.useState<AgentModel[]>([]);
    const [selectedId, setSelectedId] = React.useState('');
    const [section, setSection] = React.useState<FormSection>('overview');
    const [simResult, setSimResult] = React.useState<SimResult | null>(null);
    const [running, setRunning] = React.useState(false);
    const [loading, setLoading] = React.useState(true);
    const [error, setError] = React.useState('');
    const [task, setTask] = React.useState('Describe what you can do.');
    const wsRef = React.useRef<WebSocket | null>(null);

    // Load agents from backend
    React.useEffect(() => {
        listAgentIdentities()
            .then(list => {
                const models = list.map(identityToModel);
                setAgents(models);
                if (models.length > 0) setSelectedId(models[0].id);
            })
            .catch(() => {
                // fallback to local empty state if backend unreachable
                setError('Backend unreachable — changes will not persist.');
            })
            .finally(() => setLoading(false));
    }, []);

    const agent = agents.find(a => a.id === selectedId);

    const updateAgent = async (updated: AgentModel) => {
        setAgents(ags => ags.map(a => a.id === updated.id ? updated : a));
        updateAgentIdentity(updated.id, {
            name: updated.name, description: updated.description,
            model: updated.model, capabilities: updated.skills,
        }).catch(() => {/* silent — local state already updated */});
    };

    const addNew = async () => {
        try {
            const identity = await createAgentIdentity({ name: 'New Agent', description: '', model: 'claude-sonnet-4-6' });
            const m = identityToModel(identity);
            setAgents(ags => [...ags, m]);
            setSelectedId(m.id);
            setSection('overview');
            setSimResult(null);
        } catch {
            const m = makeDefaultAgent();
            setAgents(ags => [...ags, m]);
            setSelectedId(m.id);
        }
    };

    const removeAgent = async (id: string) => {
        deleteAgentIdentity(id).catch(() => {});
        const remaining = agents.filter(a => a.id !== id);
        setAgents(remaining);
        setSelectedId(remaining[0]?.id ?? '');
        setSimResult(null);
    };

    const runAgent = async () => {
        if (!agent) return;
        setRunning(true);
        setSimResult(null);
        wsRef.current?.close();

        try {
            const req: RunRequest = {
                agentId: agent.id, agentName: agent.name,
                model: agent.model, systemPrompt: agent.systemPrompt,
                task, tools: agent.tools, apiKey: agent.apiKey,
                temperature: agent.temperature, maxTokens: agent.maxTokens,
            };
            const { runId } = await submitRun(req);
            const steps: SimStep[] = [];

            wsRef.current = streamRun(
                runId,
                (step: unknown) => {
                    const s = step as { sequence: number; type: string; content: string; toolName?: string; toolInput?: Record<string, unknown>; inputTokens: number; outputTokens: number };
                    steps.push({
                        sequence: s.sequence, type: s.type as SimStep['type'],
                        content: s.content, toolName: s.toolName, toolInput: s.toolInput,
                        inputTokens: s.inputTokens ?? 0, outputTokens: s.outputTokens ?? 0,
                    });
                    setSimResult({
                        agentId: agent.id, agentName: agent.name, model: agent.model,
                        steps: [...steps],
                        totalInputTokens: steps.reduce((s, st) => s + st.inputTokens, 0),
                        totalOutputTokens: steps.reduce((s, st) => s + st.outputTokens, 0),
                        estimatedCostUsd: 0, durationMs: 0,
                    });
                },
                (run: unknown, err?: string) => {
                    setRunning(false);
                    if (err) setError(err);
                    const r = run as { totalInputTokens?: number; totalOutputTokens?: number; estimatedCostUsd?: number; durationMs?: number } | null;
                    if (r) {
                        setSimResult(prev => prev ? {
                            ...prev,
                            totalInputTokens: r.totalInputTokens ?? prev.totalInputTokens,
                            totalOutputTokens: r.totalOutputTokens ?? prev.totalOutputTokens,
                            estimatedCostUsd: r.estimatedCostUsd ?? 0,
                            durationMs: r.durationMs ?? 0,
                        } : prev);
                    }
                },
            );
        } catch (e) {
            setError(String(e));
            setRunning(false);
        }
    };

    if (loading) {
        return <div style={{ padding: 24, color: '#555', fontSize: 13 }}>Loading agents…</div>;
    }

    if (!agent && agents.length === 0) {
        return (
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', gap: 12, color: '#555' }}>
                {error && <div style={{ color: '#e06060', fontSize: 11, marginBottom: 4 }}>{error}</div>}
                <div style={{ fontSize: 13 }}>No agents yet</div>
                <button onClick={addNew} style={{ padding: '6px 16px', background: '#1e3a1e', border: '1px solid #3a6a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 12 }}>+ New Agent</button>
            </div>
        );
    }

    return (
        <div style={{ display: 'flex', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <AgentSidebar agents={agents} selected={selectedId} onSelect={id => { setSelectedId(id); setSimResult(null); }} onNew={addNew} />
            {agent && (
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                    <SectionTabs active={section} onChange={setSection} />
                    {error && <div style={{ padding: '4px 12px', background: '#2a1010', color: '#e06060', fontSize: 11 }}>{error}</div>}
                    <div style={{ flex: 1, overflow: 'auto' }}>
                        {section === 'overview' && <OverviewSection agent={agent} onChange={updateAgent} />}
                        {section === 'model'    && <ModelSection    agent={agent} onChange={updateAgent} />}
                        {section === 'prompt'   && <PromptSection   agent={agent} onChange={updateAgent} />}
                        {section === 'tools'    && <ToolsSection    agent={agent} onChange={updateAgent} />}
                        {section === 'skills'   && <SkillsSection   agent={agent} onChange={updateAgent} />}
                        {section === 'agents'   && <AgentsSection   agent={agent} onChange={updateAgent} />}
                    </div>
                    <div style={{ borderTop: '1px solid #1e1e1e', padding: '8px 12px', display: 'flex', flexDirection: 'column', gap: 6 }}>
                        <input
                            value={task} onChange={e => setTask(e.target.value)}
                            placeholder="Task for this agent…"
                            style={{ width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '5px 8px', borderRadius: 4, fontSize: 12, boxSizing: 'border-box' }}
                        />
                        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                            <button onClick={runAgent} disabled={running}
                                style={{ padding: '5px 14px', background: running ? '#1a2a1a' : '#1e3a1e', border: '1px solid #3a6a3a', color: running ? '#555' : '#60d060', borderRadius: 4, cursor: running ? 'not-allowed' : 'pointer', fontSize: 12, fontWeight: 600 }}>
                                {running ? '⏳ Running…' : '▶ Run'}
                            </button>
                            <button onClick={() => removeAgent(agent.id)}
                                style={{ padding: '5px 10px', background: 'transparent', border: '1px solid #3a2020', color: '#a04040', borderRadius: 4, cursor: 'pointer', fontSize: 11 }}>
                                Delete
                            </button>
                            <span style={{ fontSize: 11, color: '#444' }}>{agent.tools.length} tool(s) · {agent.skills.length} skill(s)</span>
                        </div>
                    </div>
                </div>
            )}
            {simResult && (
                <div style={{ width: 320, borderLeft: '1px solid #1e1e1e', overflow: 'auto', background: '#0d0d0d' }}>
                    <div style={{ padding: '8px 12px', borderBottom: '1px solid #1e1e1e', fontSize: 11, color: '#888', fontWeight: 600 }}>RUN OUTPUT</div>
                    <SimResultPanel result={simResult} />
                </div>
            )}
        </div>
    );
}

@injectable()
export class AgentsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:agents';
    static readonly LABEL = 'Agents';

    @postConstruct()
    protected init(): void {
        this.id = AgentsPanelWidget.ID;
        this.title.label = AgentsPanelWidget.LABEL;
        this.title.caption = AgentsPanelWidget.LABEL;
        this.title.closable = true;
        this.title.iconClass = 'fa fa-robot';
        this.update();
    }

    protected render(): React.ReactNode {
        return <AgentModelingView />;
    }
}

@injectable()
export class AgentsPanelContribution extends AbstractViewContribution<AgentsPanelWidget> {
    constructor() {
        super({ widgetId: AgentsPanelWidget.ID, widgetName: AgentsPanelWidget.LABEL, defaultWidgetOptions: { area: 'left' }, toggleCommandId: AgentsPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(AgentsPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
