import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { PlatformPanelCommand } from '../agent-ide-commands';
import { FinOpsMetrics, MonitorSnapshot, TraceRecord, RepoSource } from '@agennext/agent-ide-types';

type PlatformTab = 'overview' | 'finops' | 'traces' | 'monitor' | 'sources';

// ─── Demo data generators ─────────────────────────────────────────────────────────

const DEMO_AGENTS = [
    { id: 'research-01', name: 'ResearchAgent', model: 'claude-opus-4-8',   status: 'idle',    tools: 3, lastRun: '2m ago' },
    { id: 'coder-01',    name: 'CoderAgent',    model: 'claude-sonnet-4-6', status: 'running', tools: 4, lastRun: 'now' },
    { id: 'analyst-01', name: 'AnalystAgent',  model: 'gpt-4o',            status: 'idle',    tools: 2, lastRun: '18m ago' },
    { id: 'writer-01',  name: 'WriterAgent',   model: 'claude-haiku-4-5',  status: 'error',   tools: 1, lastRun: '1h ago' },
];

const DEMO_EVENTS = [
    { type: 'completed', agent: 'ResearchAgent', msg: 'Task research-042 completed (8.3s)', ts: '14:22:01' },
    { type: 'tool_call', agent: 'CoderAgent',    msg: 'code_exec called — step 3',          ts: '14:22:04' },
    { type: 'error',     agent: 'WriterAgent',   msg: 'Tool http_client timeout (30s)',     ts: '14:21:44' },
    { type: 'thought',   agent: 'CoderAgent',    msg: 'Planning implementation strategy',   ts: '14:22:03' },
    { type: 'stage_change', agent: 'CoderAgent', msg: 'Stage: thinking → acting',          ts: '14:22:04' },
];

// FinOps calculation
// totalInputCost  = inputTokens / 1000 * inputRate
// totalOutputCost = outputTokens / 1000 * outputRate
// Source: Anthropic API pricing docs; OpenAI API pricing page
function makeFinOps(): FinOpsMetrics {
    const inputTokens = 142800, outputTokens = 38400, cachedTokens = 42000;
    const inputRate = 0.003, outputRate = 0.015; // claude-sonnet-4-6 per 1K
    const totalIn  = inputTokens  / 1000 * inputRate;
    const totalOut = outputTokens / 1000 * outputRate;
    const total    = totalIn + totalOut;
    const budget   = 50.0;
    // cachedTokenSavings: cached reads billed at 10% of base input rate
    // Source: Anthropic prompt caching documentation
    const cacheSavings = cachedTokens / 1000 * inputRate * 0.9;
    // projectedMonthlyUsd = totalCost * (30 * 86400 * 1000 / windowMs)
    const windowMs = 3600000; // 1 hour window
    const projected = total * (30 * 86400 * 1000 / windowMs);
    return {
        inputCostPer1KTokens: inputRate, outputCostPer1KTokens: outputRate,
        totalInputCost: totalIn, totalOutputCost: totalOut, totalCost: total,
        budgetUsd: budget, budgetRemaining: budget - total,
        // costPerTask: totalCost / completedTasks
        costPerTask: total / 24,
        // tokenEfficiencyRatio = outputTokens / inputTokens
        tokenEfficiencyRatio: outputTokens / inputTokens,
        cachedTokenSavings: cacheSavings,
        projectedMonthlyUsd: projected,
        windowMs,
    };
}

// errorRatePct = failedRuns / totalRuns * 100
// throughputPerMin = completedTasks / (windowMs / 60000)
// p99LatencyMs = 99th-percentile run duration
// Source: Google SRE Book Ch.6 — Monitoring Distributed Systems
function makeMonitor(agentId: string): MonitorSnapshot {
    const isErr = agentId === 'writer-01';
    return {
        timestamp: new Date().toISOString(), agentId,
        health: isErr ? 'degraded' : 'healthy',
        uptimePct:      isErr ? 94.2 : 99.8,
        errorRatePct:   isErr ? 12.4 : 0.8,
        throughputPerMin: isErr ? 1.2 : 4.8 + Math.random(),
        queueDepth:     Math.floor(Math.random() * 8),
        activeRuns:     agentId === 'coder-01' ? 1 : 0,
        avgLatencyMs:   1200 + Math.random() * 400,
        p99LatencyMs:   3800 + Math.random() * 800,
        memoryUsageMb:  280 + Math.random() * 80,
        cpuUsagePct:    agentId === 'coder-01' ? 42 : 4 + Math.random() * 8,
    };
}

function makeTraces(): TraceRecord[] {
    return [1, 2, 3].map(i => ({
        traceId: `trace-${i.toString().padStart(4,'0')}`,
        rootSpan: {
            spanId: `span-root-${i}`, traceId: `trace-${i}`,
            operationName: ['research.execute','code.generate','analysis.run'][i-1],
            serviceName: 'agent-runtime',
            startTimeMs: Date.now() - i * 60000, endTimeMs: Date.now() - i * 60000 + 3200 + i * 800,
            durationMs: 3200 + i * 800, status: i === 2 ? 'error' : 'ok',
            attributes: { agentId: DEMO_AGENTS[i-1].id, model: DEMO_AGENTS[i-1].model }, events: [],
        },
        spans: Array.from({ length: 3 + i }, (_, j) => ({
            spanId: `span-${i}-${j}`, traceId: `trace-${i}`,
            parentSpanId: `span-root-${i}`,
            operationName: ['tool.call','llm.complete','memory.fetch'][j % 3],
            serviceName: ['tool-executor','llm-client','memory-store'][j % 3],
            startTimeMs: Date.now() - i * 60000 + j * 400,
            endTimeMs: Date.now() - i * 60000 + j * 400 + 300 + j * 100,
            durationMs: 300 + j * 100, status: (i === 2 && j === 1) ? 'error' : 'ok',
            attributes: { step: j + 1 }, events: [],
        })),
        totalDurationMs: 3200 + i * 800,
        spanCount: 4 + i, errorCount: i === 2 ? 1 : 0,
        depth: 2, p99LatencyMs: 3600 + i * 600,
    }));
}

const DEMO_SOURCES: RepoSource[] = [
    {
        id: 'src-bench', owner: 'AGenNext', repo: 'Agent-Bench', branch: 'main',
        type: 'agent-bench', manifestPath: 'agent-ide-manifest.json',
        syncStatus: 'synced', lastSyncAt: '2025-05-29T14:00:00Z',
        imported: { agents: 0, benchSuites: 8, tools: 0, tasks: 120 },
    },
    {
        id: 'src-opt', owner: 'AGenNext', repo: 'Agent-Optimize', branch: 'main',
        type: 'agent-lib', manifestPath: 'agent-ide-manifest.json',
        syncStatus: 'synced', lastSyncAt: '2025-05-29T13:45:00Z',
        imported: { agents: 2, benchSuites: 0, tools: 4, tasks: 0 },
    },
    {
        id: 'src-ide', owner: 'AGenNext', repo: 'Agent-IDE', branch: 'feature/theia-agent-ide-foundation',
        type: 'custom', manifestPath: 'agent-ide-manifest.json',
        syncStatus: 'idle', imported: { agents: 0, benchSuites: 0, tools: 0, tasks: 0 },
    },
];

// ─── Sub-components ───────────────────────────────────────────────────────────

function MetricCard({ label, value, unit, color, note }: { label: string; value: string | number; unit?: string; color?: string; note?: string }) {
    return (
        <div style={{ background: '#141414', border: '1px solid #222', borderRadius: 5, padding: '10px 12px', minWidth: 130 }}>
            <div style={{ fontSize: 10, color: '#666', marginBottom: 4 }}>{label}</div>
            <div style={{ fontSize: 18, fontWeight: 700, color: color ?? '#c0d0e0', fontFamily: 'monospace' }}>
                {value}{unit && <span style={{ fontSize: 11, color: '#555', marginLeft: 2 }}>{unit}</span>}
            </div>
            {note && <div style={{ fontSize: 9, color: '#444', marginTop: 3, fontStyle: 'italic' }}>{note}</div>}
        </div>
    );
}

function StatusDot({ status }: { status: string }) {
    const c = { running: '#40d040', idle: '#4080c0', error: '#d04040', healthy: '#40d040', degraded: '#d0a030', unhealthy: '#d04040' }[status] ?? '#606060';
    return <span style={{ width: 7, height: 7, borderRadius: '50%', background: c, display: 'inline-block', flexShrink: 0 }} />;
}

function OverviewTab() {
    const [events, setEvents] = React.useState(DEMO_EVENTS);
    React.useEffect(() => {
        const iv = setInterval(() => {
            const types = ['thought','tool_call','stage_change','completed'];
            const agents = DEMO_AGENTS.map(a => a.name);
            const msgs = ['Processing step','Tool invoked','Stage transition','Subtask done'];
            const idx = Math.floor(Math.random() * 4);
            setEvents(ev => [{ type: types[idx], agent: agents[idx % agents.length], msg: msgs[idx], ts: new Date().toLocaleTimeString() }, ...ev.slice(0, 49)]);
        }, 2800);
        return () => clearInterval(iv);
    }, []);
    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                <MetricCard label="Registered Agents" value={DEMO_AGENTS.length} color="#60b0ff" />
                <MetricCard label="Active Runs" value={1} color="#60d060" />
                <MetricCard label="Queue Depth" value={3} color="#d0a030" />
                <MetricCard label="Events (1h)" value={248} color="#c080ff" />
            </div>
            <div style={{ fontSize: 11, color: '#888', fontWeight: 700, marginTop: 4 }}>Agent Registry</div>
            {DEMO_AGENTS.map(a => (
                <div key={a.id} style={{ background: '#141414', border: '1px solid #1e1e1e', borderRadius: 4, padding: '7px 10px', display: 'flex', alignItems: 'center', gap: 8 }}>
                    <StatusDot status={a.status} />
                    <span style={{ fontSize: 12, fontWeight: 600, flex: 1 }}>{a.name}</span>
                    <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>{a.model}</span>
                    <span style={{ fontSize: 10, color: '#555' }}>{a.tools} tools</span>
                    <span style={{ fontSize: 10, color: '#444' }}>{a.lastRun}</span>
                </div>
            ))}
            <div style={{ fontSize: 11, color: '#888', fontWeight: 700, marginTop: 4 }}>Live Events</div>
            <div style={{ maxHeight: 200, overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 2 }}>
                {events.map((e, i) => {
                    const c = { completed: '#40d040', error: '#d04040', tool_call: '#d0a030', thought: '#60b0ff', stage_change: '#c080ff' }[e.type] ?? '#888';
                    return (
                        <div key={i} style={{ display: 'flex', gap: 8, fontSize: 11, padding: '2px 0', borderBottom: '1px solid #161616' }}>
                            <span style={{ color: '#444', fontFamily: 'monospace', flexShrink: 0 }}>{e.ts}</span>
                            <span style={{ color: c, flexShrink: 0, width: 90, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{e.type}</span>
                            <span style={{ color: '#70a0d0', flexShrink: 0, width: 100, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{e.agent}</span>
                            <span style={{ color: '#888' }}>{e.msg}</span>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}

function FinOpsTab() {
    const m = makeFinOps();
    const budgetUsedPct = ((m.budgetUsd - m.budgetRemaining) / m.budgetUsd * 100);
    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div style={{ fontSize: 10, color: '#444', fontStyle: 'italic', borderBottom: '1px solid #1a1a1a', paddingBottom: 6 }}>
                Cost formula: totalCost = (inputTokens/1K)*inputRate + (outputTokens/1K)*outputRate
                · Source: Anthropic API pricing; OpenAI API pricing
            </div>
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                <MetricCard label="Total Cost (1h)" value={`$${m.totalCost.toFixed(4)}`} color="#d06060" note="inputCost + outputCost" />
                <MetricCard label="Budget Remaining" value={`$${m.budgetRemaining.toFixed(2)}`} color={m.budgetRemaining < 10 ? '#d06060' : '#60d060'} />
                <MetricCard label="Cost per Task" value={`$${m.costPerTask.toFixed(5)}`} color="#d0a030" note="totalCost / completedTasks" />
                <MetricCard label="Projected/Month" value={`$${m.projectedMonthlyUsd.toFixed(2)}`} color="#c080ff" note="totalCost * (30d / window)" />
            </div>
            <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: '#888', marginBottom: 3 }}>
                    <span>Budget used: {budgetUsedPct.toFixed(1)}%</span><span>${m.budgetUsd}</span>
                </div>
                <div style={{ height: 8, background: '#1a1a1a', borderRadius: 4 }}>
                    <div style={{ width: `${budgetUsedPct}%`, height: '100%', background: budgetUsedPct > 80 ? '#d06060' : '#4a90d9', borderRadius: 4 }} />
                </div>
            </div>
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                <MetricCard label="Input tokens" value={(142800).toLocaleString()} unit="tok" color="#60b0ff" note={`$${m.inputCostPer1KTokens}/1K`} />
                <MetricCard label="Output tokens" value={(38400).toLocaleString()} unit="tok" color="#60d060" note={`$${m.outputCostPer1KTokens}/1K`} />
                <MetricCard label="Cache savings" value={`$${m.cachedTokenSavings.toFixed(5)}`} color="#40c0c0" note="cached @ 10% input rate" />
                <MetricCard label="Token efficiency" value={m.tokenEfficiencyRatio.toFixed(3)} color="#d0a030" note="output/input ratio" />
            </div>
        </div>
    );
}

function TracesTab() {
    const traces = React.useMemo(makeTraces, []);
    const [sel, setSel] = React.useState<string | null>(null);
    const selected = traces.find(t => t.traceId === sel);
    return (
        <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
            <div style={{ width: 200, borderRight: '1px solid #1e1e1e', overflow: 'auto' }}>
                {traces.map(t => (
                    <div key={t.traceId} onClick={() => setSel(t.traceId)}
                        style={{ padding: '8px 10px', cursor: 'pointer', borderBottom: '1px solid #1a1a1a', background: sel === t.traceId ? '#1a2a3a' : 'transparent' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 2 }}>
                            <StatusDot status={t.errorCount > 0 ? 'error' : 'healthy'} />
                            <span style={{ fontSize: 11, fontFamily: 'monospace', color: '#a0c0e0' }}>{t.traceId}</span>
                        </div>
                        <div style={{ fontSize: 10, color: '#666' }}>{t.rootSpan.operationName} · {t.totalDurationMs}ms · {t.spanCount} spans</div>
                    </div>
                ))}
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
                {selected ? (
                    <div>
                        <div style={{ fontSize: 12, fontWeight: 700, color: '#ccc', marginBottom: 8 }}>{selected.rootSpan.operationName}</div>
                        <div style={{ display: 'flex', gap: 10, marginBottom: 10, flexWrap: 'wrap' }}>
                            <span style={{ fontSize: 10, color: '#888' }}>Total: <b style={{ color: '#c0d0e0' }}>{selected.totalDurationMs}ms</b></span>
                            <span style={{ fontSize: 10, color: '#888' }}>Spans: <b style={{ color: '#c0d0e0' }}>{selected.spanCount}</b></span>
                            <span style={{ fontSize: 10, color: '#888' }}>p99: <b style={{ color: '#c0d0e0' }}>{selected.p99LatencyMs}ms</b></span>
                            <span style={{ fontSize: 10, color: '#888' }}>Errors: <b style={{ color: selected.errorCount > 0 ? '#d06060' : '#c0d0e0' }}>{selected.errorCount}</b></span>
                        </div>
                        {[selected.rootSpan, ...selected.spans].map(s => (
                            <div key={s.spanId} style={{ background: '#141414', border: `1px solid ${s.status === 'error' ? '#4a1a1a' : '#1e1e1e'}`, borderRadius: 3, padding: '5px 8px', marginBottom: 4, paddingLeft: s.parentSpanId ? 20 : 8 }}>
                                <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                                    <span style={{ fontSize: 10, color: s.status === 'error' ? '#d06060' : '#60d060' }}>{s.status === 'error' ? '✗' : '✓'}</span>
                                    <span style={{ fontSize: 11, color: '#c0c0c0', fontFamily: 'monospace' }}>{s.operationName}</span>
                                    <span style={{ fontSize: 10, color: '#555' }}>{s.serviceName}</span>
                                    <span style={{ marginLeft: 'auto', fontSize: 10, color: '#666', fontFamily: 'monospace' }}>{s.durationMs}ms</span>
                                </div>
                            </div>
                        ))}
                    </div>
                ) : <div style={{ color: '#444', fontSize: 12, paddingTop: 20 }}>Select a trace.</div>}
            </div>
        </div>
    );
}

function MonitorTab() {
    const [snapshots, setSnapshots] = React.useState(() => DEMO_AGENTS.map(a => makeMonitor(a.id)));
    React.useEffect(() => {
        const iv = setInterval(() => setSnapshots(DEMO_AGENTS.map(a => makeMonitor(a.id))), 3000);
        return () => clearInterval(iv);
    }, []);
    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 10 }}>
            <div style={{ fontSize: 10, color: '#444', fontStyle: 'italic', paddingBottom: 4, borderBottom: '1px solid #1a1a1a' }}>
                errorRatePct = failedRuns/totalRuns*100 · throughputPerMin = completedTasks/(windowMs/60000) · Source: Google SRE Book Ch.6
            </div>
            {snapshots.map((s, i) => (
                <div key={s.agentId} style={{ background: '#141414', border: `1px solid ${s.health === 'degraded' ? '#3a2a00' : '#1e1e1e'}`, borderRadius: 5, padding: '10px 12px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                        <StatusDot status={s.health} />
                        <span style={{ fontWeight: 700, fontSize: 12 }}>{DEMO_AGENTS[i].name}</span>
                        <span style={{ fontSize: 10, color: '#555' }}>{DEMO_AGENTS[i].model}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: s.health === 'degraded' ? '#d0a030' : '#40d040' }}>{s.health}</span>
                    </div>
                    <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap', fontSize: 11, color: '#888' }}>
                        <span>Uptime <b style={{ color: '#a0c0a0' }}>{s.uptimePct.toFixed(1)}%</b></span>
                        <span>Error rate <b style={{ color: s.errorRatePct > 5 ? '#d06060' : '#a0c0a0' }}>{s.errorRatePct.toFixed(1)}%</b></span>
                        <span>Throughput <b style={{ color: '#a0c0e0' }}>{s.throughputPerMin.toFixed(1)}/min</b></span>
                        <span>Queue <b style={{ color: '#c0c0a0' }}>{s.queueDepth}</b></span>
                        <span>p99 <b style={{ color: '#c0a0e0' }}>{s.p99LatencyMs.toFixed(0)}ms</b></span>
                        <span>CPU <b style={{ color: s.cpuUsagePct > 80 ? '#d06060' : '#c0c0c0' }}>{s.cpuUsagePct.toFixed(0)}%</b></span>
                    </div>
                </div>
            ))}
        </div>
    );
}

function SourcesTab() {
    const [sources, setSources] = React.useState<RepoSource[]>(DEMO_SOURCES);
    const [syncing, setSyncing] = React.useState<string | null>(null);

    const sync = (id: string) => {
        setSyncing(id);
        setSources(ss => ss.map(s => s.id === id ? { ...s, syncStatus: 'syncing' } : s));
        setTimeout(() => {
            setSources(ss => ss.map(s => s.id === id ? { ...s, syncStatus: 'synced', lastSyncAt: new Date().toISOString() } : s));
            setSyncing(null);
        }, 1800);
    };

    const TYPE_COLORS: Record<string, string> = { 'agent-bench': '#60d0a0', 'agent-lib': '#c080ff', 'tool-lib': '#60b0ff', custom: '#888' };

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 10 }}>
            <div style={{ fontSize: 10, color: '#444', fontStyle: 'italic', paddingBottom: 6, borderBottom: '1px solid #1a1a1a' }}>
                Headless consumption: GET https://api.github.com/repos/&#123;owner&#125;/&#123;repo&#125;/contents/&#123;manifestPath&#125;
                → decode base64 → import agents / benchSuites / tools
            </div>
            {sources.map(s => (
                <div key={s.id} style={{ background: '#141414', border: '1px solid #1e1e1e', borderRadius: 5, padding: '10px 12px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                        <span style={{ fontSize: 10, background: TYPE_COLORS[s.type] + '22', color: TYPE_COLORS[s.type], padding: '1px 6px', borderRadius: 3, fontWeight: 700 }}>{s.type}</span>
                        <span style={{ fontWeight: 700, fontSize: 12 }}>{s.owner}/{s.repo}</span>
                        <span style={{ fontSize: 10, color: '#555' }}>@{s.branch}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: { synced: '#40d040', syncing: '#d0a030', idle: '#555', error: '#d04040' }[s.syncStatus] }}>{s.syncStatus}</span>
                        <button onClick={() => sync(s.id)} disabled={syncing === s.id}
                            style={{ padding: '2px 8px', background: '#1a2a1a', border: '1px solid #2a4a2a', color: '#60d060', borderRadius: 3, cursor: 'pointer', fontSize: 10 }}>
                            {syncing === s.id ? 'Syncing…' : '↻ Sync'}
                        </button>
                    </div>
                    <div style={{ display: 'flex', gap: 12, fontSize: 10, color: '#666' }}>
                        <span>agents: <b style={{ color: '#c0d0e0' }}>{s.imported.agents}</b></span>
                        <span>bench suites: <b style={{ color: '#c0d0e0' }}>{s.imported.benchSuites}</b></span>
                        <span>tools: <b style={{ color: '#c0d0e0' }}>{s.imported.tools}</b></span>
                        <span>tasks: <b style={{ color: '#c0d0e0' }}>{s.imported.tasks}</b></span>
                        {s.lastSyncAt && <span style={{ marginLeft: 'auto' }}>last sync: {new Date(s.lastSyncAt).toLocaleTimeString()}</span>}
                    </div>
                </div>
            ))}
        </div>
    );
}

function PlatformView() {
    const [tab, setTab] = React.useState<PlatformTab>('overview');
    const TABS: { id: PlatformTab; label: string }[] = [
        { id: 'overview', label: 'Overview' }, { id: 'finops', label: 'FinOps' },
        { id: 'traces', label: 'Traces' }, { id: 'monitor', label: 'Monitor' },
        { id: 'sources', label: 'Sources' },
    ];
    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontWeight: 700, fontSize: 14, color: '#60b0ff' }}>◎ Platform</span>
                <StatusDot status="healthy" />
                <span style={{ fontSize: 10, color: '#555' }}>4 agents · 3 sources connected</span>
            </div>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {TABS.map(t => (
                    <button key={t.id} onClick={() => setTab(t.id)} style={{
                        padding: '6px 14px', border: 'none', background: 'transparent',
                        color: tab === t.id ? '#60b0ff' : '#555',
                        borderBottom: tab === t.id ? '2px solid #60b0ff' : '2px solid transparent',
                        cursor: 'pointer', fontSize: 12, fontWeight: tab === t.id ? 700 : 400,
                    }}>{t.label}</button>
                ))}
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {tab === 'overview' && <OverviewTab />}
                {tab === 'finops'   && <FinOpsTab />}
                {tab === 'traces'   && <div style={{ display: 'flex', height: '100%' }}><TracesTab /></div>}
                {tab === 'monitor'  && <MonitorTab />}
                {tab === 'sources'  && <SourcesTab />}
            </div>
        </div>
    );
}

@injectable()
export class PlatformPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:platform';
    static readonly LABEL = 'Platform';
    @postConstruct() protected init(): void {
        this.id = PlatformPanelWidget.ID; this.title.label = PlatformPanelWidget.LABEL;
        this.title.caption = 'Platform central dashboard'; this.title.closable = true;
        this.title.iconClass = 'codicon codicon-server'; this.update();
    }
    protected render(): React.ReactNode { return <PlatformView />; }
}

@injectable()
export class PlatformPanelContribution extends AbstractViewContribution<PlatformPanelWidget> {
    constructor() { super({ widgetId: PlatformPanelWidget.ID, widgetName: PlatformPanelWidget.LABEL, defaultWidgetOptions: { area: 'main' }, toggleCommandId: PlatformPanelCommand.id }); }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(PlatformPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
