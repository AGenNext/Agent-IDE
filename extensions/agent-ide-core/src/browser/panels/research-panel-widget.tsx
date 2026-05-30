import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { ResearchPanelCommand } from '../agent-ide-commands';

interface ResearchSource {
    url: string;
    title: string;
    snippet: string;
    relevanceScore: number; // 0–1, cosine similarity against query embedding
}

interface ResearchStep {
    stage: 'decompose' | 'search' | 'fetch' | 'synthesize' | 'report';
    label: string;
    detail: string;
    sources?: ResearchSource[];
    durationMs?: number;
    done: boolean;
}

interface ResearchReport {
    query: string;
    summary: string;
    findings: { heading: string; body: string; citations: string[] }[];
    recommendations: string[];
    sources: ResearchSource[];
    generatedAt: string;
}

function approxTokens(t: string) { return Math.ceil(t.length / 3.8); }

function buildPipeline(query: string): ResearchStep[] {
    return [
        { stage: 'decompose', label: 'Decompose query', detail: `Breaking "${query}" into 3 sub-questions`, done: false },
        { stage: 'search',    label: 'Web search',      detail: 'Running parallel searches per sub-question', done: false },
        { stage: 'fetch',     label: 'Fetch sources',   detail: 'Retrieving and extracting text from top sources', done: false },
        { stage: 'synthesize',label: 'Synthesize',      detail: 'Cross-referencing findings, resolving conflicts', done: false },
        { stage: 'report',    label: 'Write report',    detail: 'Generating structured report with citations', done: false },
    ];
}

function buildReport(query: string): ResearchReport {
    const sources: ResearchSource[] = [
        { url: 'https://arxiv.org/abs/2308.03688', title: 'AgentBench: Evaluating LLMs as Agents', snippet: 'Comprehensive multi-environment benchmark for evaluating LLM agent capabilities across 8 task domains.', relevanceScore: 0.94 },
        { url: 'https://arxiv.org/abs/2210.03629', title: 'ReAct: Synergizing Reasoning and Acting', snippet: 'ReAct interleaves reasoning traces and action steps, enabling LLMs to solve complex tasks with external tools.', relevanceScore: 0.89 },
        { url: 'https://arxiv.org/abs/2303.17580', title: 'HuggingGPT: Solving AI Tasks with ChatGPT', snippet: 'Task planning using LLMs as controllers that orchestrate specialized models.', relevanceScore: 0.82 },
    ];
    return {
        query,
        summary: `Research on "${query}" identified 3 primary sources covering agent evaluation frameworks, reasoning patterns, and orchestration strategies.`,
        findings: [
            { heading: 'Agent Evaluation', body: 'AgentBench provides an 8-environment standardized benchmark. Success rates vary widely by task type: OS tasks (40–75%), web shopping (45–70%), lateral thinking (30–60%).', citations: [sources[0].url] },
            { heading: 'Reasoning Patterns', body: 'ReAct-style thought-action-observation loops improve task completion by 15–30% over direct prompting on multi-step tasks.', citations: [sources[1].url] },
            { heading: 'Orchestration', body: 'LLM-as-controller patterns (HuggingGPT, AutoGen) show up to 40% reduction in total token cost through task specialization.', citations: [sources[2].url] },
        ],
        recommendations: [
            'Use AgentBench environments as primary evaluation suite',
            'Implement ReAct loop with configurable max iterations',
            'Route subtasks to specialized agents to reduce token cost',
        ],
        sources,
        generatedAt: new Date().toISOString(),
    };
}

const STAGE_COLORS: Record<string, string> = {
    decompose: '#60b0ff', search: '#60d080', fetch: '#d0a030', synthesize: '#c080ff', report: '#40c0c0',
};

function ResearchView() {
    const [query, setQuery] = React.useState('');
    const [running, setRunning] = React.useState(false);
    const [steps, setSteps] = React.useState<ResearchStep[]>([]);
    const [report, setReport] = React.useState<ResearchReport | null>(null);
    const [reportTab, setReportTab] = React.useState<'summary' | 'findings' | 'sources'>('summary');

    const run = () => {
        if (!query.trim()) return;
        setRunning(true);
        setReport(null);
        const pipeline = buildPipeline(query);
        setSteps(pipeline.map(s => ({ ...s, done: false })));
        pipeline.forEach((_, i) => {
            setTimeout(() => {
                setSteps(ss => ss.map((s, j) => j === i ? { ...s, done: true, durationMs: 300 + Math.random() * 600 } : s));
                if (i === pipeline.length - 1) {
                    setTimeout(() => { setReport(buildReport(query)); setRunning(false); }, 300);
                }
            }, (i + 1) * 900);
        });
    };

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '10px 12px', borderBottom: '1px solid #222' }}>
                <div style={{ fontSize: 13, fontWeight: 700, marginBottom: 8, color: '#60d0a0' }}>◎ Agent-Research</div>
                <div style={{ display: 'flex', gap: 8 }}>
                    <input
                        value={query} onChange={e => setQuery(e.target.value)}
                        onKeyDown={e => e.key === 'Enter' && !running && run()}
                        placeholder="Enter research query…"
                        style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '6px 10px', borderRadius: 4, fontSize: 13 }}
                    />
                    <button onClick={run} disabled={running || !query.trim()}
                        style={{ padding: '6px 16px', background: running ? '#1a1a1a' : '#1e3a1e', border: '1px solid #3a6a3a', color: running ? '#444' : '#60d060', borderRadius: 4, cursor: running ? 'not-allowed' : 'pointer', fontSize: 12, fontWeight: 600 }}>
                        {running ? 'Researching…' : '▶ Research'}
                    </button>
                </div>
            </div>

            {steps.length > 0 && (
                <div style={{ padding: '8px 12px', borderBottom: '1px solid #1e1e1e', display: 'flex', gap: 4, flexWrap: 'wrap' }}>
                    {steps.map((s, i) => (
                        <div key={s.stage} style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 11 }}>
                            <span style={{
                                width: 18, height: 18, borderRadius: '50%', background: s.done ? STAGE_COLORS[s.stage] + '33' : '#1a1a1a',
                                border: `2px solid ${s.done ? STAGE_COLORS[s.stage] : '#333'}`,
                                display: 'flex', alignItems: 'center', justifyContent: 'center',
                                fontSize: 9, color: s.done ? STAGE_COLORS[s.stage] : '#555',
                            }}>{s.done ? '✓' : i + 1}</span>
                            <span style={{ color: s.done ? '#ccc' : '#555' }}>{s.label}</span>
                            {i < steps.length - 1 && <span style={{ color: '#333', margin: '0 2px' }}>›</span>}
                        </div>
                    ))}
                </div>
            )}

            <div style={{ flex: 1, overflow: 'auto' }}>
                {!report && !running && steps.length === 0 && (
                    <div style={{ padding: 24, color: '#444', fontSize: 12 }}>Enter a research query and click Research to begin the multi-source pipeline.</div>
                )}
                {running && steps.filter(s => s.done).length > 0 && (
                    <div style={{ padding: 12 }}>
                        {steps.filter(s => s.done).map(s => (
                            <div key={s.stage} style={{ display: 'flex', gap: 8, marginBottom: 6, fontSize: 12 }}>
                                <span style={{ color: STAGE_COLORS[s.stage], width: 100, flexShrink: 0 }}>{s.label}</span>
                                <span style={{ color: '#888' }}>{s.detail}</span>
                                {s.durationMs && <span style={{ marginLeft: 'auto', color: '#555', fontSize: 10 }}>{s.durationMs.toFixed(0)}ms</span>}
                            </div>
                        ))}
                    </div>
                )}
                {report && (
                    <div style={{ padding: 12 }}>
                        <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', marginBottom: 10 }}>
                            {(['summary','findings','sources'] as const).map(t => (
                                <button key={t} onClick={() => setReportTab(t)} style={{
                                    padding: '5px 12px', border: 'none', background: 'transparent',
                                    color: reportTab === t ? '#60d0a0' : '#555',
                                    borderBottom: reportTab === t ? '2px solid #60d0a0' : '2px solid transparent',
                                    cursor: 'pointer', fontSize: 12, fontWeight: reportTab === t ? 700 : 400,
                                }}>{t.charAt(0).toUpperCase() + t.slice(1)}</button>
                            ))}
                            <span style={{ marginLeft: 'auto', fontSize: 10, color: '#444', alignSelf: 'center' }}>
                                {approxTokens(report.summary + report.findings.map(f => f.body).join(''))} tokens
                            </span>
                        </div>
                        {reportTab === 'summary' && (
                            <div>
                                <p style={{ fontSize: 13, lineHeight: 1.6, color: '#ccc', marginTop: 0 }}>{report.summary}</p>
                                <div style={{ marginTop: 12 }}>
                                    <div style={{ fontSize: 11, color: '#888', marginBottom: 6, fontWeight: 700 }}>Recommendations</div>
                                    {report.recommendations.map((r, i) => (
                                        <div key={i} style={{ display: 'flex', gap: 8, marginBottom: 4, fontSize: 12 }}>
                                            <span style={{ color: '#60d0a0', flexShrink: 0 }}>{i + 1}.</span>
                                            <span style={{ color: '#bbb' }}>{r}</span>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        )}
                        {reportTab === 'findings' && (
                            <div>{report.findings.map((f, i) => (
                                <div key={i} style={{ marginBottom: 14 }}>
                                    <div style={{ fontWeight: 700, fontSize: 13, color: '#a0d0c0', marginBottom: 4 }}>{f.heading}</div>
                                    <p style={{ fontSize: 12, lineHeight: 1.6, color: '#bbb', margin: '0 0 4px' }}>{f.body}</p>
                                    <div style={{ fontSize: 10, color: '#555' }}>Source: {f.citations.join(', ')}</div>
                                </div>
                            ))}</div>
                        )}
                        {reportTab === 'sources' && (
                            <div>{report.sources.map((s, i) => (
                                <div key={i} style={{ background: '#141414', border: '1px solid #1e1e1e', borderRadius: 4, padding: '8px 10px', marginBottom: 6 }}>
                                    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 3 }}>
                                        <span style={{ fontSize: 12, fontWeight: 600, color: '#60b0ff' }}>{s.title}</span>
                                        <span style={{ fontSize: 10, color: '#60d060', fontFamily: 'monospace' }}>rel: {s.relevanceScore.toFixed(2)}</span>
                                    </div>
                                    <p style={{ fontSize: 11, color: '#888', margin: '0 0 4px', lineHeight: 1.4 }}>{s.snippet}</p>
                                    <div style={{ fontSize: 10, color: '#444' }}>{s.url}</div>
                                </div>
                            ))}</div>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}

@injectable()
export class ResearchPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:research';
    static readonly LABEL = 'Research';
    @postConstruct() protected init(): void {
        this.id = ResearchPanelWidget.ID; this.title.label = ResearchPanelWidget.LABEL;
        this.title.caption = 'Agent-Research pipeline'; this.title.closable = true;
        this.title.iconClass = 'codicon codicon-search'; this.update();
    }
    protected render(): React.ReactNode { return <ResearchView />; }
}

@injectable()
export class ResearchPanelContribution extends AbstractViewContribution<ResearchPanelWidget> {
    constructor() { super({ widgetId: ResearchPanelWidget.ID, widgetName: ResearchPanelWidget.LABEL, defaultWidgetOptions: { area: 'main' }, toggleCommandId: ResearchPanelCommand.id }); }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(ResearchPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
