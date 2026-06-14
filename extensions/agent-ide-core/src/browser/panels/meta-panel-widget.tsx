import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { MetaPanelCommand } from '../agent-ide-commands';
import { apiFetch } from '../runtime/backend-client';

// ── Types ────────────────────────────────────────────────────────────────────

interface Peer {
    id: string;
    name: string;
    url: string;           // e.g. https://ide.agennext.com
    status: 'online' | 'offline' | 'unknown';
    lastSeen?: string;
    region?: string;
}

interface InfraInstance {
    name: string;
    url: string;
    region: string;
    ip?: string;
    status: 'healthy' | 'degraded' | 'offline' | 'unknown';
    uptime?: string;
}

interface AgentRef {
    id: string;
    name: string;
    model: string;
}

type MetaTab = 'infra' | 'peers' | 'transfer';

// ── API helpers ───────────────────────────────────────────────────────────────

const listPeers   = () => apiFetch<Peer[]>('/api/peers');
const addPeer     = (body: { name: string; url: string }) =>
    apiFetch<Peer>('/api/peers', { method: 'POST', body: JSON.stringify(body) });
const removePeer  = (id: string) => apiFetch<void>(`/api/peers/${id}`, { method: 'DELETE' });
const listAgents  = () => apiFetch<AgentRef[]>('/api/agents');
const transferAgent = (agentId: string, peerId: string) =>
    apiFetch<{ ok: boolean; message: string }>('/api/transfer', {
        method: 'POST',
        body: JSON.stringify({ agentId, peerId }),
    });
const listInfra = () => apiFetch<InfraInstance[]>('/api/infra/instances');

// ── Sub-components ────────────────────────────────────────────────────────────

function StatusDot({ status }: { status: string }) {
    const color = {
        online: '#40d040', healthy: '#40d040',
        offline: '#d04040', degraded: '#d0a030',
        unknown: '#555',
    }[status] ?? '#555';
    return <span style={{ width: 7, height: 7, borderRadius: '50%', background: color, display: 'inline-block', flexShrink: 0 }} />;
}

function SectionHeader({ children }: { children: React.ReactNode }) {
    return <div style={{ fontSize: 10, color: '#666', fontWeight: 700, letterSpacing: 1, textTransform: 'uppercase', marginBottom: 6, marginTop: 4 }}>{children}</div>;
}

// ── Infra Tab ─────────────────────────────────────────────────────────────────

function InfraTab() {
    const [instances, setInstances] = React.useState<InfraInstance[]>([]);
    const [loading, setLoading]     = React.useState(true);
    const [error, setError]         = React.useState<string | null>(null);

    React.useEffect(() => {
        listInfra()
            .then(setInstances)
            .catch(() => {
                // Backend may not have infra data yet — show config hint
                setInstances([
                    { name: 'agent-ide', url: 'https://ide.agennext.com', region: 'us-ashburn-1', status: 'unknown' },
                    { name: 'arithmetic', url: 'https://arithmetic.agennext.com', region: 'us-ashburn-1', status: 'unknown' },
                ]);
                setError('Infra API not configured. Set INFRA_API_URL in backend env.');
            })
            .finally(() => setLoading(false));
    }, []);

    if (loading) return <div style={{ padding: 16, color: '#555', fontSize: 12 }}>Loading…</div>;

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
            {error && (
                <div style={{ fontSize: 10, color: '#d0a030', background: '#1a1500', border: '1px solid #3a3000', borderRadius: 4, padding: '4px 8px', marginBottom: 4 }}>
                    {error}
                </div>
            )}
            <SectionHeader>Edge Instances</SectionHeader>
            {instances.map(inst => (
                <div key={inst.name} style={{ background: '#141414', border: '1px solid #1e1e1e', borderRadius: 5, padding: '10px 12px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                        <StatusDot status={inst.status} />
                        <span style={{ fontWeight: 700, fontSize: 12 }}>{inst.name}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: '#555' }}>{inst.region}</span>
                    </div>
                    <div style={{ display: 'flex', gap: 12, fontSize: 10, color: '#666' }}>
                        <a href={inst.url} target="_blank" rel="noopener noreferrer" style={{ color: '#4080c0' }}>{inst.url}</a>
                        {inst.ip   && <span>IP: <b style={{ color: '#a0c0e0' }}>{inst.ip}</b></span>}
                        {inst.uptime && <span>Up: <b style={{ color: '#a0c0a0' }}>{inst.uptime}</b></span>}
                        <span style={{ marginLeft: 'auto', color: { healthy: '#40d040', degraded: '#d0a030', offline: '#d04040', unknown: '#555' }[inst.status] }}>{inst.status}</span>
                    </div>
                </div>
            ))}
            <div style={{ fontSize: 10, color: '#333', marginTop: 6, fontStyle: 'italic' }}>
                All inbound through Caddy (443). No raw ports exposed. ingress via gated protocol only.
            </div>
        </div>
    );
}

// ── Peers Tab ─────────────────────────────────────────────────────────────────

function PeersTab() {
    const [peers, setPeers]     = React.useState<Peer[]>([]);
    const [loading, setLoading] = React.useState(true);
    const [name, setName]       = React.useState('');
    const [url, setUrl]         = React.useState('');
    const [adding, setAdding]   = React.useState(false);
    const [error, setError]     = React.useState<string | null>(null);

    const load = () => {
        setLoading(true);
        listPeers()
            .then(setPeers)
            .catch(() => setPeers([]))
            .finally(() => setLoading(false));
    };
    React.useEffect(load, []);

    const add = async () => {
        if (!name.trim() || !url.trim()) return;
        setAdding(true); setError(null);
        try {
            const peer = await addPeer({ name: name.trim(), url: url.trim() });
            setPeers(ps => [...ps, peer]);
            setName(''); setUrl('');
        } catch (e: any) {
            setError(e?.message ?? 'Failed to add peer');
        } finally { setAdding(false); }
    };

    const remove = async (id: string) => {
        try {
            await removePeer(id);
            setPeers(ps => ps.filter(p => p.id !== id));
        } catch { /* ignore */ }
    };

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
            <SectionHeader>Peer IDE Instances</SectionHeader>
            <div style={{ fontSize: 10, color: '#555', marginBottom: 4 }}>
                Peers connect via authenticated WebSocket (wss://). Delivery is egress-only; peers do not open raw ports.
            </div>
            {loading ? (
                <div style={{ color: '#555', fontSize: 12 }}>Loading…</div>
            ) : peers.length === 0 ? (
                <div style={{ color: '#444', fontSize: 12, fontStyle: 'italic' }}>No peers registered.</div>
            ) : (
                peers.map(p => (
                    <div key={p.id} style={{ background: '#141414', border: '1px solid #1e1e1e', borderRadius: 5, padding: '8px 12px', display: 'flex', alignItems: 'center', gap: 8 }}>
                        <StatusDot status={p.status} />
                        <div style={{ flex: 1 }}>
                            <div style={{ fontSize: 12, fontWeight: 600 }}>{p.name}</div>
                            <div style={{ fontSize: 10, color: '#555' }}>{p.url}{p.region ? ` · ${p.region}` : ''}</div>
                            {p.lastSeen && <div style={{ fontSize: 9, color: '#444' }}>last seen {new Date(p.lastSeen).toLocaleTimeString()}</div>}
                        </div>
                        <button onClick={() => remove(p.id)} style={{ padding: '2px 7px', background: '#1a0f0f', border: '1px solid #3a1a1a', color: '#d06060', borderRadius: 3, cursor: 'pointer', fontSize: 10 }}>
                            Remove
                        </button>
                    </div>
                ))
            )}

            <div style={{ marginTop: 8, paddingTop: 8, borderTop: '1px solid #1e1e1e' }}>
                <SectionHeader>Add Peer</SectionHeader>
                {error && <div style={{ color: '#d06060', fontSize: 11, marginBottom: 4 }}>{error}</div>}
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                    <input
                        placeholder="Name (e.g. staging-ide)"
                        value={name}
                        onChange={e => setName(e.target.value)}
                        style={{ background: '#0f0f0f', border: '1px solid #2a2a2a', color: '#ccc', borderRadius: 3, padding: '5px 8px', fontSize: 12 }}
                    />
                    <input
                        placeholder="URL (e.g. https://ide.agennext.com)"
                        value={url}
                        onChange={e => setUrl(e.target.value)}
                        style={{ background: '#0f0f0f', border: '1px solid #2a2a2a', color: '#ccc', borderRadius: 3, padding: '5px 8px', fontSize: 12 }}
                    />
                    <button
                        onClick={add}
                        disabled={adding || !name.trim() || !url.trim()}
                        style={{ padding: '5px 12px', background: '#0f1a0f', border: '1px solid #1a3a1a', color: '#60d060', borderRadius: 3, cursor: 'pointer', fontSize: 12, alignSelf: 'flex-start' }}
                    >
                        {adding ? 'Adding…' : '+ Add Peer'}
                    </button>
                </div>
            </div>
        </div>
    );
}

// ── Transfer Tab ──────────────────────────────────────────────────────────────

function TransferTab() {
    const [agents, setAgents]     = React.useState<AgentRef[]>([]);
    const [peers, setPeers]       = React.useState<Peer[]>([]);
    const [agentId, setAgentId]   = React.useState('');
    const [peerId, setPeerId]     = React.useState('');
    const [transferring, setTransferring] = React.useState(false);
    const [result, setResult]     = React.useState<{ ok: boolean; message: string } | null>(null);
    const [error, setError]       = React.useState<string | null>(null);

    React.useEffect(() => {
        listAgents().then(setAgents).catch(() => setAgents([]));
        listPeers().then(setPeers).catch(() => setPeers([]));
    }, []);

    const transfer = async () => {
        if (!agentId || !peerId) return;
        setTransferring(true); setResult(null); setError(null);
        try {
            const res = await transferAgent(agentId, peerId);
            setResult(res);
        } catch (e: any) {
            setError(e?.message ?? 'Transfer failed');
        } finally { setTransferring(false); }
    };

    return (
        <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 10 }}>
            <SectionHeader>Agent Teleportation</SectionHeader>
            <div style={{ fontSize: 10, color: '#555', marginBottom: 4 }}>
                Serializes agent state + identity and delivers to peer via authenticated WebSocket.
                Peer receives on <code style={{ color: '#888' }}>/transfer/receive</code> (gated, requires API key).
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <div>
                    <div style={{ fontSize: 10, color: '#666', marginBottom: 3 }}>Source Agent</div>
                    <select
                        value={agentId}
                        onChange={e => setAgentId(e.target.value)}
                        style={{ width: '100%', background: '#0f0f0f', border: '1px solid #2a2a2a', color: '#ccc', borderRadius: 3, padding: '5px 8px', fontSize: 12 }}
                    >
                        <option value="">Select agent…</option>
                        {agents.map(a => <option key={a.id} value={a.id}>{a.name} ({a.model})</option>)}
                    </select>
                </div>

                <div style={{ textAlign: 'center', color: '#444', fontSize: 16 }}>↓</div>

                <div>
                    <div style={{ fontSize: 10, color: '#666', marginBottom: 3 }}>Destination Peer</div>
                    <select
                        value={peerId}
                        onChange={e => setPeerId(e.target.value)}
                        style={{ width: '100%', background: '#0f0f0f', border: '1px solid #2a2a2a', color: '#ccc', borderRadius: 3, padding: '5px 8px', fontSize: 12 }}
                    >
                        <option value="">Select peer…</option>
                        {peers.map(p => <option key={p.id} value={p.id} disabled={p.status === 'offline'}>{p.name} ({p.status})</option>)}
                    </select>
                </div>

                {peers.length === 0 && (
                    <div style={{ fontSize: 11, color: '#555', fontStyle: 'italic' }}>
                        No peers registered. Add peers in the Peers tab first.
                    </div>
                )}

                <button
                    onClick={transfer}
                    disabled={transferring || !agentId || !peerId}
                    style={{
                        marginTop: 4, padding: '8px 16px',
                        background: transferring ? '#141414' : '#0a1a2a',
                        border: '1px solid #1a3a5a', color: '#60b0ff',
                        borderRadius: 4, cursor: 'pointer', fontSize: 13, fontWeight: 700,
                        alignSelf: 'flex-start',
                    }}
                >
                    {transferring ? '⇢ Teleporting…' : '⇢ Teleport Agent'}
                </button>

                {result && (
                    <div style={{
                        background: result.ok ? '#0a1a0a' : '#1a0a0a',
                        border: `1px solid ${result.ok ? '#1a4a1a' : '#4a1a1a'}`,
                        color: result.ok ? '#60d060' : '#d06060',
                        borderRadius: 4, padding: '8px 12px', fontSize: 12,
                    }}>
                        {result.ok ? '✓ ' : '✗ '}{result.message}
                    </div>
                )}
                {error && (
                    <div style={{ background: '#1a0a0a', border: '1px solid #4a1a1a', color: '#d06060', borderRadius: 4, padding: '8px 12px', fontSize: 12 }}>
                        ✗ {error}
                    </div>
                )}
            </div>

            <div style={{ marginTop: 8, paddingTop: 8, borderTop: '1px solid #1a1a1a', fontSize: 10, color: '#333', lineHeight: 1.6 }}>
                <b style={{ color: '#444' }}>Protocol:</b> POST wss://&lt;peer&gt;/transfer/receive
                · payload: serialized AgentIdentity + memory snapshot
                · auth: Bearer API key (set TRANSFER_API_KEY on both sides)
                · direction: egress-push only — peer never opens raw TCP to caller
            </div>
        </div>
    );
}

// ── Main widget ───────────────────────────────────────────────────────────────

function MetaView() {
    const [tab, setTab] = React.useState<MetaTab>('infra');
    const TABS: { id: MetaTab; label: string }[] = [
        { id: 'infra',    label: 'Infra' },
        { id: 'peers',    label: 'Peers' },
        { id: 'transfer', label: 'Transfer' },
    ];
    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontWeight: 700, fontSize: 14, color: '#c080ff' }}>⬡ Meta</span>
                <span style={{ fontSize: 10, color: '#555' }}>infra · peers · teleportation</span>
            </div>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {TABS.map(t => (
                    <button key={t.id} onClick={() => setTab(t.id)} style={{
                        padding: '6px 16px', border: 'none', background: 'transparent',
                        color: tab === t.id ? '#c080ff' : '#555',
                        borderBottom: tab === t.id ? '2px solid #c080ff' : '2px solid transparent',
                        cursor: 'pointer', fontSize: 12, fontWeight: tab === t.id ? 700 : 400,
                    }}>{t.label}</button>
                ))}
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {tab === 'infra'    && <InfraTab />}
                {tab === 'peers'    && <PeersTab />}
                {tab === 'transfer' && <TransferTab />}
            </div>
        </div>
    );
}

@injectable()
export class MetaPanelWidget extends ReactWidget {
    static readonly ID    = 'agent-ide:meta';
    static readonly LABEL = 'Meta';
    @postConstruct() protected init(): void {
        this.id = MetaPanelWidget.ID; this.title.label = MetaPanelWidget.LABEL;
        this.title.caption = 'Infra control plane · peer teleportation'; this.title.closable = true;
        this.title.iconClass = 'codicon codicon-broadcast'; this.update();
    }
    protected render(): React.ReactNode { return <MetaView />; }
}

@injectable()
export class MetaPanelContribution extends AbstractViewContribution<MetaPanelWidget> {
    constructor() {
        super({ widgetId: MetaPanelWidget.ID, widgetName: MetaPanelWidget.LABEL, defaultWidgetOptions: { area: 'main' }, toggleCommandId: MetaPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(MetaPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
