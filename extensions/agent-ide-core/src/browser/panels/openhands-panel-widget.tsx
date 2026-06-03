import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { OpenHandsPanelCommand } from '../agent-ide-commands';
import { invokeTool, getBackendConfig } from '../runtime/backend-client';

interface TaskRecord {
    id: string;
    task: string;
    status: 'running' | 'done' | 'error';
    result?: string;
    error?: string;
    startedAt: string;
    durationMs?: number;
}

function ts() { return new Date().toISOString(); }

function StatusBadge({ status }: { status: TaskRecord['status'] }) {
    const c = status === 'done' ? '#60d060' : status === 'error' ? '#e06060' : '#60b0ff';
    const bg = status === 'done' ? '#0f2010' : status === 'error' ? '#2a1010' : '#101a2a';
    return (
        <span style={{ padding: '2px 7px', borderRadius: 3, fontSize: 10, fontWeight: 700, color: c, background: bg, border: `1px solid ${c}33` }}>
            {status === 'running' ? '⏳ running' : status === 'done' ? '✓ done' : '✗ error'}
        </span>
    );
}

function TaskCard({ t, onSelect, selected }: { t: TaskRecord; onSelect: () => void; selected: boolean }) {
    return (
        <div onClick={onSelect} style={{
            padding: '10px 12px', borderBottom: '1px solid #1a1a1a', cursor: 'pointer',
            background: selected ? '#131f2a' : 'transparent',
            borderLeft: selected ? '2px solid #60b0ff' : '2px solid transparent',
        }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                <StatusBadge status={t.status} />
                {t.durationMs && <span style={{ fontSize: 10, color: '#444' }}>{(t.durationMs / 1000).toFixed(1)}s</span>}
            </div>
            <div style={{ fontSize: 12, color: '#ccc', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{t.task}</div>
            <div style={{ fontSize: 10, color: '#444', marginTop: 2 }}>{t.startedAt.slice(11, 19)}</div>
        </div>
    );
}

function ResultView({ t }: { t: TaskRecord }) {
    if (t.status === 'running') {
        return (
            <div style={{ padding: 20, display: 'flex', flexDirection: 'column', gap: 12, alignItems: 'center', color: '#555' }}>
                <div style={{ fontSize: 24 }}>⏳</div>
                <div style={{ fontSize: 13 }}>OpenHandS is working…</div>
                <div style={{ fontSize: 11, color: '#444' }}>{t.task}</div>
            </div>
        );
    }
    if (t.status === 'error') {
        return (
            <div style={{ padding: 16 }}>
                <div style={{ fontSize: 11, color: '#888', marginBottom: 8 }}>Error</div>
                <pre style={{ margin: 0, fontSize: 12, color: '#e07070', whiteSpace: 'pre-wrap', wordBreak: 'break-word', background: '#1a0f0f', padding: 12, borderRadius: 4, border: '1px solid #3a1515' }}>{t.error}</pre>
            </div>
        );
    }
    return (
        <div style={{ padding: 16 }}>
            <div style={{ fontSize: 11, color: '#888', marginBottom: 8 }}>Result</div>
            <pre style={{ margin: 0, fontSize: 12, color: '#ccc', whiteSpace: 'pre-wrap', wordBreak: 'break-word', background: '#0f1a0f', padding: 12, borderRadius: 4, border: '1px solid #1a3a1a', lineHeight: 1.6 }}>{t.result ?? '(no output)'}</pre>
        </div>
    );
}

function OpenHandsView() {
    const [tasks, setTasks] = React.useState<TaskRecord[]>([]);
    const [selectedId, setSelectedId] = React.useState('');
    const [task, setTask] = React.useState('');
    const [repoUrl, setRepoUrl] = React.useState('');
    const [connected, setConnected] = React.useState<boolean | null>(null);
    const [submitting, setSubmitting] = React.useState(false);

    // Check if OpenHandS is configured
    React.useEffect(() => {
        getBackendConfig()
            .then(cfg => setConnected((cfg as unknown as { hasOpenHandsUrl?: boolean }).hasOpenHandsUrl ?? false))
            .catch(() => setConnected(false));
    }, []);

    const submit = async () => {
        if (!task.trim()) return;
        const id = `oh-${Date.now()}`;
        const rec: TaskRecord = { id, task: task.trim(), status: 'running', startedAt: ts() };
        setTasks(prev => [rec, ...prev]);
        setSelectedId(id);
        setTask('');
        setSubmitting(true);

        const t0 = Date.now();
        try {
            const result = await invokeTool('openhands_task', {
                task: rec.task,
                ...(repoUrl.trim() ? { repoUrl: repoUrl.trim() } : {}),
            }) as { output?: string; error?: string; connected?: boolean };

            setTasks(prev => prev.map(t => t.id === id ? {
                ...t,
                status: result.error ? 'error' : 'done',
                result: result.output,
                error: result.error,
                durationMs: Date.now() - t0,
            } : t));
        } catch (e) {
            setTasks(prev => prev.map(t => t.id === id ? {
                ...t, status: 'error', error: String(e), durationMs: Date.now() - t0,
            } : t));
        } finally {
            setSubmitting(false);
        }
    };

    const selected = tasks.find(t => t.id === selectedId);

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>

            {/* Header */}
            <div style={{ padding: '10px 14px', borderBottom: '1px solid #1e1e1e', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontSize: 14, fontWeight: 700, color: '#ddd' }}>OpenHandS</span>
                {connected === null && <span style={{ fontSize: 11, color: '#555' }}>checking…</span>}
                {connected === true && <span style={{ fontSize: 11, color: '#60d060' }}>● connected</span>}
                {connected === false && <span style={{ fontSize: 11, color: '#e06060' }}>● not configured</span>}
            </div>

            {/* Not connected notice */}
            {connected === false && (
                <div style={{ padding: '10px 14px', background: '#1a1010', borderBottom: '1px solid #2a1515', fontSize: 11, color: '#a06060' }}>
                    Set <code style={{ background: '#111', padding: '1px 4px', borderRadius: 2 }}>OPENHANDS_URL</code> to enable.
                    Start OpenHandS: <code style={{ background: '#111', padding: '1px 4px', borderRadius: 2 }}>docker run -p 3000:3000 ghcr.io/all-hands-ai/openhands:latest</code>
                </div>
            )}

            {/* Compose */}
            <div style={{ padding: '10px 14px', borderBottom: '1px solid #1e1e1e', display: 'flex', flexDirection: 'column', gap: 8 }}>
                <textarea
                    value={task} onChange={e => setTask(e.target.value)}
                    placeholder="Describe the software task… (e.g. Add a dark mode toggle to the settings page)"
                    rows={3}
                    style={{ width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '7px 9px', borderRadius: 4, fontSize: 12, resize: 'vertical', boxSizing: 'border-box', fontFamily: 'inherit' }}
                    onKeyDown={e => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) submit(); }}
                />
                <input
                    value={repoUrl} onChange={e => setRepoUrl(e.target.value)}
                    placeholder="Repo URL (optional) — https://github.com/org/repo"
                    style={{ width: '100%', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', padding: '5px 8px', borderRadius: 4, fontSize: 12, boxSizing: 'border-box' }}
                />
                <button
                    onClick={submit} disabled={!task.trim() || submitting}
                    style={{ alignSelf: 'flex-end', padding: '5px 16px', background: submitting || !task.trim() ? '#1a2a1a' : '#1e3a1e', border: '1px solid #3a6a3a', color: submitting || !task.trim() ? '#555' : '#60d060', borderRadius: 4, cursor: submitting || !task.trim() ? 'not-allowed' : 'pointer', fontSize: 12, fontWeight: 600 }}>
                    {submitting ? 'Running…' : '▶ Run Task'}
                </button>
            </div>

            {/* Body — task list + result */}
            <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
                {/* Task list */}
                <div style={{ width: 200, borderRight: '1px solid #1e1e1e', overflow: 'auto', flexShrink: 0 }}>
                    <div style={{ padding: '6px 12px', fontSize: 10, color: '#555', fontWeight: 600, borderBottom: '1px solid #1a1a1a' }}>HISTORY</div>
                    {tasks.length === 0 && <div style={{ padding: 16, fontSize: 12, color: '#444' }}>No tasks yet.</div>}
                    {tasks.map(t => <TaskCard key={t.id} t={t} selected={selectedId === t.id} onSelect={() => setSelectedId(t.id)} />)}
                </div>

                {/* Result */}
                <div style={{ flex: 1, overflow: 'auto' }}>
                    {selected
                        ? <ResultView t={selected} />
                        : <div style={{ padding: 24, color: '#444', fontSize: 13 }}>Select a task to see output.</div>
                    }
                </div>
            </div>
        </div>
    );
}

@injectable()
export class OpenHandsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:openhands';
    static readonly LABEL = 'OpenHandS';

    @postConstruct()
    protected init(): void {
        this.id = OpenHandsPanelWidget.ID;
        this.title.label = OpenHandsPanelWidget.LABEL;
        this.title.caption = OpenHandsPanelWidget.LABEL;
        this.title.closable = true;
        this.title.iconClass = 'fa fa-code';
        this.update();
    }

    protected render(): React.ReactNode {
        return <OpenHandsView />;
    }
}

@injectable()
export class OpenHandsPanelContribution extends AbstractViewContribution<OpenHandsPanelWidget> {
    constructor() {
        super({ widgetId: OpenHandsPanelWidget.ID, widgetName: OpenHandsPanelWidget.LABEL, defaultWidgetOptions: { area: 'main' }, toggleCommandId: OpenHandsPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(OpenHandsPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
