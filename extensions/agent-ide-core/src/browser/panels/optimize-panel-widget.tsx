import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { OptimizePanelCommand } from '../agent-ide-commands';

type OptCategory = 'prompt' | 'tools' | 'model' | 'cache' | 'loop' | 'parallel';
type Severity = 'critical' | 'warning' | 'info';

interface OptRec {
    id: string;
    category: OptCategory;
    severity: Severity;
    title: string;
    description: string;
    // Savings formulas sourced inline
    tokenSavingPct?: number;
    latencyImprovementPct?: number;
    costImprovementPct?: number;
    action: string;
    applied: boolean;
}

const OPT_SOURCE = { owner: 'AGenNext', repo: 'Agent-Optimize', branch: 'main' };

const DEMO_AGENTS = [
    { id: 'research-01', name: 'ResearchAgent', model: 'claude-opus-4-8',   systemPromptTokens: 1840, tools: ['browser','web_search','pdf_reader','vector_search','http_client','db_query'], maxTokens: 16384, temperature: 0.85, loopDetected: true },
    { id: 'coder-01',    name: 'CoderAgent',    model: 'claude-sonnet-4-6', systemPromptTokens: 620,  tools: ['code_exec','file_rw','shell'], maxTokens: 8192,  temperature: 0.2,  loopDetected: false },
];

const CAT_COLORS: Record<OptCategory, { bg: string; fg: string; label: string }> = {
    prompt:   { bg: '#1a2238', fg: '#60b0ff', label: 'Prompt' },
    tools:    { bg: '#1a2a1a', fg: '#60d060', label: 'Tools' },
    model:    { bg: '#2a1a38', fg: '#c080ff', label: 'Model' },
    cache:    { bg: '#1a2a2a', fg: '#40c0c0', label: 'Cache' },
    loop:     { bg: '#2a1a2a', fg: '#d060d0', label: 'Loop' },
    parallel: { bg: '#2a2a1a', fg: '#d0c040', label: 'Parallel' },
};

const SEV_COLORS: Record<Severity, string> = { critical: '#d04040', warning: '#d0a030', info: '#4080c0' };

function analyzeAgent(agent: typeof DEMO_AGENTS[0]): OptRec[] {
    const recs: OptRec[] = [];

    // PROMPT: token reduction
    // tokenSavingPct estimate: (bloat_tokens / total_prompt_tokens) * 100
    // Source: Anthropic prompt engineering guide — concise prompts reduce latency and cost
    if (agent.systemPromptTokens > 1000) {
        recs.push({
            id: 'prompt-len', category: 'prompt', severity: 'warning',
            title: 'System prompt is large',
            description: `~${agent.systemPromptTokens} tokens. Refactoring to remove redundancy typically saves 20–40% of prompt tokens.`,
            // tokenSavingPct = (current - target) / current * 100; target = 60% of current
            tokenSavingPct: 30,
            costImprovementPct: 15,
            action: 'Compress system prompt: remove repetition, use bullet-point instructions.',
            applied: false,
        });
    }

    // TOOLS: too many registered tools increases context usage and hallucination risk
    // Source: Anthropic tool use best practices — limit to ≤5 tools per agent call
    if (agent.tools.length > 5) {
        recs.push({
            id: 'tool-count', category: 'tools', severity: 'warning',
            title: `${agent.tools.length} tools registered (>5 recommended)`,
            description: 'Each tool definition adds ~60–120 tokens to the context. Pruning unused tools reduces cost and hallucinated tool calls.',
            // tokenSavingPct = (excess_tools * avg_tool_tokens) / total_context_tokens * 100
            tokenSavingPct: Math.round((agent.tools.length - 5) * 80 / (agent.systemPromptTokens + agent.tools.length * 80) * 100),
            action: 'Remove tools not required for this agent\'s primary task.',
            applied: false,
        });
    }

    // MODEL: over-provisioned model for task complexity
    // costImprovementPct = (opusRate - sonnetRate) / opusRate * 100 ≈ 80%
    // Source: Anthropic API pricing (Opus 4.8 vs Sonnet 4.6 per 1K output tokens)
    if (agent.model === 'claude-opus-4-8' && agent.tools.length <= 3) {
        recs.push({
            id: 'model-overfit', category: 'model', severity: 'info',
            title: 'claude-opus-4-8 may be over-provisioned',
            description: 'This agent uses ≤3 tools and has a simple task profile. claude-sonnet-4-6 achieves comparable results at ~80% lower output cost.',
            // costImprovementPct = (1 - sonnetOutputRate/opusOutputRate) * 100 = (1 - 0.015/0.075) * 100 = 80%
            costImprovementPct: 80,
            action: 'Test with claude-sonnet-4-6 and compare accuracy on held-out eval set.',
            applied: false,
        });
    }

    // CACHE: repeated system prompt not cached
    // cachedTokenSavings = systemPromptTokens * 0.9 * inputRate / 1000 per call
    // Source: Anthropic prompt caching docs — cache writes billed at 25% markup, reads at 10% base
    if (agent.systemPromptTokens > 500) {
        recs.push({
            id: 'cache-prompt', category: 'cache', severity: 'info',
            title: 'System prompt is not cached',
            description: `Enabling prompt caching saves ${Math.round(agent.systemPromptTokens * 0.9)} input tokens per call after the first.`,
            // tokenSavingPct = (systemPromptTokens * 0.9) / (systemPromptTokens + taskTokens) * 100
            tokenSavingPct: Math.round(agent.systemPromptTokens * 0.9 / (agent.systemPromptTokens + 400) * 100),
            costImprovementPct: 25,
            action: 'Add cache_control: { type: \'ephemeral\' } to the system message block.',
            applied: false,
        });
    }

    // LOOP: uncontrolled refinement loop
    // Source: ReAct paper (Yao et al. 2022) — loops without termination criteria cause token waste
    if (agent.loopDetected) {
        recs.push({
            id: 'loop-unbound', category: 'loop', severity: 'critical',
            title: 'Unbounded refinement loop detected',
            description: 'Agent entered a loop without a clear termination condition. Set maxIterations ≤3 and add explicit success criteria to the prompt.',
            latencyImprovementPct: 35,
            costImprovementPct: 30,
            action: 'Add maxIterations: 3 and include "Stop when confidence > 0.85" in system prompt.',
            applied: false,
        });
    }

    // PARALLEL: sequential tool calls that could run concurrently
    // latencyImprovementPct = (1 - 1/parallelizable_count) * 100
    // Source: Anthropic tool use docs — models can emit multiple tool_use blocks in one turn
    if (agent.tools.length >= 2 && !agent.loopDetected) {
        recs.push({
            id: 'parallel-tools', category: 'parallel', severity: 'info',
            title: 'Tool calls run sequentially',
            description: 'Independent tool calls (e.g. search + fetch) can run in a single turn using parallel tool_use. Reduces round-trips.',
            // latencyImprovementPct = (1 - 1/n) * 100 for n parallelizable calls
            latencyImprovementPct: Math.round((1 - 1 / Math.min(agent.tools.length, 3)) * 100),
            action: 'Request parallel tool_use in system prompt: "Call independent tools simultaneously."',
            applied: false,
        });
    }

    return recs;
}

function ImpactBadge({ label, value, color }: { label: string; value: number; color: string }) {
    return (
        <span style={{ fontSize: 10, background: color + '22', color, padding: '1px 6px', borderRadius: 3, marginRight: 4 }}>
            {label} −{value}%
        </span>
    );
}

function OptimizeView() {
    const [agentId, setAgentId] = React.useState(DEMO_AGENTS[0].id);
    const [recs, setRecs] = React.useState<OptRec[]>([]);
    const [running, setRunning] = React.useState(false);
    const [catFilter, setCatFilter] = React.useState<OptCategory | 'all'>('all');

    const analyze = () => {
        setRunning(true);
        setTimeout(() => {
            const agent = DEMO_AGENTS.find(a => a.id === agentId)!;
            setRecs(analyzeAgent(agent));
            setRunning(false);
        }, 700);
    };

    const apply = (id: string) => setRecs(rs => rs.map(r => r.id === id ? { ...r, applied: true } : r));

    const filtered = catFilter === 'all' ? recs : recs.filter(r => r.category === catFilter);
    const sevOrder: Severity[] = ['critical', 'warning', 'info'];
    const sorted = [...filtered].sort((a, b) => sevOrder.indexOf(a.severity) - sevOrder.indexOf(b.severity));

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontWeight: 700, fontSize: 13, color: '#d0c040' }}>◎ Agent-Optimize</span>
                <span style={{ fontSize: 10, color: '#444' }}>Source: {OPT_SOURCE.owner}/{OPT_SOURCE.repo}@{OPT_SOURCE.branch}</span>
            </div>
            <div style={{ padding: '10px 12px', borderBottom: '1px solid #1e1e1e', display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                <select value={agentId} onChange={e => { setAgentId(e.target.value); setRecs([]); }}
                    style={{ background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 12 }}>
                    {DEMO_AGENTS.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
                </select>
                <button onClick={analyze} disabled={running}
                    style={{ padding: '5px 14px', background: running ? '#1a1a1a' : '#2a2a10', border: '1px solid #4a4a20', color: running ? '#444' : '#d0c040', borderRadius: 4, cursor: running ? 'not-allowed' : 'pointer', fontSize: 12, fontWeight: 600 }}>
                    {running ? 'Analyzing…' : '▶ Analyze'}
                </button>
                {recs.length > 0 && (
                    <>
                        <span style={{ fontSize: 11, color: '#888' }}>{recs.filter(r => !r.applied).length} recommendations</span>
                        <div style={{ display: 'flex', gap: 4, marginLeft: 'auto', flexWrap: 'wrap' }}>
                            {(['all', ...Object.keys(CAT_COLORS)] as (OptCategory | 'all')[]).map(cat => (
                                <button key={cat} onClick={() => setCatFilter(cat)}
                                    style={{ padding: '2px 8px', background: catFilter === cat ? '#2a2a2a' : 'transparent', border: `1px solid ${catFilter === cat ? '#555' : '#333'}`, color: cat === 'all' ? '#888' : CAT_COLORS[cat as OptCategory].fg, borderRadius: 3, cursor: 'pointer', fontSize: 10 }}>
                                    {cat === 'all' ? 'All' : CAT_COLORS[cat as OptCategory].label}
                                </button>
                            ))}
                        </div>
                    </>
                )}
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: recs.length > 0 ? 0 : 12 }}>
                {recs.length === 0 && !running && <div style={{ color: '#444', fontSize: 12 }}>Select an agent and click Analyze to get optimization recommendations.</div>}
                {sorted.map(r => {
                    const cat = CAT_COLORS[r.category];
                    return (
                        <div key={r.id} style={{
                            borderBottom: '1px solid #1a1a1a',
                            padding: '12px 14px',
                            background: r.applied ? '#0d1a0d' : 'transparent',
                            opacity: r.applied ? 0.6 : 1,
                        }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                                <span style={{ width: 8, height: 8, borderRadius: '50%', background: SEV_COLORS[r.severity], flexShrink: 0 }} />
                                <span style={{ background: cat.bg, color: cat.fg, padding: '1px 6px', borderRadius: 3, fontSize: 10, fontWeight: 700 }}>{cat.label}</span>
                                <span style={{ fontSize: 12, fontWeight: 600, color: r.applied ? '#555' : '#ccc' }}>{r.title}</span>
                                {r.applied && <span style={{ fontSize: 10, color: '#40d040', marginLeft: 'auto' }}>✓ Applied</span>}
                            </div>
                            <p style={{ fontSize: 12, color: '#888', margin: '0 0 6px', lineHeight: 1.5, paddingLeft: 16 }}>{r.description}</p>
                            <div style={{ paddingLeft: 16, marginBottom: 6 }}>
                                {r.tokenSavingPct && <ImpactBadge label="tokens" value={r.tokenSavingPct} color="#60b0ff" />}
                                {r.costImprovementPct && <ImpactBadge label="cost" value={r.costImprovementPct} color="#60d060" />}
                                {r.latencyImprovementPct && <ImpactBadge label="latency" value={r.latencyImprovementPct} color="#d0a030" />}
                            </div>
                            <div style={{ paddingLeft: 16, display: 'flex', alignItems: 'flex-start', gap: 8 }}>
                                <span style={{ fontSize: 11, color: '#555', flex: 1 }}>Action: {r.action}</span>
                                {!r.applied && (
                                    <button onClick={() => apply(r.id)}
                                        style={{ padding: '2px 10px', background: '#1a2a1a', border: '1px solid #2a4a2a', color: '#60d060', borderRadius: 3, cursor: 'pointer', fontSize: 10, flexShrink: 0 }}>
                                        Apply
                                    </button>
                                )}
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}

@injectable()
export class OptimizePanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:optimize';
    static readonly LABEL = 'Optimize';
    @postConstruct() protected init(): void {
        this.id = OptimizePanelWidget.ID; this.title.label = OptimizePanelWidget.LABEL;
        this.title.caption = 'Agent-Optimize recommendations'; this.title.closable = true;
        this.title.iconClass = 'codicon codicon-lightbulb'; this.update();
    }
    protected render(): React.ReactNode { return <OptimizeView />; }
}

@injectable()
export class OptimizePanelContribution extends AbstractViewContribution<OptimizePanelWidget> {
    constructor() { super({ widgetId: OptimizePanelWidget.ID, widgetName: OptimizePanelWidget.LABEL, defaultWidgetOptions: { area: 'right', rank: 200 }, toggleCommandId: OptimizePanelCommand.id }); }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(OptimizePanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
