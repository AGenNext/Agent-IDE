import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { WorkspacesPanelCommand } from '../agent-ide-commands';
import {
    isBackendReachable, login, getMe, listWorkspaces, createWorkspace,
    renameWorkspace, deleteWorkspace, activateWorkspace,
    AuthUser, WorkspaceRecord,
} from '../runtime/backend-client';
import { getSession, setSession, clearSession, updateSession } from '../runtime/session-store';

type WsTab = 'account' | 'workspaces';

// ─── Status badge ─────────────────────────────────────────────────────────────

const STATUS_META: Record<string, { color: string; label: string }> = {
    active:       { color: '#40a040', label: 'active' },
    inactive:     { color: '#555555', label: 'inactive' },
    provisioning: { color: '#d0a030', label: 'provisioning' },
    error:        { color: '#c04040', label: 'error' },
};

function StatusBadge({ status }: { status: string }) {
    const m = STATUS_META[status] ?? { color: '#888', label: status };
    return <span style={{ fontSize: 10, color: m.color, fontFamily: 'monospace', border: `1px solid ${m.color}44`, padding: '1px 5px', borderRadius: 3 }}>{m.label}</span>;
}

// ─── Avatar initials ──────────────────────────────────────────────────────────

function Avatar({ name }: { name: string }) {
    const initials = name.split(' ').map(p => p[0]).join('').toUpperCase().slice(0, 2);
    return (
        <div style={{ width: 40, height: 40, borderRadius: '50%', background: 'linear-gradient(135deg, #7ab4ff, #c080ff)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 15, fontWeight: 700, color: '#fff', flexShrink: 0 }}>
            {initials}
        </div>
    );
}

// ─── Account tab ─────────────────────────────────────────────────────────────

function AccountTab({ user, liveBackend, onLogin, onLogout }: {
    user: AuthUser | null;
    liveBackend: boolean;
    onLogin: (user: AuthUser, token: string) => void;
    onLogout: () => void;
}) {
    const [email, setEmail]       = React.useState('');
    const [password, setPassword] = React.useState('');
    const [loading, setLoading]   = React.useState(false);
    const [err, setErr]           = React.useState('');

    async function handleLogin(e: React.FormEvent) {
        e.preventDefault();
        setLoading(true); setErr('');
        try {
            const result = await login(email, password);
            onLogin(result.user, result.token);
        } catch (ex) {
            setErr(String(ex).replace(/^Error: Backend .*→ \d+: /, ''));
        } finally {
            setLoading(false);
        }
    }

    const inputStyle: React.CSSProperties = { width: '100%', boxSizing: 'border-box', background: '#111', border: '1px solid #333', color: '#ddd', borderRadius: 3, padding: '6px 10px', fontSize: 12, outline: 'none' };

    if (user) {
        return (
            <div style={{ padding: 16 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20, padding: 14, background: '#0d0d0d', borderRadius: 8, border: '1px solid #1e1e1e' }}>
                    <Avatar name={user.name} />
                    <div>
                        <div style={{ fontSize: 13, fontWeight: 700, color: '#e0e0e0' }}>{user.name}</div>
                        <div style={{ fontSize: 11, color: '#888', marginTop: 2 }}>{user.email}</div>
                        <div style={{ fontSize: 10, color: '#555', marginTop: 2, fontFamily: 'monospace' }}>{user.userId}</div>
                    </div>
                </div>

                {!liveBackend && (
                    <div style={{ padding: '8px 12px', background: '#111', border: '1px solid #2a2a2a', borderRadius: 4, fontSize: 11, color: '#888', marginBottom: 16 }}>
                        ○ Running in offline demo mode. Start the backend to enable real auth.
                    </div>
                )}

                <div style={{ fontSize: 10, color: '#555', fontWeight: 700, marginBottom: 8, textTransform: 'uppercase', letterSpacing: 1 }}>Auth Provider</div>
                <div style={{ padding: '8px 12px', background: '#0d0d0d', border: '1px solid #1a1a1a', borderRadius: 4, fontSize: 11, color: '#888', marginBottom: 20 }}>
                    <div style={{ marginBottom: 4 }}>Provider: <span style={{ color: '#7ab4ff' }}>Built-in (demo)</span></div>
                    <div>Clerk / Auth0: <span style={{ color: '#555' }}>not configured — set CLERK_PUBLISHABLE_KEY or AUTH0_DOMAIN</span></div>
                </div>

                <button onClick={onLogout}
                    style={{ padding: '7px 18px', background: '#1a0a0a', border: '1px solid #5a2a2a', color: '#d06060', borderRadius: 4, cursor: 'pointer', fontSize: 12, fontWeight: 600 }}>
                    Sign out
                </button>
            </div>
        );
    }

    return (
        <div style={{ padding: 16 }}>
            <div style={{ fontSize: 13, fontWeight: 700, color: '#e0e0e0', marginBottom: 4 }}>Sign in</div>
            <div style={{ fontSize: 11, color: '#666', marginBottom: 20 }}>
                {liveBackend ? 'Use any email with password "demo" in dev mode.' : 'Backend offline — running as demo user automatically.'}
            </div>
            <form onSubmit={handleLogin}>
                <div style={{ marginBottom: 10 }}>
                    <div style={{ fontSize: 10, color: '#777', marginBottom: 4 }}>Email</div>
                    <input type="email" value={email} onChange={e => setEmail(e.target.value)} placeholder="you@example.com" style={inputStyle} />
                </div>
                <div style={{ marginBottom: 14 }}>
                    <div style={{ fontSize: 10, color: '#777', marginBottom: 4 }}>Password</div>
                    <input type="password" value={password} onChange={e => setPassword(e.target.value)} placeholder="••••••••" style={inputStyle} />
                </div>
                {err && <div style={{ fontSize: 11, color: '#c04040', marginBottom: 10 }}>{err}</div>}
                <button type="submit" disabled={loading || !liveBackend}
                    style={{ width: '100%', padding: '8px', background: loading ? '#111' : '#0a1a2a', border: '1px solid #2a5a8a', color: loading ? '#555' : '#7ab4ff', borderRadius: 4, cursor: loading ? 'default' : 'pointer', fontSize: 12, fontWeight: 700 }}>
                    {loading ? 'Signing in…' : !liveBackend ? 'Backend offline' : 'Sign in'}
                </button>
            </form>
        </div>
    );
}

// ─── Workspaces tab ───────────────────────────────────────────────────────────

function WorkspacesTab({ user, token, liveBackend }: { user: AuthUser | null; token: string | null; liveBackend: boolean }) {
    const [workspaces, setWorkspaces] = React.useState<WorkspaceRecord[]>([]);
    const [loading, setLoading]       = React.useState(true);
    const [newName, setNewName]       = React.useState('');
    const [creating, setCreating]     = React.useState(false);
    const [editId, setEditId]         = React.useState<string | null>(null);
    const [editName, setEditName]     = React.useState('');

    React.useEffect(() => {
        if (!liveBackend) { setLoading(false); return; }
        listWorkspaces(token ?? undefined).then(ws => setWorkspaces(ws)).catch(() => {}).finally(() => setLoading(false));
    }, [liveBackend, token]);

    async function handleCreate(e: React.FormEvent) {
        e.preventDefault();
        if (!newName.trim()) return;
        setCreating(true);
        try {
            const w = await createWorkspace(newName.trim(), token ?? undefined);
            setWorkspaces(prev => [...prev, w]);
            setNewName('');
        } catch { /* ignore */ }
        setCreating(false);
    }

    async function handleActivate(id: string) {
        const w = await activateWorkspace(id, token ?? undefined);
        setWorkspaces(prev => prev.map(ws => ws.id === id ? w : { ...ws, status: ws.status === 'active' ? 'inactive' : ws.status }));
        updateSession({ activeWorkspaceId: id });
    }

    async function handleDelete(id: string) {
        await deleteWorkspace(id, token ?? undefined);
        setWorkspaces(prev => prev.filter(ws => ws.id !== id));
    }

    async function handleRename(id: string) {
        if (!editName.trim()) return;
        const w = await renameWorkspace(id, editName.trim(), token ?? undefined);
        setWorkspaces(prev => prev.map(ws => ws.id === id ? w : ws));
        setEditId(null);
    }

    const session = getSession();
    const activeId = session?.activeWorkspaceId ?? workspaces.find(w => w.status === 'active')?.id;

    if (!liveBackend) {
        return (
            <div style={{ padding: 24, textAlign: 'center', color: '#555', fontSize: 12 }}>
                Backend offline. Workspace management requires the agent backend.
            </div>
        );
    }

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #1a1a1a', fontSize: 11, color: '#555', background: '#0d0d0d' }}>
                {loading ? 'Loading…' : `${workspaces.length} workspace${workspaces.length !== 1 ? 's' : ''}`}
                {user && <span style={{ color: '#444', marginLeft: 8 }}>· {user.userId}</span>}
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: 10 }}>
                {workspaces.map(w => (
                    <div key={w.id} style={{ border: `1px solid ${w.id === activeId ? '#2a4a2a' : '#1e1e1e'}`, borderRadius: 6, padding: 12, marginBottom: 8, background: w.id === activeId ? '#0a0f0a' : '#0d0d0d' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                            {editId === w.id ? (
                                <input value={editName} onChange={e => setEditName(e.target.value)}
                                    onKeyDown={e => { if (e.key === 'Enter') handleRename(w.id); if (e.key === 'Escape') setEditId(null); }}
                                    autoFocus
                                    style={{ flex: 1, background: '#111', border: '1px solid #7ab4ff', color: '#ddd', borderRadius: 3, padding: '3px 6px', fontSize: 12, outline: 'none' }} />
                            ) : (
                                <span style={{ fontSize: 12, fontWeight: 700, color: '#d0d0d0', flex: 1 }}>{w.name}</span>
                            )}
                            <StatusBadge status={w.status} />
                        </div>
                        <div style={{ fontSize: 10, color: '#444', fontFamily: 'monospace', marginBottom: 8 }}>{w.id}</div>
                        <div style={{ fontSize: 10, color: '#444', marginBottom: 8 }}>
                            Created {new Date(w.createdAt).toLocaleDateString()}
                        </div>
                        <div style={{ display: 'flex', gap: 6 }}>
                            {w.id !== activeId && (
                                <button onClick={() => handleActivate(w.id)}
                                    style={{ padding: '3px 10px', fontSize: 11, background: '#0a1a0a', border: '1px solid #3a7a3a', color: '#60d060', borderRadius: 3, cursor: 'pointer', fontWeight: 600 }}>
                                    Activate
                                </button>
                            )}
                            {w.id === activeId && (
                                <span style={{ padding: '3px 10px', fontSize: 11, color: '#40a040', fontWeight: 700 }}>● Active</span>
                            )}
                            {editId !== w.id ? (
                                <button onClick={() => { setEditId(w.id); setEditName(w.name); }}
                                    style={{ padding: '3px 8px', fontSize: 11, background: 'none', border: '1px solid #2a2a2a', color: '#888', borderRadius: 3, cursor: 'pointer' }}>
                                    Rename
                                </button>
                            ) : (
                                <button onClick={() => handleRename(w.id)}
                                    style={{ padding: '3px 8px', fontSize: 11, background: 'none', border: '1px solid #4a7a4a', color: '#60d060', borderRadius: 3, cursor: 'pointer' }}>
                                    Save
                                </button>
                            )}
                            <button onClick={() => handleDelete(w.id)}
                                style={{ padding: '3px 8px', fontSize: 11, background: 'none', border: '1px solid #2a2a2a', color: '#666', borderRadius: 3, cursor: 'pointer', marginLeft: 'auto' }}>
                                Delete
                            </button>
                        </div>
                    </div>
                ))}

                <form onSubmit={handleCreate} style={{ marginTop: 8, paddingTop: 12, borderTop: '1px solid #1a1a1a', display: 'flex', gap: 8 }}>
                    <input value={newName} onChange={e => setNewName(e.target.value)} placeholder="New workspace name…"
                        style={{ flex: 1, background: '#111', border: '1px solid #2a2a2a', color: '#ddd', borderRadius: 3, padding: '6px 10px', fontSize: 11, outline: 'none' }} />
                    <button type="submit" disabled={creating || !newName.trim()}
                        style={{ padding: '6px 12px', background: '#0a1a0a', border: '1px solid #3a7a3a', color: creating ? '#555' : '#60d060', borderRadius: 3, cursor: 'pointer', fontSize: 11, fontWeight: 700 }}>
                        {creating ? '…' : '+ New'}
                    </button>
                </form>
            </div>
        </div>
    );
}

// ─── Root view ────────────────────────────────────────────────────────────────

function WorkspacesView() {
    const [tab, setTab]             = React.useState<WsTab>('account');
    const [liveBackend, setLive]    = React.useState(false);
    const [user, setUser]           = React.useState<AuthUser | null>(null);
    const [token, setToken]         = React.useState<string | null>(null);

    React.useEffect(() => {
        (async () => {
            const reachable = await isBackendReachable();
            setLive(reachable);

            // Restore session from localStorage
            const session = getSession();
            if (session) {
                setToken(session.token);
                setUser({ userId: session.userId, email: session.email, name: session.name });
            } else if (reachable) {
                // Fetch demo user (no auth required in demo mode)
                try {
                    const me = await getMe();
                    setUser(me);
                    // Don't persist a token-less session
                } catch { /* ignore */ }
            } else {
                // Offline demo user
                setUser({ userId: 'user_demo', email: 'demo@agent-ide.local', name: 'Demo User' });
            }
        })();
    }, []);

    function handleLogin(loggedInUser: AuthUser, issuedToken: string) {
        setUser(loggedInUser);
        setToken(issuedToken);
        setSession({ token: issuedToken, userId: loggedInUser.userId, email: loggedInUser.email, name: loggedInUser.name });
        setTab('workspaces');
    }

    function handleLogout() {
        clearSession();
        setUser(null);
        setToken(null);
    }

    const tabBtn = (t: WsTab, label: string) => (
        <button onClick={() => setTab(t)}
            style={{ padding: '8px 16px', background: 'none', border: 'none', borderBottom: tab === t ? '2px solid #7ab4ff' : '2px solid transparent', color: tab === t ? '#7ab4ff' : '#666', cursor: 'pointer', fontSize: 12, fontWeight: tab === t ? 700 : 400 }}>
            {label}
        </button>
    );

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                {tabBtn('account', 'Account')}
                {tabBtn('workspaces', 'Workspaces')}
                <span style={{ marginLeft: 'auto', padding: '8px 12px', fontSize: 10, color: liveBackend ? '#40a040' : '#555' }}>
                    {liveBackend ? '● live' : '○ offline'}
                </span>
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {tab === 'account' && (
                    <AccountTab user={user} liveBackend={liveBackend} onLogin={handleLogin} onLogout={handleLogout} />
                )}
                {tab === 'workspaces' && (
                    <WorkspacesTab user={user} token={token} liveBackend={liveBackend} />
                )}
            </div>
        </div>
    );
}

// ─── Widget + Contribution ────────────────────────────────────────────────────

@injectable()
export class WorkspacesPanelWidget extends ReactWidget {
    static readonly ID    = 'agent-ide:workspaces';
    static readonly LABEL = 'Workspaces';

    @postConstruct()
    protected init(): void {
        this.id = WorkspacesPanelWidget.ID;
        this.title.label = WorkspacesPanelWidget.LABEL;
        this.title.caption = 'User account, authentication, and workspace management';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-account';
        this.update();
    }

    protected render(): React.ReactNode {
        return <WorkspacesView />;
    }
}

@injectable()
export class WorkspacesPanelContribution extends AbstractViewContribution<WorkspacesPanelWidget> {
    constructor() {
        super({ widgetId: WorkspacesPanelWidget.ID, widgetName: WorkspacesPanelWidget.LABEL, defaultWidgetOptions: { area: 'left' }, toggleCommandId: WorkspacesPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(WorkspacesPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
