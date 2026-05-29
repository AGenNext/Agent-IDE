import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { GovernancePolicy, GovernanceRule, GovernanceAction } from '@agennext/agent-ide-types';
import { GovernancePanelCommand } from '../agent-ide-commands';

type GovTab = 'policies' | 'audit' | 'approvals';

function uid(): string { return Math.random().toString(36).slice(2, 9); }

const ACTION_META: Record<GovernanceAction, { label: string; color: string; bg: string }> = {
    allow:            { label: 'ALLOW',    color: '#40a040', bg: '#0a1a0a' },
    deny:             { label: 'DENY',     color: '#c04040', bg: '#1a0a0a' },
    require_approval: { label: 'APPROVE',  color: '#d0a030', bg: '#1a1400' },
    audit_only:       { label: 'AUDIT',    color: '#5090d0', bg: '#0a1420' },
};

const DEMO_POLICIES: GovernancePolicy[] = [
    {
        id: 'p1', name: 'Tool Execution Policy', description: 'Controls which tools agents may invoke and under what conditions.',
        scope: 'global', enabled: true,
        createdAt: '2026-05-20T09:00:00Z', updatedAt: '2026-05-28T14:00:00Z',
        rules: [
            { id: 'r1', policyId: 'p1', condition: 'tool.id == "shell" && agent.trustLevel < 3', action: 'require_approval', rationale: 'Shell execution by untrusted agents requires human sign-off.', priority: 1 },
            { id: 'r2', policyId: 'p1', condition: 'tool.id in ["browser", "http_client"] && task.type == "research"', action: 'allow', rationale: 'Web access allowed for research tasks.', priority: 2 },
            { id: 'r3', policyId: 'p1', condition: 'tool.id == "file_rw" && file.path starts_with "/etc"', action: 'deny', rationale: 'System config files must never be modified by agents.', priority: 3 },
        ],
    },
    {
        id: 'p2', name: 'Token Budget Policy', description: 'Enforces per-run and per-agent token consumption limits.',
        scope: 'workspace', enabled: true,
        createdAt: '2026-05-21T10:00:00Z', updatedAt: '2026-05-27T12:00:00Z',
        rules: [
            { id: 'r4', policyId: 'p2', condition: 'run.totalInputTokens > 100000', action: 'require_approval', rationale: 'Large context runs flagged for cost review.', priority: 1 },
            { id: 'r5', policyId: 'p2', condition: 'run.estimatedCostUsd > 1.00', action: 'require_approval', rationale: 'Runs costing more than $1 require approval.', priority: 2 },
            { id: 'r6', policyId: 'p2', condition: 'run.totalInputTokens > 200000', action: 'deny', rationale: 'Hard limit: no single run may exceed 200K input tokens.', priority: 3 },
        ],
    },
    {
        id: 'p3', name: 'Data Handling Policy', description: 'Governs how agents handle PII and sensitive workspace artifacts.',
        scope: 'agent', enabled: false,
        createdAt: '2026-05-22T08:00:00Z', updatedAt: '2026-05-22T08:00:00Z',
        rules: [
            { id: 'r7', policyId: 'p3', condition: 'artifact.metadata.contains_pii == true', action: 'audit_only', rationale: 'PII artifacts logged for compliance audit trail.', priority: 1 },
        ],
    },
];

const AUDIT_LOG = [
    { id: uid(), ts: '14:22:08', agent: 'ResearchAgent',  action: 'tool.browser', decision: 'allow' as GovernanceAction,            policy: 'Tool Execution Policy', detail: 'research task — auto-allowed' },
    { id: uid(), ts: '14:08:31', agent: 'CoderAgent',     action: 'tool.file_rw', decision: 'allow' as GovernanceAction,            policy: 'Tool Execution Policy', detail: 'path /workspace/src — permitted' },
    { id: uid(), ts: '13:55:12', agent: 'AnalystAgent',   action: 'run.start',    decision: 'require_approval' as GovernanceAction, policy: 'Token Budget Policy',   detail: 'est. 112K tokens — awaiting approval' },
    { id: uid(), ts: '13:30:44', agent: 'WriterAgent',    action: 'tool.http_client', decision: 'allow' as GovernanceAction,        policy: 'Tool Execution Policy', detail: 'research task — auto-allowed' },
    { id: uid(), ts: '13:01:19', agent: 'ResearchAgent',  action: 'run.start',    decision: 'deny' as GovernanceAction,             policy: 'Token Budget Policy',   detail: 'est. 210K tokens — hard limit exceeded' },
    { id: uid(), ts: '12:45:02', agent: 'CoderAgent',     action: 'tool.shell',   decision: 'require_approval' as GovernanceAction, policy: 'Tool Execution Policy', detail: 'trustLevel 2 — human review needed' },
];

const PENDING_APPROVALS = [
    { id: uid(), ts: '13:55:12', agent: 'AnalystAgent',  action: 'run.start',   policy: 'Token Budget Policy',   detail: 'Estimated 112K tokens, $0.34 cost.', age: '8 min ago' },
    { id: uid(), ts: '12:45:02', agent: 'CoderAgent',    action: 'tool.shell',  policy: 'Tool Execution Policy', detail: 'Shell: ls -la /workspace/src', age: '23 min ago' },
];

function ActionBadge({ action }: { action: GovernanceAction }) {
    const m = ACTION_META[action];
    return <span style={{ background: m.bg, color: m.color, padding: '1px 6px', borderRadius: 3, fontSize: 10, fontWeight: 700, fontFamily: 'monospace', border: `1px solid ${m.color}22` }}>{m.label}</span>;
}

function ScopeBadge({ scope }: { scope: GovernancePolicy['scope'] }) {
    const c: Record<string, string> = { global: '#c080ff', workspace: '#7ab4ff', agent: '#60d060', tool: '#d0a030' };
    return <span style={{ fontSize: 10, color: c[scope] ?? '#888', fontFamily: 'monospace' }}>{scope}</span>;
}

function PolicyCard({ policy, selected, onSelect }: { policy: GovernancePolicy; selected: boolean; onSelect: () => void }) {
    return (
        <div onClick={onSelect} style={{ border: `1px solid ${selected ? '#7ab4ff' : '#1e1e1e'}`, borderRadius: 6, padding: 12, marginBottom: 8, cursor: 'pointer', background: selected ? '#0f1a2a' : '#0d0d0d' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                <span style={{ fontSize: 12, fontWeight: 700, color: policy.enabled ? '#e0e0e0' : '#555', flex: 1 }}>{policy.name}</span>
                <ScopeBadge scope={policy.scope} />
                <span style={{ fontSize: 10, color: policy.enabled ? '#40a040' : '#604040', fontFamily: 'monospace' }}>{policy.enabled ? '● ACTIVE' : '○ DISABLED'}</span>
            </div>
            <div style={{ fontSize: 11, color: '#666', marginBottom: 8 }}>{policy.description}</div>
            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                {policy.rules.map(r => <ActionBadge key={r.id} action={r.action} />)}
            </div>
        </div>
    );
}

function RuleList({ rules }: { rules: GovernanceRule[] }) {
    return (
        <div>
            {rules.map(r => (
                <div key={r.id} style={{ border: '1px solid #1a1a1a', borderRadius: 4, padding: 10, marginBottom: 6, background: '#111' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                        <span style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>P{r.priority}</span>
                        <ActionBadge action={r.action} />
                        <span style={{ fontSize: 11, color: '#888', flex: 1, fontStyle: 'italic' }}>{r.rationale}</span>
                    </div>
                    <div style={{ fontSize: 11, fontFamily: 'monospace', color: '#a0d0a0', background: '#0d0d0d', padding: '4px 8px', borderRadius: 3 }}>{r.condition}</div>
                </div>
            ))}
        </div>
    );
}

function PoliciesTab() {
    const [selected, setSelected] = React.useState<GovernancePolicy | null>(null);

    return (
        <div style={{ display: 'flex', height: '100%', overflow: 'hidden' }}>
            <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
                {DEMO_POLICIES.map(p => (
                    <PolicyCard key={p.id} policy={p} selected={selected?.id === p.id} onSelect={() => setSelected(s => s?.id === p.id ? null : p)} />
                ))}
            </div>
            {selected && (
                <div style={{ width: 320, borderLeft: '1px solid #1e1e1e', overflow: 'auto', padding: 12, background: '#0d0d0d' }}>
                    <div style={{ fontSize: 12, fontWeight: 700, color: '#ccc', marginBottom: 4 }}>{selected.name}</div>
                    <ScopeBadge scope={selected.scope} />
                    <div style={{ fontSize: 11, color: '#666', margin: '8px 0', lineHeight: 1.5 }}>{selected.description}</div>
                    <div style={{ fontSize: 10, color: '#555', marginBottom: 8 }}>RULES ({selected.rules.length})</div>
                    <RuleList rules={selected.rules} />
                    <div style={{ marginTop: 12, fontSize: 10, color: '#555', lineHeight: 1.6 }}>
                        <div>Created: {selected.createdAt.slice(0, 10)}</div>
                        <div>Updated: {selected.updatedAt.slice(0, 10)}</div>
                    </div>
                </div>
            )}
        </div>
    );
}

function AuditTab() {
    return (
        <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
            <div style={{ border: '1px solid #1e1e1e', borderRadius: 6, overflow: 'hidden' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '52px 110px 90px 130px 80px 1fr', padding: '6px 10px', borderBottom: '1px solid #1e1e1e', background: '#111', fontSize: 10, color: '#555', fontWeight: 700 }}>
                    <span>TIME</span><span>AGENT</span><span>ACTION</span><span>POLICY</span><span>DECISION</span><span>DETAIL</span>
                </div>
                {AUDIT_LOG.map(entry => (
                    <div key={entry.id} style={{ display: 'grid', gridTemplateColumns: '52px 110px 90px 130px 80px 1fr', padding: '7px 10px', borderBottom: '1px solid #0f0f0f', background: '#0d0d0d', fontSize: 11 }}>
                        <span style={{ color: '#555', fontFamily: 'monospace' }}>{entry.ts}</span>
                        <span style={{ color: '#7ab4ff' }}>{entry.agent}</span>
                        <span style={{ color: '#a0a0a0', fontFamily: 'monospace', fontSize: 10 }}>{entry.action}</span>
                        <span style={{ color: '#888', fontSize: 10 }}>{entry.policy}</span>
                        <ActionBadge action={entry.decision} />
                        <span style={{ color: '#666', fontSize: 10 }}>{entry.detail}</span>
                    </div>
                ))}
            </div>
        </div>
    );
}

function ApprovalsTab() {
    const [pending, setPending] = React.useState(PENDING_APPROVALS);

    function approve(id: string)  { setPending(p => p.filter(x => x.id !== id)); }
    function reject(id: string)   { setPending(p => p.filter(x => x.id !== id)); }

    return (
        <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
            {pending.length === 0 && (
                <div style={{ textAlign: 'center', color: '#555', fontSize: 12, marginTop: 40 }}>No pending approvals.</div>
            )}
            {pending.map(item => (
                <div key={item.id} style={{ border: '1px solid #3a2a0a', borderRadius: 6, padding: 14, marginBottom: 10, background: '#1a1400' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                        <span style={{ fontSize: 12, fontWeight: 700, color: '#d0a030' }}>{item.agent}</span>
                        <span style={{ fontSize: 11, fontFamily: 'monospace', color: '#a0a0a0' }}>{item.action}</span>
                        <span style={{ marginLeft: 'auto', fontSize: 10, color: '#666' }}>{item.age}</span>
                    </div>
                    <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>Policy: {item.policy}</div>
                    <div style={{ fontSize: 11, color: '#ccc', marginBottom: 12, lineHeight: 1.5 }}>{item.detail}</div>
                    <div style={{ display: 'flex', gap: 8 }}>
                        <button onClick={() => approve(item.id)}
                            style={{ padding: '5px 16px', background: '#0a1a0a', border: '1px solid #3a7a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
                            ✓ Approve
                        </button>
                        <button onClick={() => reject(item.id)}
                            style={{ padding: '5px 16px', background: '#1a0a0a', border: '1px solid #7a3a3a', color: '#d06060', borderRadius: 4, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
                            ✗ Reject
                        </button>
                    </div>
                </div>
            ))}
        </div>
    );
}

function GovernanceView() {
    const [tab, setTab] = React.useState<GovTab>('policies');
    const pendingCount = PENDING_APPROVALS.length;

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {(['policies', 'audit', 'approvals'] as GovTab[]).map(t => (
                    <button key={t} onClick={() => setTab(t)}
                        style={{ padding: '8px 14px', background: 'none', border: 'none', borderBottom: tab === t ? '2px solid #d0a030' : '2px solid transparent', color: tab === t ? '#d0a030' : '#666', cursor: 'pointer', fontSize: 12, fontWeight: tab === t ? 700 : 400, textTransform: 'capitalize', display: 'flex', alignItems: 'center', gap: 5 }}>
                        {t}
                        {t === 'approvals' && pendingCount > 0 && (
                            <span style={{ background: '#d0a030', color: '#000', borderRadius: 8, fontSize: 10, fontWeight: 700, padding: '0 5px', lineHeight: 1.6 }}>{pendingCount}</span>
                        )}
                    </button>
                ))}
            </div>
            <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                {tab === 'policies'  && <PoliciesTab />}
                {tab === 'audit'     && <AuditTab />}
                {tab === 'approvals' && <ApprovalsTab />}
            </div>
        </div>
    );
}

@injectable()
export class GovernancePanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:governance';
    static readonly LABEL = 'Governance';

    @postConstruct()
    protected init(): void {
        this.id = GovernancePanelWidget.ID;
        this.title.label = GovernancePanelWidget.LABEL;
        this.title.caption = 'Policies, rules, and audit logs';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-shield';
        this.update();
    }

    protected render(): React.ReactNode {
        return <GovernanceView />;
    }
}

@injectable()
export class GovernancePanelContribution extends AbstractViewContribution<GovernancePanelWidget> {
    constructor() {
        super({ widgetId: GovernancePanelWidget.ID, widgetName: GovernancePanelWidget.LABEL, defaultWidgetOptions: { area: 'right' }, toggleCommandId: GovernancePanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(GovernancePanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
