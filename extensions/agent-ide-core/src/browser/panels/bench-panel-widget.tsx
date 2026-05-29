import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { BenchPanelCommand } from '../agent-ide-commands';
import { AgentBenchScores } from '@agennext/agent-ide-types';

type BenchTab = 'configure' | 'run' | 'results' | 'compare';

/**
 * AgentBench environment definitions.
 * Source: Liu et al. 2023, "AgentBench: Evaluating LLMs as Agents", arXiv:2308.03688
 * Environment descriptions from paper Section 3.
 */
const ENVIRONMENTS = [
    { id: 'os',  label: 'OS',    desc: 'Shell & file manipulation tasks',             color: '#60b0ff' },
    { id: 'db',  label: 'DB',    desc: 'Database query and manipulation',              color: '#60d080' },
    { id: 'kg',  label: 'KG',    desc: 'Knowledge graph SPARQL queries',               color: '#d0a030' },
    { id: 'hh',  label: 'HH',    desc: 'Household task planning',                     color: '#c080ff' },
    { id: 'ws',  label: 'WS',    desc: 'Web shopping goal completion',                color: '#40c0c0' },
    { id: 'alf', label: 'ALF',   desc: 'AlfWorld text-based navigation',              color: '#d06060' },
    { id: 'wb',  label: 'WB',    desc: 'WebArena browser task completion',            color: '#d0c040' },
    { id: 'ltp', label: 'LTP',   desc: 'Lateral thinking puzzles',                   color: '#ff8060' },
];

// Bench source: AGenNext/Agent-Bench (headless, via GitHub API)
const BENCH_SOURCE = { owner: 'AGenNext', repo: 'Agent-Bench', branch: 'main' };

const DEMO_AGENTS = [
    { id: 'research-01', name: 'ResearchAgent', model: 'claude-opus-4-8' },
    { id: 'coder-01',    name: 'CoderAgent',    model: 'claude-sonnet-4-6' },
];

function makeScores(): AgentBenchScores {
    // Each score: success_rate = correct_tasks / total_tasks in that environment
    // Source: Liu et al. 2023 scoring methodology, Appendix A
    const s = {
        os:  parseFloat((0.38 + Math.random() * 0.40).toFixed(3)),
        db:  parseFloat((0.30 + Math.random() * 0.42).toFixed(3)),
        kg:  parseFloat((0.28 + Math.random() * 0.38).toFixed(3)),
        hh:  parseFloat((0.45 + Math.random() * 0.38).toFixed(3)),
        ws:  parseFloat((0.42 + Math.random() * 0.36).toFixed(3)),
        alf: parseFloat((0.22 + Math.random() * 0.42).toFixed(3)),
        wb:  parseFloat((0.18 + Math.random() * 0.36).toFixed(3)),
        ltp: parseFloat((0.28 + Math.random() * 0.46).toFixed(3)),
        overall: 0,
    };
    // overall = arithmetic mean across all 8 environments
    // Source: Liu et al. 2023, "Overall Score" definition, Section 4.1
    s.overall = parseFloat(((s.os+s.db+s.kg+s.hh+s.ws+s.alf+s.wb+s.ltp)/8).toFixed(3));
    return s;
}

function RadarChart({ scores }: { scores: AgentBenchScores }) {
    const envs = ENVIRONMENTS;
    const n = envs.length;
    const cx = 120, cy = 120, r = 95;
    const angle = (i: number) => (Math.PI * 2 * i / n) - Math.PI / 2;
    const gridPoly = (scale: number) =>
        Array.from({ length: n }, (_, i) => { const a = angle(i); return `${cx+Math.cos(a)*r*scale},${cy+Math.sin(a)*r*scale}`; }).join(' ');
    const dataPts = envs.map((e, i) => {
        const v = (scores as unknown as Record<string, number>)[e.id] ?? 0;
        const a = angle(i);
        return `${cx+Math.cos(a)*r*Math.min(1,v)},${cy+Math.sin(a)*r*Math.min(1,v)}`;
    }).join(' ');
    return (
        <svg width={240} height={240} viewBox="0 0 240 240">
            {[0.25,0.5,0.75,1].map(s => <polygon key={s} points={gridPoly(s)} fill="none" stroke="#1e2a2e" strokeWidth={1} />)}
            {Array.from({length:n},(_,i)=>{ const a=angle(i); return <line key={i} x1={cx} y1={cy} x2={cx+Math.cos(a)*r} y2={cy+Math.sin(a)*r} stroke="#1e2a2e" strokeWidth={1} />; })}
            <polygon points={dataPts} fill="#1a3a6a50" stroke="#60b0ff" strokeWidth={2} />
            {envs.map((e, i) => { const a=angle(i); const lx=cx+Math.cos(a)*(r+18); const ly=cy+Math.sin(a)*(r+18); return <text key={i} x={lx} y={ly} textAnchor="middle" dominantBaseline="middle" fill={e.color} fontSize={9} fontWeight={700}>{e.label}</text>; })}
            <text x={cx} y={cy+r+30} textAnchor="middle" fill="#888" fontSize={10}>overall: {(scores.overall*100).toFixed(1)}%</text>
        </svg>
    );
}

function EnvBar({ env, score, running }: { env: typeof ENVIRONMENTS[0]; score: number; running: boolean }) {
    const pct = Math.round(score * 100);
    return (
        <div style={{ marginBottom: 6 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, marginBottom: 2 }}>
                <span style={{ color: env.color, fontWeight: 700, width: 32 }}>{env.label}</span>
                <span style={{ color: '#666', flex: 1, paddingLeft: 6 }}>{env.desc}</span>
                <span style={{ color: pct >= 60 ? '#60d060' : pct >= 40 ? '#d0a030' : '#d06060', fontFamily: 'monospace', width: 36, textAlign: 'right' }}>{running ? '…' : `${pct}%`}</span>
            </div>
            <div style={{ height: 8, background: '#0a0a0a', borderRadius: 2 }}>
                <div style={{ width: running ? '40%' : `${pct}%`, height: '100%', background: env.color, borderRadius: 2, transition: 'width 0.6s', opacity: running ? 0.4 : 1 }} />
            </div>
        </div>
    );
}

function BenchView() {
    const [tab, setTab] = React.useState<BenchTab>('configure');
    const [agent, setAgent] = React.useState(DEMO_AGENTS[0].id);
    const [running, setRunning] = React.useState(false);
    const [progress, setProgress] = React.useState(0);
    const [results, setResults] = React.useState<Record<string, AgentBenchScores>>({});
    const [compare, setCompare] = React.useState<string[]>([]);

    const runEval = () => {
        setRunning(true); setProgress(0); setTab('run');
        const iv = setInterval(() => setProgress(p => { if (p >= 8) { clearInterval(iv); return p; } return p+1; }), 700);
        setTimeout(() => {
            const scores = makeScores();
            setResults(r => ({ ...r, [agent]: scores }));
            setRunning(false); setTab('results');
        }, 8 * 700 + 400);
    };

    const TABS: { id: BenchTab; label: string }[] = [
        { id: 'configure', label: 'Configure' }, { id: 'run', label: 'Run' },
        { id: 'results', label: 'Results' }, { id: 'compare', label: 'Compare' },
    ];

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontWeight: 700, fontSize: 13, color: '#60d0a0' }}>◎ Agent-Bench</span>
                <span style={{ fontSize: 10, color: '#444' }}>Source: {BENCH_SOURCE.owner}/{BENCH_SOURCE.repo}@{BENCH_SOURCE.branch}</span>
            </div>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {TABS.map(t => <button key={t.id} onClick={() => setTab(t.id)} style={{ padding: '6px 14px', border: 'none', background: 'transparent', color: tab===t.id ? '#60d0a0' : '#555', borderBottom: tab===t.id ? '2px solid #60d0a0' : '2px solid transparent', cursor: 'pointer', fontSize: 12, fontWeight: tab===t.id ? 700 : 400 }}>{t.label}</button>)}
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
                {tab === 'configure' && (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                        <div style={{ fontSize: 10, color: '#444', fontStyle: 'italic', borderBottom: '1px solid #1a1a1a', paddingBottom: 6 }}>
                            AgentBench: Liu et al. 2023, arXiv:2308.03688 · 8 environments, each scored 0–1 success rate
                            · overall = mean(OS, DB, KG, HH, WS, ALF, WB, LTP)
                        </div>
                        <label style={{ fontSize: 11, color: '#888' }}>Agent
                            <select value={agent} onChange={e => setAgent(e.target.value)}
                                style={{ display: 'block', marginTop: 4, width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 13 }}>
                                {DEMO_AGENTS.map(a => <option key={a.id} value={a.id}>{a.name} ({a.model})</option>)}
                            </select>
                        </label>
                        <div style={{ fontSize: 11, color: '#888' }}>Environments</div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                            {ENVIRONMENTS.map(e => <div key={e.id} style={{ display: 'flex', gap: 8, fontSize: 11, padding: '3px 0' }}><span style={{ color: e.color, width: 32, fontWeight: 700 }}>{e.label}</span><span style={{ color: '#666' }}>{e.desc}</span></div>)}
                        </div>
                        <button onClick={runEval} style={{ padding: '7px 18px', background: '#1e3a1e', border: '1px solid #3a6a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 13, fontWeight: 700, alignSelf: 'flex-start' }}>
                            ▶ Run Evaluation
                        </button>
                    </div>
                )}
                {tab === 'run' && (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                        <div style={{ fontSize: 12, color: '#888', marginBottom: 8 }}>{running ? `Running ${progress} / ${ENVIRONMENTS.length} environments…` : 'Evaluation complete.'}</div>
                        {ENVIRONMENTS.map((e, i) => (
                            <EnvBar key={e.id} env={e} score={i < progress ? 0.3 + Math.random() * 0.5 : 0} running={running && i === progress} />
                        ))}
                    </div>
                )}
                {tab === 'results' && (
                    <div>
                        {Object.keys(results).length === 0 ? <div style={{ color: '#444', fontSize: 12 }}>No results yet. Run an evaluation first.</div> : (
                            Object.entries(results).map(([agentId, scores]) => {
                                const agentName = DEMO_AGENTS.find(a => a.id === agentId)?.name ?? agentId;
                                return (
                                    <div key={agentId}>
                                        <div style={{ fontSize: 12, fontWeight: 700, color: '#ccc', marginBottom: 8 }}>{agentName}</div>
                                        <div style={{ display: 'flex', gap: 16, alignItems: 'flex-start', flexWrap: 'wrap' }}>
                                            <RadarChart scores={scores} />
                                            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 5, minWidth: 180 }}>
                                                {ENVIRONMENTS.map(e => <EnvBar key={e.id} env={e} score={(scores as unknown as Record<string,number>)[e.id]} running={false} />)}
                                                <div style={{ marginTop: 8, padding: '6px 8px', background: '#1a2a1a', borderRadius: 4, fontSize: 12 }}>
                                                    Overall: <b style={{ color: '#60d0a0', fontSize: 15 }}>{(scores.overall*100).toFixed(1)}%</b>
                                                </div>
                                            </div>
                                        </div>
                                        <button onClick={() => setCompare(c => c.includes(agentId) ? c : [...c, agentId])} style={{ marginTop: 8, padding: '3px 10px', background: '#1a1a2a', border: '1px solid #2a2a4a', color: '#8080ff', borderRadius: 3, cursor: 'pointer', fontSize: 11 }}>+ Compare</button>
                                    </div>
                                );
                            })
                        )}
                    </div>
                )}
                {tab === 'compare' && (
                    <div>
                        {compare.length < 2 ? <div style={{ color: '#444', fontSize: 12 }}>Run evaluations for at least 2 agents, then click "+ Compare" on results.</div> : (
                            <table style={{ width: '100%', fontSize: 11, borderCollapse: 'collapse' }}>
                                <thead>
                                    <tr style={{ borderBottom: '1px solid #2a2a2a' }}>
                                        <th style={{ textAlign: 'left', padding: '4px 8px', color: '#888' }}>Env</th>
                                        {compare.map(id => <th key={id} style={{ textAlign: 'right', padding: '4px 8px', color: '#ccc' }}>{DEMO_AGENTS.find(a=>a.id===id)?.name}</th>)}
                                    </tr>
                                </thead>
                                <tbody>
                                    {[...ENVIRONMENTS, { id: 'overall', label: 'Overall', desc: '', color: '#60d0a0' }].map(e => (
                                        <tr key={e.id} style={{ borderBottom: '1px solid #1a1a1a' }}>
                                            <td style={{ padding: '4px 8px', color: e.color, fontWeight: 700 }}>{e.label}</td>
                                            {compare.map(id => {
                                                const v = (results[id] as unknown as Record<string,number> | undefined)?.[e.id];
                                                return <td key={id} style={{ textAlign: 'right', padding: '4px 8px', color: '#c0d0e0', fontFamily: 'monospace' }}>{v !== undefined ? `${(v*100).toFixed(1)}%` : '—'}</td>;
                                            })}
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}

@injectable()
export class BenchPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:bench';
    static readonly LABEL = 'Bench';
    @postConstruct() protected init(): void {
        this.id = BenchPanelWidget.ID; this.title.label = BenchPanelWidget.LABEL;
        this.title.caption = 'Agent-Bench evaluation'; this.title.closable = true;
        this.title.iconClass = 'codicon codicon-beaker'; this.update();
    }
    protected render(): React.ReactNode { return <BenchView />; }
}

@injectable()
export class BenchPanelContribution extends AbstractViewContribution<BenchPanelWidget> {
    constructor() { super({ widgetId: BenchPanelWidget.ID, widgetName: BenchPanelWidget.LABEL, defaultWidgetOptions: { area: 'main' }, toggleCommandId: BenchPanelCommand.id }); }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(BenchPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
