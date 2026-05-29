import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import * as React from 'react';

@injectable()
export class AgentDashboardWidget extends ReactWidget {
    static readonly ID = 'agent-ide:dashboard';
    static readonly LABEL = 'Agent Dashboard';

    @postConstruct()
    protected init(): void {
        this.id = AgentDashboardWidget.ID;
        this.title.label = AgentDashboardWidget.LABEL;
        this.title.caption = 'Agent Workspace OS — Dashboard';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-server-process';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <div className="agent-dashboard">
                <div className="agent-dashboard__header">
                    <h1 className="agent-dashboard__title">Agent Workspace OS</h1>
                    <p className="agent-dashboard__subtitle">
                        Active Workspace &nbsp;&middot;&nbsp; {new Date().toLocaleDateString()}
                    </p>
                </div>

                <div className="agent-dashboard__stats">
                    <StatCard icon="codicon-robot" label="Agents" value={0} />
                    <StatCard icon="codicon-checklist" label="Tasks" value={0} />
                    <StatCard icon="codicon-play-circle" label="Runs" value={0} />
                    <StatCard icon="codicon-package" label="Artifacts" value={0} />
                </div>

                <div className="agent-dashboard__governance">
                    <span className="agent-dashboard__gov-badge agent-dashboard__gov-badge--active">
                        <span className="codicon codicon-shield" /> Governance &middot; Active
                    </span>
                </div>

                <div className="agent-dashboard__panels">
                    <h2 className="agent-dashboard__section-title">Panels</h2>
                    <ul className="agent-dashboard__panel-list">
                        {PANEL_REGISTRY.map(p => (
                            <li key={p.id} className="agent-dashboard__panel-item">
                                <span className={`codicon ${p.icon}`} />
                                <span>{p.label}</span>
                                <span className="agent-dashboard__panel-hint">{p.hint}</span>
                            </li>
                        ))}
                    </ul>
                </div>
            </div>
        );
    }
}

interface StatCardProps { icon: string; label: string; value: number; }
const StatCard: React.FC<StatCardProps> = ({ icon, label, value }) => (
    <div className="agent-dashboard__stat">
        <span className={`codicon ${icon} agent-dashboard__stat-icon`} />
        <span className="agent-dashboard__stat-value">{value}</span>
        <span className="agent-dashboard__stat-label">{label}</span>
    </div>
);

const PANEL_REGISTRY = [
    { id: 'agents',     label: 'Agents',     icon: 'codicon-robot',        hint: 'Manage AI agents' },
    { id: 'tasks',      label: 'Tasks',      icon: 'codicon-checklist',    hint: 'Track work items' },
    { id: 'knowledge',  label: 'Knowledge',  icon: 'codicon-book',         hint: 'Workspace knowledge base' },
    { id: 'artifacts',  label: 'Artifacts',  icon: 'codicon-package',      hint: 'Agent-produced outputs' },
    { id: 'runs',       label: 'Runs',       icon: 'codicon-play-circle',  hint: 'Execution history' },
    { id: 'replay',     label: 'Replay',     icon: 'codicon-history',      hint: 'Step through past runs' },
    { id: 'governance', label: 'Governance', icon: 'codicon-shield',       hint: 'Policies and guardrails' },
    { id: 'builder',    label: 'Builder',    icon: 'codicon-circuit-board', hint: 'Visual agent designer' },
];
