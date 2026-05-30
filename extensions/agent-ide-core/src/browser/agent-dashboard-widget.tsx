import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';

interface StatCard { label: string; value: string | number; icon: string; color: string; }
interface PanelLink { id: string; label: string; desc: string; icon: string; area: string; }

const STATS: StatCard[] = [
    { label: 'Agents',      value: 4,          icon: '◉', color: '#7ab4ff' },
    { label: 'Tasks',       value: 12,         icon: '☑', color: '#60d060' },
    { label: 'Runs',        value: 37,         icon: '▶', color: '#d0a030' },
    { label: 'Artifacts',   value: 11,         icon: '⬡', color: '#c080ff' },
    { label: 'Governance',  value: 'Active',   icon: '⚖', color: '#40c0a0' },
    { label: 'Policies',    value: 3,          icon: '📋', color: '#f06040' },
];

const PANELS: PanelLink[] = [
    { id: 'agents',     label: 'Agents',        desc: 'Define and configure agent models',       icon: '◉', area: 'left' },
    { id: 'tasks',      label: 'Tasks',         desc: 'Manage task queue and assignments',        icon: '☑', area: 'left' },
    { id: 'knowledge',  label: 'Knowledge',     desc: 'Browse and query the knowledge base',     icon: '◈', area: 'left' },
    { id: 'artifacts',  label: 'Artifacts',     desc: 'Inspect all 12 artifact types',           icon: '⬡', area: 'left' },
    { id: 'runs',       label: 'Runs',          desc: 'Trace, token flow, and performance',      icon: '▶', area: 'bottom' },
    { id: 'replay',     label: 'Replay',        desc: 'Step-through trace replay viewer',        icon: '⏮', area: 'bottom' },
    { id: 'governance', label: 'Governance',    desc: 'Policies, rules, and audit logs',         icon: '⚖', area: 'right' },
    { id: 'builder',    label: 'Agent Builder', desc: 'Visual workflow graph editor',            icon: '⬡', area: 'main' },
    { id: 'platform',   label: 'Platform',      desc: 'FinOps, traces, and monitoring',          icon: '⊞', area: 'main' },
    { id: 'bench',      label: 'Bench',         desc: 'AgentBench evaluation harness',           icon: '⚗', area: 'main' },
    { id: 'optimize',   label: 'Optimize',      desc: 'Prompt, cost, and latency tuning',        icon: '▲', area: 'main' },
    { id: 'research',   label: 'Research',      desc: 'Multi-source research orchestration',     icon: '🔬', area: 'main' },
];

const ACTIVITY = [
    { time: '14:22', agent: 'ResearchAgent',  task: 'Market analysis Q2',       status: 'completed', tokens: 8721 },
    { time: '14:08', agent: 'CoderAgent',     task: 'Refactor orchestrator.py', status: 'completed', tokens: 4312 },
    { time: '13:55', agent: 'AnalystAgent',   task: 'Evaluation report',        status: 'in_progress', tokens: 6180 },
    { time: '13:30', agent: 'WriterAgent',    task: 'Architecture decisions',   status: 'completed', tokens: 3540 },
    { time: '13:01', agent: 'ResearchAgent',  task: 'Competitive landscape',    status: 'failed',    tokens: 1220 },
];

const STATUS_COLORS: Record<string, string> = {
    completed: '#40a040', in_progress: '#4080d0', failed: '#c04040', pending: '#808040',
};

function StatCards() {
    return (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 10, padding: '16px 16px 0' }}>
            {STATS.map(s => (
                <div key={s.label} style={{ background: '#111', border: '1px solid #222', borderRadius: 6, padding: '10px 14px', display: 'flex', alignItems: 'center', gap: 10 }}>
                    <span style={{ fontSize: 20, color: s.color }}>{s.icon}</span>
                    <div>
                        <div style={{ fontSize: 18, fontWeight: 700, color: s.color, lineHeight: 1 }}>{s.value}</div>
                        <div style={{ fontSize: 11, color: '#666', marginTop: 2 }}>{s.label}</div>
                    </div>
                </div>
            ))}
        </div>
    );
}

function PanelGrid() {
    return (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 8, padding: '12px 16px' }}>
            {PANELS.map(p => (
                <div key={p.id} style={{ background: '#111', border: '1px solid #1e1e1e', borderRadius: 6, padding: '10px 12px', cursor: 'default' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                        <span style={{ fontSize: 14, color: '#7ab4ff' }}>{p.icon}</span>
                        <span style={{ fontWeight: 600, fontSize: 12 }}>{p.label}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: '#444', fontFamily: 'monospace' }}>{p.area}</span>
                    </div>
                    <div style={{ fontSize: 11, color: '#666', lineHeight: 1.4 }}>{p.desc}</div>
                </div>
            ))}
        </div>
    );
}

function ActivityLog() {
    return (
        <div style={{ padding: '0 16px 16px' }}>
            <div style={{ fontSize: 11, fontWeight: 700, color: '#555', letterSpacing: 1, marginBottom: 8 }}>RECENT ACTIVITY</div>
            <div style={{ border: '1px solid #1e1e1e', borderRadius: 6, overflow: 'hidden' }}>
                {ACTIVITY.map((a, i) => (
                    <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '7px 12px', borderBottom: i < ACTIVITY.length - 1 ? '1px solid #1a1a1a' : 'none', background: '#0d0d0d' }}>
                        <span style={{ fontSize: 11, color: '#555', fontFamily: 'monospace', width: 38 }}>{a.time}</span>
                        <span style={{ fontSize: 11, color: '#7ab4ff', width: 120 }}>{a.agent}</span>
                        <span style={{ fontSize: 11, color: '#ccc', flex: 1 }}>{a.task}</span>
                        <span style={{ fontSize: 10, color: '#555' }}>{a.tokens.toLocaleString()} tok</span>
                        <span style={{ fontSize: 10, fontWeight: 700, color: STATUS_COLORS[a.status] ?? '#888', width: 80, textAlign: 'right' }}>{a.status}</span>
                    </div>
                ))}
            </div>
        </div>
    );
}

function DashboardView() {
    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'auto', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ padding: '14px 16px 8px', borderBottom: '1px solid #1e1e1e' }}>
                <div style={{ fontSize: 16, fontWeight: 700, color: '#e0e0e0' }}>Agent IDE</div>
                <div style={{ fontSize: 11, color: '#555', marginTop: 2 }}>Agent Workspace OS — Eclipse Theia</div>
            </div>
            <StatCards />
            <div style={{ padding: '12px 16px 4px', fontSize: 11, fontWeight: 700, color: '#555', letterSpacing: 1 }}>PANELS</div>
            <PanelGrid />
            <ActivityLog />
        </div>
    );
}

@injectable()
export class AgentDashboardWidget extends ReactWidget {
    static readonly ID = 'agent-ide:dashboard';
    static readonly LABEL = 'Agent Dashboard';

    @postConstruct()
    protected init(): void {
        this.id = AgentDashboardWidget.ID;
        this.title.label = AgentDashboardWidget.LABEL;
        this.title.caption = 'Agent Workspace OS overview';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-dashboard';
        this.update();
    }

    protected render(): React.ReactNode {
        return <DashboardView />;
    }
}
