import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { IdentityPanelCommand } from '../agent-ide-commands';
import {
    getIdentityMe, updateIdentityMe, changePassword,
    listAgentIdentities, createAgentIdentity, updateAgentIdentity, deleteAgentIdentity,
    listApiKeys, createApiKey, revokeApiKey,
    listOrgs, createOrg, listOrgMembers, listOrgTeams, createOrgTeam,
    type IdentityUser, type AgentIdentity, type ApiKey, type OrgRecord, type OrgMember, type TeamRecord,
} from '../runtime/backend-client';

type IdentityTab = 'profile' | 'agents' | 'keys' | 'org';

const STATUS_COLOR: Record<string, string> = {
    active:    '#60d060',
    suspended: '#d0a030',
    retired:   '#888',
    pending:   '#7ab4ff',
};

const ROLE_COLOR: Record<string, string> = {
    admin:     '#ff8060',
    developer: '#60b0ff',
    viewer:    '#888',
    owner:     '#d060d0',
    member:    '#60d0a0',
};

function Badge({ label, color }: { label: string; color: string }) {
    return <span style={{ fontSize: 10, fontWeight: 700, color, border: `1px solid ${color}`, borderRadius: 3, padding: '1px 5px', textTransform: 'uppercase' }}>{label}</span>;
}

function Section({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) {
    return (
        <div style={{ marginBottom: 20 }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8, paddingBottom: 4, borderBottom: '1px solid #1a1a1a' }}>
                <span style={{ fontSize: 11, fontWeight: 700, color: '#888', textTransform: 'uppercase', letterSpacing: 1 }}>{title}</span>
                {action}
            </div>
            {children}
        </div>
    );
}

// ─── Profile Tab ──────────────────────────────────────────────────────────────

function ProfileTab() {
    const [user, setUser]           = React.useState<IdentityUser | null>(null);
    const [editing, setEditing]     = React.useState(false);
    const [name, setName]           = React.useState('');
    const [newPw, setNewPw]         = React.useState('');
    const [pwConfirm, setPwConfirm] = React.useState('');
    const [msg, setMsg]             = React.useState('');

    React.useEffect(() => {
        getIdentityMe().then(u => { setUser(u); setName(u.name); }).catch(() => setMsg('Backend not reachable'));
    }, []);

    async function save() {
        if (!user) return;
        try {
            const updated = await updateIdentityMe({ name });
            setUser(updated); setEditing(false); setMsg('Profile updated.');
        } catch { setMsg('Update failed.'); }
    }

    async function savePw() {
        if (newPw.length < 8) { setMsg('Password must be ≥ 8 characters.'); return; }
        if (newPw !== pwConfirm) { setMsg('Passwords do not match.'); return; }
        try { await changePassword(newPw); setNewPw(''); setPwConfirm(''); setMsg('Password changed.'); }
        catch { setMsg('Password change failed.'); }
    }

    if (!user) return <div style={{ padding: 16, color: '#555', fontSize: 12 }}>Loading profile…{msg && ` ${msg}`}</div>;

    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 16 }}>
            {msg && <div style={{ padding: '6px 10px', background: '#1a2a1a', borderRadius: 4, fontSize: 11, color: '#60d0a0' }}>{msg}</div>}

            <Section title="Account">
                <div style={{ display: 'flex', gap: 12, alignItems: 'flex-start' }}>
                    <div style={{ width: 48, height: 48, borderRadius: '50%', background: '#1a2a3a', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 20, color: '#7ab4ff', flexShrink: 0 }}>
                        {user.avatarUrl ? <img src={user.avatarUrl} style={{ width: 48, height: 48, borderRadius: '50%' }} alt="" /> : user.name[0]?.toUpperCase()}
                    </div>
                    <div style={{ flex: 1 }}>
                        {editing ? (
                            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                                <input value={name} onChange={e => setName(e.target.value)}
                                    style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '4px 8px', borderRadius: 4, fontSize: 13 }} />
                                <button onClick={save} style={btnPrimary}>Save</button>
                                <button onClick={() => { setEditing(false); setName(user.name); }} style={btnSecondary}>Cancel</button>
                            </div>
                        ) : (
                            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                                <span style={{ fontSize: 14, color: '#ddd', fontWeight: 700 }}>{user.name}</span>
                                <button onClick={() => setEditing(true)} style={btnSecondary}>Edit</button>
                            </div>
                        )}
                        <div style={{ fontSize: 12, color: '#666', marginTop: 3 }}>{user.email}</div>
                        <div style={{ display: 'flex', gap: 6, marginTop: 5 }}>
                            <Badge label={user.role} color={ROLE_COLOR[user.role] ?? '#888'} />
                            <Badge label={user.status} color={STATUS_COLOR[user.status] ?? '#888'} />
                            {user.mfaEnabled && <Badge label="MFA" color="#60d0a0" />}
                        </div>
                    </div>
                </div>
            </Section>

            <Section title="Change Password">
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                    <input type="password" placeholder="New password (min 8 chars)" value={newPw} onChange={e => setNewPw(e.target.value)}
                        style={{ background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 12 }} />
                    <input type="password" placeholder="Confirm password" value={pwConfirm} onChange={e => setPwConfirm(e.target.value)}
                        style={{ background: '#1a1a1a', border: '1px solid #333', color: '#ddd', padding: '5px 8px', borderRadius: 4, fontSize: 12 }} />
                    <button onClick={savePw} style={{ ...btnPrimary, alignSelf: 'flex-start' }}>Update Password</button>
                </div>
            </Section>

            <Section title="User ID">
                <div style={{ fontFamily: 'monospace', fontSize: 11, color: '#555', padding: '4px 0' }}>{user.userId}</div>
            </Section>
        </div>
    );
}

// ─── Agents Tab ───────────────────────────────────────────────────────────────

function AgentsTab() {
    const [agents, setAgents] = React.useState<AgentIdentity[]>([]);
    const [creating, setCreating] = React.useState(false);
    const [form, setForm] = React.useState({ name: '', description: '', model: 'claude-sonnet-4-6', capabilities: 'web_search,http_client' });
    const [msg, setMsg] = React.useState('');

    async function refresh() {
        try { setAgents(await listAgentIdentities()); } catch { setMsg('Backend not reachable'); }
    }
    React.useEffect(() => { refresh(); }, []);

    async function create() {
        try {
            await createAgentIdentity({ name: form.name, description: form.description, model: form.model, capabilities: form.capabilities.split(',').map(s => s.trim()).filter(Boolean) });
            setCreating(false); setForm({ name: '', description: '', model: 'claude-sonnet-4-6', capabilities: 'web_search,http_client' }); await refresh();
        } catch (err: unknown) { setMsg(err instanceof Error ? err.message : 'Create failed'); }
    }

    async function setStatus(id: string, status: 'active' | 'suspended' | 'retired') {
        try { await updateAgentIdentity(id, { status }); await refresh(); } catch { setMsg('Update failed'); }
    }

    async function remove(id: string) {
        try { await deleteAgentIdentity(id); await refresh(); } catch { setMsg('Delete failed'); }
    }

    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
            {msg && <div style={{ padding: '6px 10px', background: '#2a1a1a', borderRadius: 4, fontSize: 11, color: '#d06060' }}>{msg}</div>}

            <Section title={`Agent Identities (${agents.length})`}
                action={<button onClick={() => setCreating(c => !c)} style={btnPrimary}>+ New Agent</button>}>
                {creating && (
                    <div style={{ background: '#111', border: '1px solid #2a2a2a', borderRadius: 6, padding: 12, marginBottom: 10, display: 'flex', flexDirection: 'column', gap: 8 }}>
                        <input placeholder="Name" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                            style={inputStyle} />
                        <input placeholder="Description" value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))}
                            style={inputStyle} />
                        <input placeholder="Model (e.g. claude-sonnet-4-6)" value={form.model} onChange={e => setForm(f => ({ ...f, model: e.target.value }))}
                            style={inputStyle} />
                        <input placeholder="Capabilities (comma-separated)" value={form.capabilities} onChange={e => setForm(f => ({ ...f, capabilities: e.target.value }))}
                            style={inputStyle} />
                        <div style={{ display: 'flex', gap: 8 }}>
                            <button onClick={create} style={btnPrimary}>Create</button>
                            <button onClick={() => setCreating(false)} style={btnSecondary}>Cancel</button>
                        </div>
                    </div>
                )}
                {agents.length === 0 && !creating && <div style={{ fontSize: 12, color: '#444' }}>No agent identities yet.</div>}
                {agents.map(a => (
                    <div key={a.id} style={{ padding: '8px 10px', background: '#0d0d0d', border: '1px solid #1e1e1e', borderRadius: 4, marginBottom: 6 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                            <span style={{ fontSize: 13, color: '#ccc', fontWeight: 700, flex: 1 }}>{a.name}</span>
                            <Badge label={a.status} color={STATUS_COLOR[a.status] ?? '#888'} />
                        </div>
                        <div style={{ fontSize: 11, color: '#666', marginTop: 2 }}>{a.description}</div>
                        <div style={{ fontSize: 10, color: '#555', marginTop: 3, fontFamily: 'monospace' }}>model: {a.model} · {a.id}</div>
                        {a.capabilities.length > 0 && (
                            <div style={{ display: 'flex', gap: 4, marginTop: 5, flexWrap: 'wrap' }}>
                                {a.capabilities.map(c => <span key={c} style={{ fontSize: 10, color: '#60b0ff', border: '1px solid #1a3a4a', borderRadius: 3, padding: '1px 4px' }}>{c}</span>)}
                            </div>
                        )}
                        <div style={{ display: 'flex', gap: 6, marginTop: 8 }}>
                            {a.status === 'active' && <button onClick={() => setStatus(a.id, 'suspended')} style={btnSecondary}>Suspend</button>}
                            {a.status === 'suspended' && <button onClick={() => setStatus(a.id, 'active')} style={btnPrimary}>Activate</button>}
                            {a.status !== 'retired' && <button onClick={() => setStatus(a.id, 'retired')} style={{ ...btnSecondary, color: '#d06060', borderColor: '#3a1a1a' }}>Retire</button>}
                            <button onClick={() => remove(a.id)} style={{ ...btnSecondary, color: '#d06060', borderColor: '#3a1a1a' }}>Delete</button>
                        </div>
                    </div>
                ))}
            </Section>
        </div>
    );
}

// ─── API Keys Tab ─────────────────────────────────────────────────────────────

function ApiKeysTab() {
    const [keys, setKeys]         = React.useState<ApiKey[]>([]);
    const [creating, setCreating] = React.useState(false);
    const [form, setForm]         = React.useState({ name: '', scopes: 'runs:write,tools:invoke' });
    const [revealed, setRevealed] = React.useState<string | null>(null); // raw key shown once
    const [msg, setMsg]           = React.useState('');

    async function refresh() { try { setKeys(await listApiKeys()); } catch { /* ignore */ } }
    React.useEffect(() => { refresh(); }, []);

    async function create() {
        try {
            const { raw, ...key } = await createApiKey({ name: form.name, scopes: form.scopes.split(',').map(s => s.trim()).filter(Boolean) });
            setRevealed(raw ?? null);
            setCreating(false); setForm({ name: '', scopes: 'runs:write,tools:invoke' }); await refresh();
        } catch (err: unknown) { setMsg(err instanceof Error ? err.message : 'Create failed'); }
    }

    async function revoke(id: string) {
        try { await revokeApiKey(id); await refresh(); } catch { setMsg('Revoke failed'); }
    }

    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
            {msg && <div style={{ padding: '6px 10px', background: '#2a1a1a', borderRadius: 4, fontSize: 11, color: '#d06060' }}>{msg}</div>}
            {revealed && (
                <div style={{ padding: 12, background: '#1a2a1a', border: '1px solid #2a4a2a', borderRadius: 6 }}>
                    <div style={{ fontSize: 11, color: '#60d0a0', marginBottom: 6, fontWeight: 700 }}>API key created — copy it now, it won't be shown again:</div>
                    <code style={{ fontSize: 11, color: '#a0e0a0', wordBreak: 'break-all', fontFamily: 'monospace' }}>{revealed}</code>
                    <button onClick={() => setRevealed(null)} style={{ ...btnSecondary, marginTop: 8, display: 'block' }}>Dismiss</button>
                </div>
            )}

            <Section title={`API Keys (${keys.length})`}
                action={<button onClick={() => setCreating(c => !c)} style={btnPrimary}>+ New Key</button>}>
                {creating && (
                    <div style={{ background: '#111', border: '1px solid #2a2a2a', borderRadius: 6, padding: 12, marginBottom: 10, display: 'flex', flexDirection: 'column', gap: 8 }}>
                        <input placeholder="Key name (e.g. CI/CD pipeline)" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                            style={inputStyle} />
                        <input placeholder="Scopes (comma-separated)" value={form.scopes} onChange={e => setForm(f => ({ ...f, scopes: e.target.value }))}
                            style={inputStyle} />
                        <div style={{ fontSize: 10, color: '#555' }}>Available scopes: runs:write, runs:read, tools:invoke, knowledge:read, knowledge:write</div>
                        <div style={{ display: 'flex', gap: 8 }}>
                            <button onClick={create} style={btnPrimary}>Create</button>
                            <button onClick={() => setCreating(false)} style={btnSecondary}>Cancel</button>
                        </div>
                    </div>
                )}
                {keys.length === 0 && !creating && <div style={{ fontSize: 12, color: '#444' }}>No API keys. Create one to access the backend programmatically.</div>}
                {keys.map(k => (
                    <div key={k.id} style={{ padding: '8px 10px', background: '#0d0d0d', border: '1px solid #1e1e1e', borderRadius: 4, marginBottom: 6 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                            <span style={{ fontSize: 13, color: '#ccc', fontWeight: 700, flex: 1 }}>{k.name}</span>
                            <code style={{ fontSize: 10, color: '#60b0ff', fontFamily: 'monospace' }}>{k.prefix}…</code>
                        </div>
                        <div style={{ display: 'flex', gap: 12, marginTop: 4, fontSize: 10, color: '#555', fontFamily: 'monospace' }}>
                            <span>created {k.createdAt.slice(0, 10)}</span>
                            {k.lastUsedAt && <span>last used {k.lastUsedAt.slice(0, 10)}</span>}
                            {k.expiresAt && <span>expires {k.expiresAt.slice(0, 10)}</span>}
                        </div>
                        <div style={{ display: 'flex', gap: 4, marginTop: 5, flexWrap: 'wrap' }}>
                            {k.scopes.map(s => <span key={s} style={{ fontSize: 10, color: '#d0a030', border: '1px solid #3a2a0a', borderRadius: 3, padding: '1px 4px' }}>{s}</span>)}
                        </div>
                        <button onClick={() => revoke(k.id)} style={{ ...btnSecondary, marginTop: 8, color: '#d06060', borderColor: '#3a1a1a' }}>Revoke</button>
                    </div>
                ))}
            </Section>
        </div>
    );
}

// ─── Org Tab ──────────────────────────────────────────────────────────────────

function OrgTab() {
    const [orgs, setOrgs]         = React.useState<OrgRecord[]>([]);
    const [members, setMembers]   = React.useState<OrgMember[]>([]);
    const [teams, setTeams]       = React.useState<TeamRecord[]>([]);
    const [selOrg, setSelOrg]     = React.useState<OrgRecord | null>(null);
    const [creating, setCreating] = React.useState(false);
    const [orgName, setOrgName]   = React.useState('');
    const [newTeam, setNewTeam]   = React.useState('');
    const [msg, setMsg]           = React.useState('');

    async function loadOrgs() {
        try {
            const list = await listOrgs();
            setOrgs(list);
            if (list.length > 0 && !selOrg) selectOrg(list[0]!);
        } catch { setMsg('Backend not reachable'); }
    }

    async function selectOrg(org: OrgRecord) {
        setSelOrg(org);
        try {
            const [m, t] = await Promise.all([listOrgMembers(org.id), listOrgTeams(org.id)]);
            setMembers(m); setTeams(t);
        } catch { /* ignore */ }
    }

    React.useEffect(() => { loadOrgs(); }, []);

    async function createO() {
        try { const org = await createOrg({ name: orgName }); setCreating(false); setOrgName(''); await loadOrgs(); selectOrg(org); }
        catch (err: unknown) { setMsg(err instanceof Error ? err.message : 'Create failed'); }
    }

    async function createT() {
        if (!selOrg || !newTeam) return;
        try { await createOrgTeam(selOrg.id, { name: newTeam }); setNewTeam(''); selectOrg(selOrg); }
        catch { setMsg('Team create failed'); }
    }

    return (
        <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
            {msg && <div style={{ padding: '6px 10px', background: '#2a1a1a', borderRadius: 4, fontSize: 11, color: '#d06060' }}>{msg}</div>}

            <Section title="Organizations" action={<button onClick={() => setCreating(c => !c)} style={btnPrimary}>+ New Org</button>}>
                {creating && (
                    <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
                        <input placeholder="Organization name" value={orgName} onChange={e => setOrgName(e.target.value)} style={{ ...inputStyle, flex: 1 }} />
                        <button onClick={createO} style={btnPrimary}>Create</button>
                        <button onClick={() => setCreating(false)} style={btnSecondary}>✕</button>
                    </div>
                )}
                {orgs.length === 0 && !creating && <div style={{ fontSize: 12, color: '#444' }}>No organizations yet.</div>}
                {orgs.map(o => (
                    <div key={o.id} onClick={() => selectOrg(o)}
                        style={{ padding: '6px 10px', borderRadius: 4, cursor: 'pointer', border: `1px solid ${selOrg?.id === o.id ? '#2a4a6a' : '#1e1e1e'}`, background: selOrg?.id === o.id ? '#111a2a' : '#0d0d0d', marginBottom: 4 }}>
                        <div style={{ fontSize: 13, color: '#ccc', fontWeight: selOrg?.id === o.id ? 700 : 400 }}>{o.name}</div>
                        <div style={{ fontSize: 10, color: '#555', fontFamily: 'monospace' }}>{o.slug} · {o.id}</div>
                    </div>
                ))}
            </Section>

            {selOrg && (
                <>
                    <Section title={`Members (${members.length})`}>
                        {members.map(m => (
                            <div key={m.userId} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 0', borderBottom: '1px solid #111' }}>
                                <span style={{ fontSize: 12, color: '#aaa', flex: 1, fontFamily: 'monospace' }}>{m.userId}</span>
                                <Badge label={m.role} color={ROLE_COLOR[m.role] ?? '#888'} />
                            </div>
                        ))}
                    </Section>

                    <Section title={`Teams (${teams.length})`}
                        action={
                            <div style={{ display: 'flex', gap: 6 }}>
                                <input placeholder="Team name" value={newTeam} onChange={e => setNewTeam(e.target.value)}
                                    style={{ ...inputStyle, padding: '2px 6px', fontSize: 11 }} />
                                <button onClick={createT} style={btnPrimary}>+</button>
                            </div>
                        }>
                        {teams.length === 0 && <div style={{ fontSize: 12, color: '#444' }}>No teams yet.</div>}
                        {teams.map(t => (
                            <div key={t.id} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 0', borderBottom: '1px solid #111' }}>
                                <span style={{ fontSize: 12, color: '#aaa', flex: 1 }}>{t.name}</span>
                                {t.description && <span style={{ fontSize: 10, color: '#555' }}>{t.description}</span>}
                            </div>
                        ))}
                    </Section>
                </>
            )}
        </div>
    );
}

// ─── Main panel ───────────────────────────────────────────────────────────────

const TABS: { id: IdentityTab; label: string; icon: string }[] = [
    { id: 'profile', label: 'Profile',   icon: '◉' },
    { id: 'agents',  label: 'Agents',    icon: '⬡' },
    { id: 'keys',    label: 'API Keys',  icon: '⚿' },
    { id: 'org',     label: 'Org',       icon: '⊞' },
];

function IdentityView() {
    const [tab, setTab] = React.useState<IdentityTab>('profile');
    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0d0d0d', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #1e1e1e', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontWeight: 700, fontSize: 13, color: '#60b0ff' }}>◉ Identity</span>
                <span style={{ fontSize: 10, color: '#333' }}>Lifecycle Management</span>
            </div>
            <div style={{ display: 'flex', borderBottom: '1px solid #1a1a1a', background: '#0a0a0a' }}>
                {TABS.map(t => (
                    <button key={t.id} onClick={() => setTab(t.id)} style={{
                        padding: '6px 14px', border: 'none', background: 'transparent',
                        color: tab === t.id ? '#60b0ff' : '#555',
                        borderBottom: tab === t.id ? '2px solid #60b0ff' : '2px solid transparent',
                        cursor: 'pointer', fontSize: 12, fontWeight: tab === t.id ? 700 : 400,
                        display: 'flex', gap: 5, alignItems: 'center',
                    }}>
                        <span>{t.icon}</span>{t.label}
                    </button>
                ))}
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {tab === 'profile' && <ProfileTab />}
                {tab === 'agents'  && <AgentsTab />}
                {tab === 'keys'    && <ApiKeysTab />}
                {tab === 'org'     && <OrgTab />}
            </div>
        </div>
    );
}

const btnPrimary: React.CSSProperties = {
    padding: '4px 12px', background: '#1a2a3a', border: '1px solid #2a4a6a',
    color: '#60b0ff', borderRadius: 4, cursor: 'pointer', fontSize: 12, fontWeight: 700,
};

const btnSecondary: React.CSSProperties = {
    padding: '4px 10px', background: '#111', border: '1px solid #2a2a2a',
    color: '#888', borderRadius: 4, cursor: 'pointer', fontSize: 11,
};

const inputStyle: React.CSSProperties = {
    background: '#1a1a1a', border: '1px solid #333', color: '#ddd',
    padding: '5px 8px', borderRadius: 4, fontSize: 12, width: '100%',
};

@injectable()
export class IdentityPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:identity';
    static readonly LABEL = 'Identity';

    @postConstruct()
    protected init(): void {
        this.id = IdentityPanelWidget.ID;
        this.title.label = IdentityPanelWidget.LABEL;
        this.title.caption = 'Identity & lifecycle management';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-account';
        this.update();
    }

    protected render(): React.ReactNode { return <IdentityView />; }
}

@injectable()
export class IdentityPanelContribution extends AbstractViewContribution<IdentityPanelWidget> {
    constructor() {
        super({ widgetId: IdentityPanelWidget.ID, widgetName: IdentityPanelWidget.LABEL, defaultWidgetOptions: { area: 'left' }, toggleCommandId: IdentityPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(IdentityPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
