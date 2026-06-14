// Metal Panel — Autonomyx hardware control surface.
// Faces: Metal (device identity + HSM), Registry (Zot OCI), Build (Stacker SI),
//        Deliver (multi-k8s cluster status), Worlds (agent deployment targets).
//
// This IS the meta surface: everything about the physical device that hosts
// the platform is visible and controllable from this panel.

import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';

// ── Types ─────────────────────────────────────────────────────────────────────

type MetalFace = 'metal' | 'registry' | 'build' | 'deliver' | 'worlds';

interface DeviceIdentity {
    did:          string;
    keyType:      string;
    storage:      string;
    rotatesAt:    string;
    status:       'valid' | 'rotating' | 'expired';
}

interface ZotImage {
    repo:     string;
    tag:      string;
    digest:   string;
    size:     string;
    signed:   boolean;
    pushedAt: string;
}

interface BuildJob {
    id:        string;
    layer:     string;
    status:    'pending' | 'building' | 'done' | 'failed';
    startedAt: string;
    duration:  string;
    hash:      string;
}

interface ClusterStatus {
    name:     string;
    region:   string;
    status:   'synced' | 'syncing' | 'outOfSync' | 'failed';
    app:      string;
    revision: string;
    lastSync: string;
}

interface WorldStatus {
    name:    string;
    target:  string;
    runtime: string;
    status:  'running' | 'stopped' | 'deploying' | 'error';
    agent:   string;
    region:  string;
}

// ── Demo data ─────────────────────────────────────────────────────────────────

const DEMO_DEVICE: DeviceIdentity = {
    did:       'did:autonomyx:8xKvTpMnQrZ7bYwAjLsEuCfHdNgO3mWi',
    keyType:   'ed25519',
    storage:   'hsm',
    rotatesAt: '2026-09-14',
    status:    'valid',
};

const DEMO_IMAGES: ZotImage[] = [
    { repo: 'agennext/agent-ide', tag: 'main',  digest: 'sha256:4a7f2c...', size: '128MB', signed: true,  pushedAt: '2m ago' },
    { repo: 'agennext/agent-ide', tag: 'v0.1.0',digest: 'sha256:2b3d9e...', size: '127MB', signed: true,  pushedAt: '1d ago' },
    { repo: 'agennext/runner',    tag: 'main',  digest: 'sha256:9c1a4f...', size: '12MB',  signed: true,  pushedAt: '2m ago' },
    { repo: 'agennext/runner',    tag: 'arm64', digest: 'sha256:5e8b2a...', size: '11MB',  signed: true,  pushedAt: '2m ago' },
];

const DEMO_BUILDS: BuildJob[] = [
    { id: 'b-001', layer: 'autonomyx',    status: 'done',     startedAt: '14:30:00', duration: '4m 12s', hash: 'sha256:4a7f2c...' },
    { id: 'b-002', layer: 'rust-build',  status: 'done',     startedAt: '14:25:48', duration: '6m 08s', hash: 'sha256:9c1a4f...' },
    { id: 'b-003', layer: 'node-build',  status: 'done',     startedAt: '14:22:10', duration: '3m 38s', hash: 'sha256:7d3e1b...' },
    { id: 'b-004', layer: 'base-alpine', status: 'done',     startedAt: '14:20:00', duration: '0m 42s', hash: 'sha256:2f8c5a...' },
];

const DEMO_CLUSTERS: ClusterStatus[] = [
    { name: 'k8s-eu-prod',  region: 'EU West',  status: 'synced',     app: 'autonomyx', revision: '4990c33', lastSync: '2m ago' },
    { name: 'k8s-us-prod',  region: 'US East',  status: 'synced',     app: 'autonomyx', revision: '4990c33', lastSync: '2m ago' },
    { name: 'k8s-ap-prod',  region: 'AP SE',    status: 'syncing',    app: 'autonomyx', revision: '4990c33', lastSync: '30s ago' },
    { name: 'k8s-onprem',   region: 'On-Prem',  status: 'outOfSync',  app: 'autonomyx', revision: '998e7c2', lastSync: '1h ago' },
];

const DEMO_WORLDS: WorldStatus[] = [
    { name: 'server-world',  target: 'server',  runtime: 'rust',     status: 'running',   agent: 'DeepThinkAgent', region: 'eu-west-1' },
    { name: 'edge-world',    target: 'edge',    runtime: 'rust',     status: 'running',   agent: 'ResearchAgent',  region: 'global' },
    { name: 'browser-world', target: 'browser', runtime: 'wasm',     status: 'running',   agent: 'ResearchAgent',  region: 'client' },
    { name: 'k8s-prod',      target: 'k8s',     runtime: 'rust',     status: 'deploying', agent: 'DeepThinkAgent', region: 'eu+us+ap' },
    { name: 'desktop-theia', target: 'desktop', runtime: 'node',     status: 'running',   agent: 'LocalAgent',     region: 'local' },
    { name: 'mobile-ios',    target: 'mobile',  runtime: 'ios',      status: 'stopped',   agent: '-',              region: 'device' },
    { name: 'embedded-esp32',target: 'embedded',runtime: 'bare',     status: 'stopped',   agent: '-',              region: 'device' },
];

// ── Panel widget ──────────────────────────────────────────────────────────────

export const METAL_PANEL_ID = 'agent-ide:metal';

@injectable()
export class MetalPanelWidget extends ReactWidget {
    static readonly ID    = METAL_PANEL_ID;
    static readonly LABEL = 'Metal';

    private face: MetalFace = 'metal';

    @postConstruct()
    protected init(): void {
        this.id            = MetalPanelWidget.ID;
        this.title.label   = 'Metal';
        this.title.caption = 'Autonomyx Metal — Device, Registry, SI Builds, Delivery';
        this.title.iconClass = 'codicon codicon-server-environment';
        this.title.closable = true;
        this.update();
    }

    private setFace = (f: MetalFace) => { this.face = f; this.update(); };

    protected render(): React.ReactNode {
        const faces: { id: MetalFace; label: string; icon: string }[] = [
            { id: 'metal',    label: 'Metal',    icon: 'codicon-circuit-board' },
            { id: 'registry', label: 'Registry', icon: 'codicon-package' },
            { id: 'build',    label: 'Build',    icon: 'codicon-tools' },
            { id: 'deliver',  label: 'Deliver',  icon: 'codicon-rocket' },
            { id: 'worlds',   label: 'Worlds',   icon: 'codicon-globe' },
        ];

        return (
            <div style={{ height: '100%', display: 'flex', flexDirection: 'column', fontFamily: 'var(--theia-code-font-family)', fontSize: 12 }}>
                {/* Face selector */}
                <div style={{ display: 'flex', borderBottom: '1px solid var(--theia-border-color)', padding: '4px 8px', gap: 4 }}>
                    {faces.map(f => (
                        <button
                            key={f.id}
                            onClick={() => this.setFace(f.id)}
                            style={{
                                background:  this.face === f.id ? 'var(--theia-button-background)' : 'transparent',
                                color:       this.face === f.id ? 'var(--theia-button-foreground)' : 'var(--theia-foreground)',
                                border:      'none',
                                borderRadius: 3,
                                padding:     '3px 8px',
                                cursor:      'pointer',
                                fontSize:    11,
                                display:     'flex',
                                alignItems:  'center',
                                gap:         4,
                            }}
                        >
                            <span className={`codicon ${f.icon}`} style={{ fontSize: 11 }} />
                            {f.label}
                        </button>
                    ))}
                </div>

                {/* Content */}
                <div style={{ flex: 1, overflow: 'auto', padding: 8 }}>
                    {this.face === 'metal'    && this.renderMetal()}
                    {this.face === 'registry' && this.renderRegistry()}
                    {this.face === 'build'    && this.renderBuild()}
                    {this.face === 'deliver'  && this.renderDeliver()}
                    {this.face === 'worlds'   && this.renderWorlds()}
                </div>
            </div>
        );
    }

    // ── Metal face — device identity ─────────────────────────────────────────

    private renderMetal(): React.ReactNode {
        const d = DEMO_DEVICE;
        const statusColor = { valid: '#40d040', rotating: '#d0a030', expired: '#d04040' }[d.status];
        return (
            <div>
                <div style={{ color: 'var(--theia-foreground)', marginBottom: 12, fontWeight: 'bold' }}>
                    <span className="codicon codicon-circuit-board" style={{ marginRight: 6 }} />
                    Device Identity
                </div>
                <div style={{ background: '#0a1a2a', border: '1px solid #1a3a5a', borderRadius: 4, padding: 12, marginBottom: 10 }}>
                    <div style={{ color: '#80b0ff', fontSize: 10, marginBottom: 4 }}>DID</div>
                    <div style={{ color: '#c0c0c0', fontFamily: 'monospace', fontSize: 10, wordBreak: 'break-all' }}>{d.did}</div>
                </div>
                {[
                    ['Key Type',     d.keyType,   '#60b0ff'],
                    ['Storage',      d.storage,   '#60d0a0'],
                    ['Rotates At',   d.rotatesAt, '#c0c0c0'],
                ].map(([k, v, c]) => (
                    <div key={k as string} style={{ display: 'flex', justifyContent: 'space-between', padding: '4px 0', borderBottom: '1px solid #1a1a1a' }}>
                        <span style={{ color: '#555' }}>{k}</span>
                        <span style={{ color: c as string }}>{v}</span>
                    </div>
                ))}
                <div style={{ display: 'flex', justifyContent: 'space-between', padding: '4px 0', marginTop: 4 }}>
                    <span style={{ color: '#555' }}>Status</span>
                    <span style={{ color: statusColor, fontWeight: 'bold' }}>{d.status.toUpperCase()}</span>
                </div>
                <div style={{ marginTop: 12, display: 'flex', gap: 6 }}>
                    <button style={btnStyle('#1a3a2a', '#40d040')}>Rotate Key</button>
                    <button style={btnStyle('#1a2a3a', '#60b0ff')}>Export Public Key</button>
                    <button style={btnStyle('#2a1a1a', '#d04040')}>Revoke</button>
                </div>
            </div>
        );
    }

    // ── Registry face — Zot OCI ──────────────────────────────────────────────

    private renderRegistry(): React.ReactNode {
        return (
            <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
                    <span style={{ color: 'var(--theia-foreground)', fontWeight: 'bold' }}>
                        <span className="codicon codicon-package" style={{ marginRight: 6 }} />
                        Zot Registry (self-hosted)
                    </span>
                    <span style={{ color: '#40d040', fontSize: 10 }}>● online</span>
                </div>
                {DEMO_IMAGES.map(img => (
                    <div key={img.digest} style={{ background: '#111', border: '1px solid #222', borderRadius: 4, padding: 8, marginBottom: 6 }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                            <span style={{ color: '#80b0ff', fontFamily: 'monospace', fontSize: 10 }}>{img.repo}:{img.tag}</span>
                            <span style={{ color: img.signed ? '#40d040' : '#d04040', fontSize: 10 }}>
                                {img.signed ? '✓ signed' : '✗ unsigned'}
                            </span>
                        </div>
                        <div style={{ color: '#444', fontFamily: 'monospace', fontSize: 9, marginTop: 2 }}>{img.digest}</div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, color: '#666', fontSize: 10 }}>
                            <span>{img.size}</span>
                            <span>{img.pushedAt}</span>
                        </div>
                    </div>
                ))}
                <div style={{ marginTop: 8, display: 'flex', gap: 6 }}>
                    <button style={btnStyle('#1a2a3a', '#60b0ff')}>Open UI</button>
                    <button style={btnStyle('#1a3a2a', '#40d040')}>GC</button>
                </div>
            </div>
        );
    }

    // ── Build face — Stacker SI ──────────────────────────────────────────────

    private renderBuild(): React.ReactNode {
        const statusColor = { done: '#40d040', building: '#d0a030', pending: '#555', failed: '#d04040' };
        return (
            <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
                    <span style={{ color: 'var(--theia-foreground)', fontWeight: 'bold' }}>
                        <span className="codicon codicon-tools" style={{ marginRight: 6 }} />
                        Stacker SI Builds
                    </span>
                    <button style={btnStyle('#1a3a2a', '#40d040')} onClick={() => {}}>▶ Build</button>
                </div>
                {DEMO_BUILDS.map(b => (
                    <div key={b.id} style={{ background: '#111', border: '1px solid #222', borderRadius: 4, padding: 8, marginBottom: 6 }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                            <span style={{ color: '#80b0ff', fontFamily: 'monospace', fontSize: 10 }}>{b.layer}</span>
                            <span style={{ color: statusColor[b.status], fontWeight: 'bold', fontSize: 10 }}>{b.status.toUpperCase()}</span>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, color: '#555', fontSize: 10 }}>
                            <span>{b.startedAt}</span>
                            <span>{b.duration}</span>
                            <span style={{ fontFamily: 'monospace', color: '#333' }}>{b.hash}</span>
                        </div>
                    </div>
                ))}
            </div>
        );
    }

    // ── Deliver face — multi-cluster ArgoCD ──────────────────────────────────

    private renderDeliver(): React.ReactNode {
        const statusColor = { synced: '#40d040', syncing: '#d0a030', outOfSync: '#d08030', failed: '#d04040' };
        return (
            <div>
                <div style={{ color: 'var(--theia-foreground)', fontWeight: 'bold', marginBottom: 10 }}>
                    <span className="codicon codicon-rocket" style={{ marginRight: 6 }} />
                    Multi-Cluster Delivery (ArgoCD)
                </div>
                {DEMO_CLUSTERS.map(c => (
                    <div key={c.name} style={{ background: '#111', border: '1px solid #222', borderRadius: 4, padding: 8, marginBottom: 6 }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                            <div>
                                <span style={{ color: '#c0c0c0', fontFamily: 'monospace', fontSize: 11 }}>{c.name}</span>
                                <span style={{ color: '#444', fontSize: 10, marginLeft: 8 }}>{c.region}</span>
                            </div>
                            <span style={{ color: statusColor[c.status], fontWeight: 'bold', fontSize: 10 }}>{c.status}</span>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, color: '#555', fontSize: 10 }}>
                            <span style={{ fontFamily: 'monospace' }}>{c.revision}</span>
                            <span>{c.lastSync}</span>
                            <button style={{ ...btnStyle('#1a2a3a', '#60b0ff'), padding: '1px 6px', fontSize: 9 }}>Sync</button>
                        </div>
                    </div>
                ))}
                <div style={{ marginTop: 8 }}>
                    <button style={btnStyle('#1a2a3a', '#60b0ff')}>Sync All</button>
                </div>
            </div>
        );
    }

    // ── Worlds face — agent deployment targets ────────────────────────────────

    private renderWorlds(): React.ReactNode {
        const statusColor = { running: '#40d040', stopped: '#555', deploying: '#d0a030', error: '#d04040' };
        const targetIcon: Record<string, string> = {
            server: 'codicon-server', edge: 'codicon-globe', browser: 'codicon-browser',
            k8s: 'codicon-layers', desktop: 'codicon-window', mobile: 'codicon-device-mobile',
            embedded: 'codicon-circuit-board',
        };
        return (
            <div>
                <div style={{ color: 'var(--theia-foreground)', fontWeight: 'bold', marginBottom: 10 }}>
                    <span className="codicon codicon-globe" style={{ marginRight: 6 }} />
                    Agent Worlds
                </div>
                {DEMO_WORLDS.map(w => (
                    <div key={w.name} style={{ background: '#111', border: '1px solid #222', borderRadius: 4, padding: 8, marginBottom: 6 }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                                <span className={`codicon ${targetIcon[w.target] ?? 'codicon-circle'}`} style={{ color: '#80b0ff' }} />
                                <span style={{ color: '#c0c0c0', fontSize: 11 }}>{w.name}</span>
                            </div>
                            <span style={{ color: statusColor[w.status], fontSize: 10 }}>● {w.status}</span>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, color: '#555', fontSize: 10 }}>
                            <span>{w.runtime}</span>
                            <span style={{ color: '#404040' }}>{w.agent}</span>
                            <span>{w.region}</span>
                        </div>
                    </div>
                ))}
            </div>
        );
    }
}

function btnStyle(bg: string, color: string): React.CSSProperties {
    return {
        background: bg, color, border: `1px solid ${color}`,
        borderRadius: 3, padding: '3px 10px', cursor: 'pointer', fontSize: 10,
    };
}

// ── Theia contribution ────────────────────────────────────────────────────────

export const MetalPanelCommand = { id: 'agentIde.openMetal', label: 'Open Metal Panel', category: 'Autonomyx' };

@injectable()
export class MetalPanelContribution extends AbstractViewContribution<MetalPanelWidget> {
    constructor() {
        super({
            widgetId:       MetalPanelWidget.ID,
            widgetName:     MetalPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'bottom' },
            toggleCommandId: MetalPanelCommand.id,
        });
    }

    registerCommands(registry: CommandRegistry): void {
        super.registerCommands(registry);
        registry.registerCommand(MetalPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
