import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { Task, TaskStatus } from '@agennext/agent-ide-types';
import { TasksPanelCommand } from '../agent-ide-commands';

type FilterStatus = 'all' | TaskStatus;

function uid(): string { return Math.random().toString(36).slice(2, 9); }

const AGENTS = ['ResearchAgent', 'CoderAgent', 'AnalystAgent', 'WriterAgent', 'ReviewerAgent'];

const DEMO_TASKS: Task[] = [
    { id: uid(), title: 'Market analysis Q2 2025', description: 'Gather and synthesize competitive intelligence for Q2 planning.', status: 'completed', assignedAgentId: 'research-agent', subtaskIds: [], artifactIds: ['a1'], createdAt: '2026-05-28T14:00:00Z', updatedAt: '2026-05-28T14:22:00Z', completedAt: '2026-05-28T14:22:00Z', metadata: {} },
    { id: uid(), title: 'Refactor orchestrator module', description: 'Modularize orchestrator.py for async task handling and improved testability.', status: 'completed', assignedAgentId: 'coder-agent', subtaskIds: [], artifactIds: ['a7'], createdAt: '2026-05-28T13:00:00Z', updatedAt: '2026-05-28T14:08:00Z', completedAt: '2026-05-28T14:08:00Z', metadata: {} },
    { id: uid(), title: 'Generate evaluation report', description: 'Compile performance metrics across 3 agent models for May 2026.', status: 'in_progress', assignedAgentId: 'analyst-agent', subtaskIds: [], artifactIds: [], createdAt: '2026-05-28T13:30:00Z', updatedAt: '2026-05-28T13:55:00Z', metadata: {} },
    { id: uid(), title: 'Write architecture decisions doc', description: 'Document key ADRs for Phase 1 and Phase 2 milestones.', status: 'completed', assignedAgentId: 'writer-agent', subtaskIds: [], artifactIds: ['a10'], createdAt: '2026-05-27T09:00:00Z', updatedAt: '2026-05-28T13:30:00Z', completedAt: '2026-05-28T13:30:00Z', metadata: {} },
    { id: uid(), title: 'Review API spec openapi.yaml', description: 'Audit endpoints, verify request/response schemas, flag missing error codes.', status: 'pending', assignedAgentId: 'reviewer-agent', subtaskIds: [], artifactIds: ['a4'], createdAt: '2026-05-28T12:00:00Z', updatedAt: '2026-05-28T12:00:00Z', metadata: {} },
    { id: uid(), title: 'Competitive landscape research', description: 'Analyze agent IDE competitors: Cursor, Windsurf, Continue, Cody.', status: 'failed', assignedAgentId: 'research-agent', subtaskIds: [], artifactIds: [], createdAt: '2026-05-28T12:45:00Z', updatedAt: '2026-05-28T13:01:00Z', metadata: {} },
    { id: uid(), title: 'Deploy browser app to staging', description: 'Build Docker image and push to staging k8s namespace.', status: 'pending', subtaskIds: [], artifactIds: [], createdAt: '2026-05-28T11:00:00Z', updatedAt: '2026-05-28T11:00:00Z', metadata: {} },
    { id: uid(), title: 'Implement knowledge vector search', description: 'Wire pgvector store to knowledge panel with semantic search endpoint.', status: 'pending', subtaskIds: [], artifactIds: [], createdAt: '2026-05-27T16:00:00Z', updatedAt: '2026-05-27T16:00:00Z', metadata: {} },
];

const STATUS_META: Record<TaskStatus, { label: string; color: string; bg: string }> = {
    completed:   { label: 'DONE',        color: '#40a040', bg: '#0a1a0a' },
    in_progress: { label: 'IN PROGRESS', color: '#4090d0', bg: '#0a1422' },
    pending:     { label: 'PENDING',     color: '#909030', bg: '#18180a' },
    failed:      { label: 'FAILED',      color: '#c04040', bg: '#1a0a0a' },
    cancelled:   { label: 'CANCELLED',   color: '#606060', bg: '#141414' },
};

const FILTER_TABS: { id: FilterStatus; label: string }[] = [
    { id: 'all',         label: 'All' },
    { id: 'in_progress', label: 'Active' },
    { id: 'pending',     label: 'Pending' },
    { id: 'completed',   label: 'Done' },
    { id: 'failed',      label: 'Failed' },
];

function StatusBadge({ status }: { status: TaskStatus }) {
    const m = STATUS_META[status];
    return (
        <span style={{ background: m.bg, color: m.color, padding: '1px 6px', borderRadius: 3, fontSize: 10, fontWeight: 700, fontFamily: 'monospace', border: `1px solid ${m.color}22` }}>
            {m.label}
        </span>
    );
}

function AgentTag({ agentId }: { agentId?: string }) {
    if (!agentId) return <span style={{ fontSize: 11, color: '#444' }}>unassigned</span>;
    const name = AGENTS.find(a => a.toLowerCase().replace(/agent/, '-agent').startsWith(agentId.split('-')[0])) ?? agentId;
    return <span style={{ fontSize: 11, color: '#7ab4ff' }}>{name}</span>;
}

function TaskDetail({ task, onClose }: { task: Task; onClose: () => void }) {
    const m = STATUS_META[task.status];
    return (
        <div style={{ width: 320, borderLeft: '1px solid #1e1e1e', display: 'flex', flexDirection: 'column', background: '#0d0d0d' }}>
            <div style={{ padding: '10px 12px', borderBottom: '1px solid #1e1e1e', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontSize: 12, fontWeight: 700, flex: 1, color: '#ccc' }}>Task Detail</span>
                <button onClick={onClose} style={{ background: 'none', border: 'none', color: '#666', cursor: 'pointer', fontSize: 14 }}>×</button>
            </div>
            <div style={{ padding: 14, overflow: 'auto', flex: 1 }}>
                <div style={{ fontSize: 13, fontWeight: 700, color: '#e0e0e0', marginBottom: 8 }}>{task.title}</div>
                <div style={{ fontSize: 11, color: '#888', lineHeight: 1.6, marginBottom: 12 }}>{task.description}</div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, fontSize: 11 }}>
                    <Row label="Status">     <StatusBadge status={task.status} /></Row>
                    <Row label="Agent">      <AgentTag agentId={task.assignedAgentId} /></Row>
                    <Row label="Created">    <span style={{ color: '#888' }}>{task.createdAt.slice(0, 16).replace('T', ' ')}</span></Row>
                    <Row label="Updated">    <span style={{ color: '#888' }}>{task.updatedAt.slice(0, 16).replace('T', ' ')}</span></Row>
                    {task.completedAt && <Row label="Completed"><span style={{ color: '#40a040' }}>{task.completedAt.slice(0, 16).replace('T', ' ')}</span></Row>}
                    <Row label="Artifacts">  <span style={{ color: task.artifactIds.length ? '#c080ff' : '#444' }}>{task.artifactIds.length} linked</span></Row>
                </div>

                {task.status === 'pending' && (
                    <button style={{ marginTop: 16, width: '100%', padding: '7px 0', background: '#1a3a1a', border: '1px solid #3a7a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
                        ▶ Assign &amp; Run
                    </button>
                )}
            </div>
        </div>
    );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
    return (
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span style={{ color: '#555' }}>{label}</span>
            {children}
        </div>
    );
}

function TasksView() {
    const [tasks, setTasks] = React.useState<Task[]>(DEMO_TASKS);
    const [filter, setFilter] = React.useState<FilterStatus>('all');
    const [selected, setSelected] = React.useState<Task | null>(null);
    const [adding, setAdding] = React.useState(false);
    const [newTitle, setNewTitle] = React.useState('');

    const filtered = filter === 'all' ? tasks : tasks.filter(t => t.status === filter);

    function addTask() {
        if (!newTitle.trim()) return;
        const t: Task = {
            id: uid(), title: newTitle.trim(),
            description: '', status: 'pending',
            subtaskIds: [], artifactIds: [],
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            metadata: {},
        };
        setTasks(prev => [t, ...prev]);
        setNewTitle('');
        setAdding(false);
    }

    return (
        <div style={{ display: 'flex', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                {/* Toolbar */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 12px', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                    <span style={{ fontSize: 12, fontWeight: 700, color: '#aaa', flex: 1 }}>TASKS</span>
                    <button onClick={() => setAdding(a => !a)}
                        style={{ padding: '4px 10px', background: '#1a3a1a', border: '1px solid #3a7a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 11 }}>
                        + New Task
                    </button>
                </div>

                {/* Add task inline */}
                {adding && (
                    <div style={{ display: 'flex', gap: 6, padding: '8px 12px', borderBottom: '1px solid #1e1e1e', background: '#111' }}>
                        <input
                            autoFocus
                            value={newTitle}
                            onChange={e => setNewTitle(e.target.value)}
                            onKeyDown={e => { if (e.key === 'Enter') addTask(); if (e.key === 'Escape') setAdding(false); }}
                            placeholder="Task title…"
                            style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', borderRadius: 4, padding: '5px 8px', fontSize: 12, outline: 'none' }}
                        />
                        <button onClick={addTask} style={{ padding: '4px 10px', background: '#1a3a1a', border: '1px solid #3a7a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 11 }}>Add</button>
                        <button onClick={() => setAdding(false)} style={{ padding: '4px 8px', background: 'none', border: '1px solid #333', color: '#888', borderRadius: 4, cursor: 'pointer', fontSize: 11 }}>Cancel</button>
                    </div>
                )}

                {/* Filter tabs */}
                <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                    {FILTER_TABS.map(t => (
                        <button key={t.id} onClick={() => setFilter(t.id)}
                            style={{ padding: '6px 12px', background: 'none', border: 'none', borderBottom: filter === t.id ? '2px solid #7ab4ff' : '2px solid transparent', color: filter === t.id ? '#7ab4ff' : '#666', cursor: 'pointer', fontSize: 11, fontWeight: filter === t.id ? 700 : 400 }}>
                            {t.label}
                            <span style={{ marginLeft: 4, fontSize: 10, color: '#444' }}>
                                {t.id === 'all' ? tasks.length : tasks.filter(x => x.status === t.id).length}
                            </span>
                        </button>
                    ))}
                </div>

                {/* Task list */}
                <div style={{ flex: 1, overflow: 'auto' }}>
                    {filtered.map(task => (
                        <div
                            key={task.id}
                            onClick={() => setSelected(s => s?.id === task.id ? null : task)}
                            style={{
                                display: 'flex', alignItems: 'center', gap: 10, padding: '9px 12px',
                                borderBottom: '1px solid #1a1a1a', cursor: 'pointer',
                                background: selected?.id === task.id ? '#111a2a' : 'transparent',
                            }}
                        >
                            <StatusBadge status={task.status} />
                            <div style={{ flex: 1, minWidth: 0 }}>
                                <div style={{ fontSize: 12, color: '#d0d0d0', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{task.title}</div>
                                <AgentTag agentId={task.assignedAgentId} />
                            </div>
                            <span style={{ fontSize: 10, color: '#444', whiteSpace: 'nowrap' }}>{task.updatedAt.slice(5, 16).replace('T', ' ')}</span>
                        </div>
                    ))}
                    {filtered.length === 0 && (
                        <div style={{ padding: 24, textAlign: 'center', color: '#444', fontSize: 12 }}>No tasks matching this filter.</div>
                    )}
                </div>
            </div>

            {selected && <TaskDetail task={selected} onClose={() => setSelected(null)} />}
        </div>
    );
}

@injectable()
export class TasksPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:tasks';
    static readonly LABEL = 'Tasks';

    @postConstruct()
    protected init(): void {
        this.id = TasksPanelWidget.ID;
        this.title.label = TasksPanelWidget.LABEL;
        this.title.caption = 'Task queue and assignments';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-checklist';
        this.update();
    }

    protected render(): React.ReactNode {
        return <TasksView />;
    }
}

@injectable()
export class TasksPanelContribution extends AbstractViewContribution<TasksPanelWidget> {
    constructor() {
        super({ widgetId: TasksPanelWidget.ID, widgetName: TasksPanelWidget.LABEL, defaultWidgetOptions: { area: 'left' }, toggleCommandId: TasksPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(TasksPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
