const SESSION_KEY = 'agent-ide:session';

export interface Session {
    token:             string;
    userId:            string;
    email:             string;
    name:              string;
    activeWorkspaceId?: string;
}

export function getSession(): Session | null {
    try {
        if (typeof localStorage === 'undefined') return null;
        const raw = localStorage.getItem(SESSION_KEY);
        return raw ? JSON.parse(raw) as Session : null;
    } catch { return null; }
}

export function setSession(session: Session): void {
    try {
        if (typeof localStorage !== 'undefined') localStorage.setItem(SESSION_KEY, JSON.stringify(session));
    } catch { /* ignore quota/security errors */ }
}

export function updateSession(patch: Partial<Session>): void {
    const existing = getSession();
    if (!existing) return;
    setSession({ ...existing, ...patch });
}

export function clearSession(): void {
    try {
        if (typeof localStorage !== 'undefined') localStorage.removeItem(SESSION_KEY);
    } catch { /* ignore */ }
}
