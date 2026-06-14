import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { CloudPanelCommand } from '../agent-ide-commands';

// ─── Types ────────────────────────────────────────────────────────────────────

type CloudTab = 'core' | 'cloud' | 'mesh' | 'deploy' | 'build';

interface HealthData {
    runtime?: string;
    uptime?: number;
    version?: string;
}

interface RunnerStatus {
    phase?: number;
    runtime?: string;
    binary?: string;
    version?: string;
    tokio_threads?: number;
}

interface Instance {
    name: string;
    url: string;
    region: string;
    status: string;
    provider: string;
}

interface Peer {
    id: string;
    host: string;
    status: 'online' | 'offline' | 'unknown';
    latencyMs?: number;
    version?: string;
}

interface KubeStatus {
    name?: string;
    image?: string;
    replicas?: number;
    health?: string;
    phase?: string;
}

interface CIRun {
    id: string;
    name: string;
    status: 'success' | 'failure' | 'in_progress' | 'queued';
    branch: string;
    commitSha: string;
    triggeredAt: string;
    durationMs?: number;
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

const ACCENT = '#c080ff';

function StatusDot({ color }: { color: string }) {
    return <span style={{ width: 7, height: 7, borderRadius: '50%', background: color, display: 'inline-block', flexShrink: 0 }} />;
}

function ProviderBadge({ provider }: { provider: string }) {
    const colors: Record<string, string> = { OCI: '#f08030', CloudStack: '#4090d0', k3s: '#60c060' };
    const c = colors[provider] ?? '#888';
    return (
        <span style={{ fontSize: 9, background: c + '22', color: c, padding: '1px 5px', borderRadius: 3, fontWeight: 700, border: `1px solid ${c}44` }}>
            {provider}
        </span>
    );
}

function MetricCard({ label, value, unit, color }: { label: string; value: string | number; unit?: string; color?: string }) {
    return (
        <div style={{ background: '#141414', border: '1px solid #222', borderRadius: 5, padding: '10px 12px', minWidth: 130 }}>
            <div style={{ fontSize: 10, color: '#666', marginBottom: 4 }}>{label}</div>
            <div style={{ fontSize: 18, fontWeight: 700, color: color ?? '#c0d0e0', fontFamily: 'monospace' }}>
                {value}{unit && <span style={{ fontSize: 11, color: '#555', marginLeft: 2 }}>{unit}</span>}
            </div>
        </div>
    );
}

function ActionButton({ label, onClick, color }: { label: string; onClick: () => void; color?: string }) {
    const c = color ?? ACCENT;
    return (
        <button onClick={onClick} style={{
            padding: '4px 12px', background: c + '22', border: `1px solid ${c}66`,
            color: c, borderRadius: 4, cursor: 'pointer', fontSize: 11, fontWeight: 600,
        }}>{label}</button>
    );
}

function statusColor(status: string): string {
    if (status === 'online' || status === 'healthy' || status === 'Running') return '#40d040';
    if (status === 'offline' || status === 'error' || status === 'Failed') return '#d04040';
    return '#888';
}

// ─── Face 1: Core ─────────────────────────────────────────────────────────────

const STACK_LAYERS = [
    { name: 'gate',     color: '#4090d0', note: 'auth middleware' },
    { name: 'routes',   color: '#60d0a0', note: 'REST API' },
    { name: 'agent',    color: ACCENT,    note: 'agent executor' },
    { name: 'tools',    color: '#d0a030', note: 'tool registry' },
    { name: 'store',    color: '#60b0ff', note: 'app state' },
    { name: 'transfer', color: '#d06060', note: 'streaming' },
];

function CoreTab() {
    const [health, setHealth] = React.useState<HealthData>({});
    const [runnerStatus, setRunnerStatus] = React.useState<RunnerStatus | null>(null);
    const [loading, setLoading] = React.useState(true);

    const fetchData = React.useCallback(() => {
        fetch('/health')
            .then(r => r.json())
            .then((d: HealthData) => setHealth(d))
            .catch(() => setHealth({}));
        fetch('/api/infra/runner-status')
            .then(r => r.json())
            .then((d: RunnerStatus) => setRunnerStatus(d))
            .catch(() => setRunnerStatus(null))
            .finally(() => setLoading(false));
    }, []);

    React.useEffect(() => {
        fetchData();
        const iv = setInterval(fetchData, 10000);
        return () => clearInterval(iv);
    }, [fetchData]);

    const isRust = health.runtime === 'rust' || runnerStatus?.runtime === 'rust';
    const phase = isRust ? 2 : 1;
    const phaseLabel = isRust ? 'PHASE 2 · Rust' : 'PHASE 1 · TypeScript';
    const phaseColor = isRust ? '#40d040' : '#d0a030';

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 14 }}>
            {/* Phase badge */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <div style={{
                    padding: '8px 20px', background: phaseColor + '18', border: `2px solid ${phaseColor}`,
                    borderRadius: 8, fontWeight: 700, fontSize: 16, color: phaseColor, fontFamily: 'monospace', letterSpacing: 1,
                }}>
                    {phaseLabel}
                </div>
                <div style={{ fontSize: 10, color: '#555' }}>
                    {loading ? 'polling…' : `phase ${phase} active`}
                </div>
            </div>

            {/* Health metrics */}
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                <MetricCard label="Uptime" value={health.uptime != null ? `${Math.floor(health.uptime)}` : '—'} unit="s" color="#60b0ff" />
                <MetricCard label="Version" value={health.version ?? runnerStatus?.version ?? '—'} color="#60d0a0" />
                {runnerStatus && (
                    <MetricCard label="Tokio threads" value={runnerStatus.tokio_threads ?? '—'} color={ACCENT} />
                )}
            </div>

            {/* Runner status */}
            <div style={{ background: '#141414', border: '1px solid #222', borderRadius: 5, padding: '10px 12px' }}>
                <div style={{ fontSize: 10, color: '#666', marginBottom: 6 }}>Rust binary · agent-runner</div>
                {runnerStatus ? (
                    <div style={{ display: 'flex', gap: 16, fontSize: 11, color: '#888', flexWrap: 'wrap' }}>
                        <span><StatusDot color="#40d040" /> <b style={{ color: '#c0d0e0' }}>{runnerStatus.binary ?? 'agent-runner'}</b></span>
                        <span>runtime: <b style={{ color: '#60d0a0' }}>{runnerStatus.runtime ?? '—'}</b></span>
                        <span>v{runnerStatus.version ?? '—'}</span>
                    </div>
                ) : (
                    <div style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 11, color: '#d04040' }}>
                        <StatusDot color="#d04040" /> not reachable
                    </div>
                )}
            </div>

            {/* Stack layers */}
            <div>
                <div style={{ fontSize: 11, color: '#888', fontWeight: 700, marginBottom: 6 }}>Stack Layers</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {STACK_LAYERS.map(l => (
                        <div key={l.name} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '5px 8px', background: '#141414', border: '1px solid #1e1e1e', borderRadius: 4 }}>
                            <StatusDot color={l.color} />
                            <span style={{ fontSize: 12, fontWeight: 600, color: l.color, width: 70, fontFamily: 'monospace' }}>{l.name}</span>
                            <span style={{ fontSize: 10, color: '#555' }}>{l.note}</span>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}

// ─── Face 2: Cloud ────────────────────────────────────────────────────────────

type ProviderFilter = 'All' | 'OCI' | 'CloudStack' | 'k3s';

function CloudTab() {
    const [instances, setInstances] = React.useState<Instance[]>([]);
    const [filter, setFilter] = React.useState<ProviderFilter>('All');
    const [showForm, setShowForm] = React.useState(false);
    const [form, setForm] = React.useState({ name: '', url: '', region: '', provider: 'OCI' });
    const [submitting, setSubmitting] = React.useState(false);

    React.useEffect(() => {
        fetch('/api/infra/instances')
            .then(r => r.json())
            .then((d: Instance[]) => Array.isArray(d) ? setInstances(d) : setInstances([]))
            .catch(() => setInstances([]));
    }, []);

    const PROVIDERS: ProviderFilter[] = ['All', 'OCI', 'CloudStack', 'k3s'];
    const visible = filter === 'All' ? instances : instances.filter(i => i.provider === filter);

    const submit = () => {
        if (!form.name || !form.url) return;
        setSubmitting(true);
        fetch('/api/infra/instances', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(form),
        }).finally(() => {
            setSubmitting(false);
            setShowForm(false);
            setForm({ name: '', url: '', region: '', provider: 'OCI' });
        });
    };

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 12 }}>
            {/* Provider filter tabs */}
            <div style={{ display: 'flex', gap: 4 }}>
                {PROVIDERS.map(p => (
                    <button key={p} onClick={() => setFilter(p)} style={{
                        padding: '4px 10px', border: 'none', background: filter === p ? ACCENT + '22' : 'transparent',
                        color: filter === p ? ACCENT : '#555', borderBottom: filter === p ? `2px solid ${ACCENT}` : '2px solid transparent',
                        cursor: 'pointer', fontSize: 11, fontWeight: filter === p ? 700 : 400,
                    }}>{p}</button>
                ))}
                <div style={{ flex: 1 }} />
                <ActionButton label="+ Add instance" onClick={() => setShowForm(v => !v)} />
            </div>

            {/* Add instance form */}
            {showForm && (
                <div style={{ background: '#141414', border: `1px solid ${ACCENT}44`, borderRadius: 6, padding: '12px 14px', display: 'flex', flexDirection: 'column', gap: 8 }}>
                    <div style={{ fontSize: 11, color: ACCENT, fontWeight: 700, marginBottom: 2 }}>New Instance</div>
                    {(['name', 'url', 'region'] as const).map(f => (
                        <input key={f} placeholder={f} value={form[f]} onChange={e => setForm(v => ({ ...v, [f]: e.target.value }))}
                            style={{ background: '#0f0f0f', border: '1px solid #2a2a2a', borderRadius: 3, color: '#ccc', padding: '4px 8px', fontSize: 11 }} />
                    ))}
                    <select value={form.provider} onChange={e => setForm(v => ({ ...v, provider: e.target.value }))}
                        style={{ background: '#0f0f0f', border: '1px solid #2a2a2a', borderRadius: 3, color: '#ccc', padding: '4px 8px', fontSize: 11 }}>
                        <option>OCI</option><option>CloudStack</option><option>k3s</option>
                    </select>
                    <div style={{ display: 'flex', gap: 8 }}>
                        <ActionButton label={submitting ? 'Adding…' : 'Add'} onClick={submit} />
                        <ActionButton label="Cancel" onClick={() => setShowForm(false)} color="#888" />
                    </div>
                </div>
            )}

            {/* Instance list */}
            {visible.length === 0 ? (
                <div style={{ color: '#444', fontSize: 12, padding: '12px 0' }}>No instances. Set INFRA_INSTANCES env or add one above.</div>
            ) : visible.map((inst, i) => (
                <div key={i} style={{ background: '#141414', border: '1px solid #1e1e1e', borderRadius: 5, padding: '10px 12px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                        <StatusDot color={statusColor(inst.status)} />
                        <span style={{ fontWeight: 700, fontSize: 12 }}>{inst.name}</span>
                        <ProviderBadge provider={inst.provider} />
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: '#555' }}>{inst.region}</span>
                    </div>
                    <div style={{ fontSize: 10, color: '#60b0ff', fontFamily: 'monospace' }}>{inst.url}</div>
                </div>
            ))}
        </div>
    );
}

// ─── Face 3: Mesh ─────────────────────────────────────────────────────────────

function MeshTab() {
    const [peers, setPeers] = React.useState<Peer[]>([]);
    const [selected, setSelected] = React.useState<Peer | null>(null);
    const thisHost = typeof window !== 'undefined' ? window.location.host : 'localhost';

    React.useEffect(() => {
        fetch('/api/peers')
            .then(r => r.json())
            .then((d: Peer[]) => Array.isArray(d) ? setPeers(d) : setPeers([]))
            .catch(() => setPeers([]));
    }, []);

    // Simple SVG grid layout
    const BOX_W = 110, BOX_H = 44, PAD = 20;
    const cols = 3;
    const allNodes = [{ id: '__self__', host: thisHost, status: 'online' as const }, ...peers];
    const svgW = cols * (BOX_W + PAD) + PAD;
    const rows = Math.ceil(allNodes.length / cols);
    const svgH = rows * (BOX_H + PAD) + PAD;

    const nodePos = (idx: number) => ({
        x: PAD + (idx % cols) * (BOX_W + PAD),
        y: PAD + Math.floor(idx / cols) * (BOX_H + PAD),
    });

    const centerPos = (idx: number) => {
        const p = nodePos(idx);
        return { cx: p.x + BOX_W / 2, cy: p.y + BOX_H / 2 };
    };

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div style={{ fontSize: 10, color: '#555' }}>
                This instance: <b style={{ color: ACCENT }}>{thisHost}</b> · {peers.length} peer(s)
            </div>

            <svg width={svgW} height={svgH} style={{ background: '#0f0f0f', borderRadius: 6, border: '1px solid #1e1e1e', display: 'block' }}>
                {/* Lines from self to all peers */}
                {peers.map((_, i) => {
                    const from = centerPos(0);
                    const to = centerPos(i + 1);
                    return (
                        <line key={i} x1={from.cx} y1={from.cy} x2={to.cx} y2={to.cy}
                            stroke="#2a2a2a" strokeWidth={1.5} strokeDasharray="4 3" />
                    );
                })}
                {/* Nodes */}
                {allNodes.map((node, i) => {
                    const { x, y } = nodePos(i);
                    const isSelf = node.id === '__self__';
                    const c = isSelf ? ACCENT : statusColor(node.status);
                    const isSelected = selected?.id === node.id;
                    return (
                        <g key={node.id} style={{ cursor: isSelf ? 'default' : 'pointer' }}
                            onClick={() => !isSelf && setSelected(p => p?.id === node.id ? null : node as Peer)}>
                            <rect x={x} y={y} width={BOX_W} height={BOX_H} rx={5}
                                fill={isSelected ? c + '22' : '#141414'} stroke={c} strokeWidth={isSelf ? 2 : 1} />
                            <circle cx={x + 12} cy={y + BOX_H / 2} r={4} fill={c} />
                            <text x={x + 22} y={y + BOX_H / 2 - 5} fill="#ccc" fontSize={9} fontFamily="monospace">
                                {node.host.length > 14 ? node.host.slice(0, 13) + '…' : node.host}
                            </text>
                            <text x={x + 22} y={y + BOX_H / 2 + 8} fill={c} fontSize={8}>
                                {isSelf ? '★ this' : node.status}
                            </text>
                        </g>
                    );
                })}
            </svg>

            {/* Selected peer detail */}
            {selected && (
                <div style={{ background: '#141414', border: `1px solid ${ACCENT}44`, borderRadius: 5, padding: '10px 12px' }}>
                    <div style={{ fontSize: 11, color: ACCENT, fontWeight: 700, marginBottom: 6 }}>Peer detail</div>
                    <div style={{ fontSize: 11, color: '#888', display: 'flex', flexDirection: 'column', gap: 3 }}>
                        <span>host: <b style={{ color: '#c0d0e0' }}>{selected.host}</b></span>
                        <span>status: <b style={{ color: statusColor(selected.status) }}>{selected.status}</b></span>
                        {selected.latencyMs != null && <span>latency: <b style={{ color: '#c0d0e0' }}>{selected.latencyMs}ms</b></span>}
                        {selected.version && <span>version: <b style={{ color: '#c0d0e0' }}>{selected.version}</b></span>}
                    </div>
                </div>
            )}

            {peers.length === 0 && (
                <div style={{ color: '#444', fontSize: 12 }}>No peers discovered. Check /api/peers.</div>
            )}
        </div>
    );
}

// ─── Face 4: Deploy ───────────────────────────────────────────────────────────

function DeployTab() {
    const [status, setStatus] = React.useState<KubeStatus>({});
    const [applying, setApplying] = React.useState(false);
    const [applyResult, setApplyResult] = React.useState<string | null>(null);
    const [buildPhase, setBuildPhase] = React.useState<1 | 2>(1);

    const fetchStatus = React.useCallback(() => {
        fetch('/api/deploy/status')
            .then(r => r.json())
            .then((d: KubeStatus) => setStatus(d))
            .catch(() => setStatus({}));
    }, []);

    React.useEffect(() => {
        fetchStatus();
        const iv = setInterval(fetchStatus, 8000);
        return () => clearInterval(iv);
    }, [fetchStatus]);

    const apply = () => {
        setApplying(true);
        setApplyResult(null);
        fetch('/api/deploy/apply', { method: 'POST' })
            .then(r => r.json())
            .then(() => setApplyResult('Applied successfully'))
            .catch(() => setApplyResult('Apply failed'))
            .finally(() => setApplying(false));
    };

    const phaseImage = buildPhase === 1 ? 'agent-runner:node-latest' : 'agent-runner:rust-latest';
    const healthColor = statusColor(status.health ?? 'unknown');
    const phaseColor = statusColor(status.phase ?? 'unknown');

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 14 }}>
            {/* KubeContainer status */}
            <div style={{ background: '#141414', border: '1px solid #222', borderRadius: 5, padding: '12px 14px' }}>
                <div style={{ fontSize: 10, color: '#555', marginBottom: 8 }}>KubeContainer · CRD</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, fontSize: 11, color: '#888' }}>
                    <div style={{ display: 'flex', gap: 8 }}>
                        <span style={{ width: 80 }}>name</span>
                        <b style={{ color: '#c0d0e0', fontFamily: 'monospace' }}>{status.name ?? '—'}</b>
                    </div>
                    <div style={{ display: 'flex', gap: 8 }}>
                        <span style={{ width: 80 }}>image</span>
                        <b style={{ color: '#c0d0e0', fontFamily: 'monospace' }}>{status.image ?? '—'}</b>
                    </div>
                    <div style={{ display: 'flex', gap: 8 }}>
                        <span style={{ width: 80 }}>replicas</span>
                        <b style={{ color: ACCENT }}>{status.replicas ?? '—'}</b>
                    </div>
                    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                        <span style={{ width: 80 }}>health</span>
                        <StatusDot color={healthColor} />
                        <b style={{ color: healthColor }}>{status.health ?? '—'}</b>
                    </div>
                    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                        <span style={{ width: 80 }}>phase</span>
                        <StatusDot color={phaseColor} />
                        <b style={{ color: phaseColor }}>{status.phase ?? '—'}</b>
                    </div>
                </div>
            </div>

            {/* Build phase switcher */}
            <div style={{ background: '#141414', border: '1px solid #222', borderRadius: 5, padding: '10px 12px' }}>
                <div style={{ fontSize: 10, color: '#555', marginBottom: 8 }}>Build phase · image selector</div>
                <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
                    {([1, 2] as const).map(p => (
                        <button key={p} onClick={() => setBuildPhase(p)} style={{
                            padding: '4px 12px', border: `1px solid ${buildPhase === p ? ACCENT : '#333'}`,
                            background: buildPhase === p ? ACCENT + '22' : 'transparent',
                            color: buildPhase === p ? ACCENT : '#555', borderRadius: 4, cursor: 'pointer', fontSize: 11,
                        }}>Phase {p} · {p === 1 ? 'Node TS' : 'Rust'}</button>
                    ))}
                </div>
                <div style={{ fontSize: 10, color: '#666', fontFamily: 'monospace' }}>
                    image: <span style={{ color: '#60d0a0' }}>{phaseImage}</span>
                </div>
            </div>

            {/* Apply button */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <ActionButton label={applying ? 'Applying…' : 'Apply to cluster'} onClick={apply} />
                {applyResult && <span style={{ fontSize: 11, color: applyResult.includes('fail') ? '#d06060' : '#40d040' }}>{applyResult}</span>}
            </div>
        </div>
    );
}

// ─── Face 5: Build ────────────────────────────────────────────────────────────

type CIStepStatus = 'success' | 'failure' | 'in_progress' | 'pending';

const CI_STEPS = ['build', 'push', 'deploy'] as const;

function CIStepIcon({ status }: { status: CIStepStatus }) {
    if (status === 'success')     return <span style={{ color: '#40d040', fontSize: 14 }}>✓</span>;
    if (status === 'failure')     return <span style={{ color: '#d04040', fontSize: 14 }}>✗</span>;
    if (status === 'in_progress') return <span style={{ color: '#d0a030', fontSize: 14 }}>⟳</span>;
    return <span style={{ color: '#555', fontSize: 14 }}>○</span>;
}

function runStepStatus(run: CIRun, step: typeof CI_STEPS[number]): CIStepStatus {
    if (run.status === 'failure') {
        if (step === 'build') return 'failure';
        return 'pending';
    }
    if (run.status === 'success') return 'success';
    if (run.status === 'in_progress') {
        if (step === 'build') return 'in_progress';
        return 'pending';
    }
    return 'pending';
}

function BuildTab() {
    const [runs, setRuns] = React.useState<CIRun[]>([]);
    const [triggering, setTriggering] = React.useState(false);
    const [triggerMsg, setTriggerMsg] = React.useState<string | null>(null);

    const fetchRuns = React.useCallback(() => {
        fetch('/api/ci/runs')
            .then(r => r.json())
            .then((d: CIRun[]) => Array.isArray(d) ? setRuns(d) : setRuns([]))
            .catch(() => setRuns([]));
    }, []);

    React.useEffect(() => {
        fetchRuns();
        const iv = setInterval(fetchRuns, 15000);
        return () => clearInterval(iv);
    }, [fetchRuns]);

    const trigger = () => {
        setTriggering(true);
        setTriggerMsg(null);
        fetch('/api/ci/trigger', { method: 'POST' })
            .then(r => r.json())
            .then((d: { message?: string }) => setTriggerMsg(d.message ?? 'Triggered'))
            .catch(() => setTriggerMsg('Trigger failed'))
            .finally(() => setTriggering(false));
    };

    const runStatusColor = (s: string) => {
        if (s === 'success') return '#40d040';
        if (s === 'failure') return '#d04040';
        if (s === 'in_progress') return '#d0a030';
        return '#555';
    };

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <ActionButton label={triggering ? 'Triggering…' : '▶ Trigger build'} onClick={trigger} />
                {triggerMsg && <span style={{ fontSize: 11, color: triggerMsg.includes('fail') ? '#d04040' : '#40d040' }}>{triggerMsg}</span>}
            </div>

            {runs.length === 0 ? (
                <div style={{ color: '#444', fontSize: 12 }}>No CI runs yet. /api/ci/runs returns [].</div>
            ) : runs.map(run => (
                <div key={run.id} style={{ background: '#141414', border: `1px solid ${runStatusColor(run.status)}44`, borderRadius: 5, padding: '10px 12px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                        <StatusDot color={runStatusColor(run.status)} />
                        <span style={{ fontWeight: 700, fontSize: 12 }}>{run.name}</span>
                        <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>{run.commitSha.slice(0, 7)}</span>
                        <span style={{ fontSize: 10, color: '#555' }}>@{run.branch}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: runStatusColor(run.status) }}>{run.status}</span>
                    </div>
                    {/* Pipeline steps */}
                    <div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
                        {CI_STEPS.map((step, i) => (
                            <React.Fragment key={step}>
                                <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2 }}>
                                    <CIStepIcon status={runStepStatus(run, step)} />
                                    <span style={{ fontSize: 9, color: '#666' }}>{step}</span>
                                </div>
                                {i < CI_STEPS.length - 1 && (
                                    <div style={{ width: 24, height: 1, background: '#2a2a2a', margin: '0 4px', marginBottom: 14 }} />
                                )}
                            </React.Fragment>
                        ))}
                        {run.durationMs != null && (
                            <span style={{ marginLeft: 'auto', fontSize: 10, color: '#555', fontFamily: 'monospace' }}>
                                {(run.durationMs / 1000).toFixed(1)}s
                            </span>
                        )}
                    </div>
                    <div style={{ fontSize: 10, color: '#444', marginTop: 4 }}>
                        {new Date(run.triggeredAt).toLocaleString()}
                    </div>
                </div>
            ))}
        </div>
    );
}

// ─── Root view ────────────────────────────────────────────────────────────────

function CloudView() {
    const [tab, setTab] = React.useState<CloudTab>('core');
    const TABS: { id: CloudTab; label: string }[] = [
        { id: 'core',   label: 'Core'   },
        { id: 'cloud',  label: 'Cloud'  },
        { id: 'mesh',   label: 'Mesh'   },
        { id: 'deploy', label: 'Deploy' },
        { id: 'build',  label: 'Build'  },
    ];
    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontWeight: 700, fontSize: 14, color: ACCENT }}>⬡ Metal Core Cloud</span>
                <span style={{ fontSize: 10, color: '#555' }}>full-stack control plane</span>
            </div>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {TABS.map(t => (
                    <button key={t.id} onClick={() => setTab(t.id)} style={{
                        padding: '6px 14px', border: 'none', background: 'transparent',
                        color: tab === t.id ? ACCENT : '#555',
                        borderBottom: tab === t.id ? `2px solid ${ACCENT}` : '2px solid transparent',
                        cursor: 'pointer', fontSize: 12, fontWeight: tab === t.id ? 700 : 400,
                    }}>{t.label}</button>
                ))}
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {tab === 'core'   && <CoreTab />}
                {tab === 'cloud'  && <CloudTab />}
                {tab === 'mesh'   && <MeshTab />}
                {tab === 'deploy' && <DeployTab />}
                {tab === 'build'  && <BuildTab />}
            </div>
        </div>
    );
}

// ─── Theia widget & contribution ──────────────────────────────────────────────

@injectable()
export class CloudPanelWidget extends ReactWidget {
    static readonly ID    = 'agent-ide:cloud';
    static readonly LABEL = 'Cloud';
    @postConstruct() protected init(): void {
        this.id = CloudPanelWidget.ID;
        this.title.label   = CloudPanelWidget.LABEL;
        this.title.caption = 'Metal Core Multi Face Cloud Panel';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-cloud';
        this.update();
    }
    protected render(): React.ReactNode { return <CloudView />; }
}

@injectable()
export class CloudPanelContribution extends AbstractViewContribution<CloudPanelWidget> {
    constructor() {
        super({
            widgetId: CloudPanelWidget.ID,
            widgetName: CloudPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'main' },
            toggleCommandId: CloudPanelCommand.id,
        });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(CloudPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
