import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';
import { v4 as uuidv4 } from 'uuid';

export type UserRole    = 'admin' | 'developer' | 'viewer';
export type UserStatus  = 'active' | 'suspended' | 'pending';
export type AgentStatus = 'active' | 'suspended' | 'retired';

export interface UserRecord {
    userId:       string;
    email:        string;
    name:         string;
    avatarUrl?:   string;
    role:         UserRole;
    passwordHash: string;  // pbkdf2-sha256: salt:iterations:hash
    createdAt:    string;
    lastLoginAt?: string;
    mfaEnabled:   boolean;
    status:       UserStatus;
}

export interface AgentIdentity {
    id:           string;
    name:         string;
    description:  string;
    ownerId:      string;
    orgId?:       string;
    model:        string;
    status:       AgentStatus;
    capabilities: string[];
    createdAt:    string;
    updatedAt:    string;
}

export interface ApiKey {
    id:          string;
    name:        string;
    prefix:      string;
    keyHash:     string;  // sha-256 of raw key
    userId:      string;
    agentId?:    string;
    scopes:      string[];
    lastUsedAt?: string;
    expiresAt?:  string;
    createdAt:   string;
    revoked:     boolean;
}

// ─── Storage helpers ──────────────────────────────────────────────────────────

function dataFile(name: string): string {
    return path.join(process.env.DATA_DIR ?? '.', name);
}

function loadJson<T>(filename: string, def: T): T {
    try {
        const p = dataFile(filename);
        if (!fs.existsSync(p)) return def;
        return JSON.parse(fs.readFileSync(p, 'utf8')) as T;
    } catch { return def; }
}

function saveJson(filename: string, data: unknown): void {
    try {
        const p = dataFile(filename);
        const dir = path.dirname(p);
        if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
        fs.writeFileSync(p, JSON.stringify(data, null, 2));
    } catch { /* non-fatal */ }
}

// ─── Password hashing (PBKDF2, no external deps) ─────────────────────────────

function hashPassword(password: string): string {
    const salt = crypto.randomBytes(16).toString('hex');
    const iter = 100_000;
    const hash = crypto.pbkdf2Sync(password, salt, iter, 32, 'sha256').toString('hex');
    return `${salt}:${iter}:${hash}`;
}

function verifyPassword(password: string, stored: string): boolean {
    const parts = stored.split(':');
    if (parts.length !== 3) return false;
    const [salt, iterStr, expected] = parts as [string, string, string];
    const hash = crypto.pbkdf2Sync(password, salt, Number(iterStr), 32, 'sha256').toString('hex');
    try { return crypto.timingSafeEqual(Buffer.from(hash, 'hex'), Buffer.from(expected, 'hex')); }
    catch { return false; }
}

// ─── API key generation ───────────────────────────────────────────────────────

export function generateApiKey(): { raw: string; prefix: string; keyHash: string } {
    const raw     = `aik_${crypto.randomBytes(24).toString('base64url')}`;
    const prefix  = raw.slice(0, 12);
    const keyHash = crypto.createHash('sha256').update(raw).digest('hex');
    return { raw, prefix, keyHash };
}

// ─── Identity Store ───────────────────────────────────────────────────────────

class IdentityStore {
    private users   = new Map<string, UserRecord>();
    private byEmail = new Map<string, string>();
    private agents  = new Map<string, AgentIdentity>();
    private apiKeys = new Map<string, ApiKey>();

    constructor() { this.load(); }

    private load(): void {
        for (const u of loadJson<UserRecord[]>('.identity-users.json', [])) {
            this.users.set(u.userId, u);
            this.byEmail.set(u.email.toLowerCase(), u.userId);
        }
        for (const a of loadJson<AgentIdentity[]>('.identity-agents.json', [])) this.agents.set(a.id, a);
        for (const k of loadJson<ApiKey[]>('.identity-keys.json', [])) this.apiKeys.set(k.id, k);
    }

    private persist(): void {
        saveJson('.identity-users.json',  [...this.users.values()]);
        saveJson('.identity-agents.json', [...this.agents.values()]);
        saveJson('.identity-keys.json',   [...this.apiKeys.values()]);
    }

    // ─ Users ─────────────────────────────────────────────────────────────────

    createUser(email: string, name: string, password: string, role: UserRole = 'developer'): UserRecord {
        if (this.byEmail.has(email.toLowerCase())) throw new Error('Email already registered');
        const u: UserRecord = {
            userId:       `user_${uuidv4().replace(/-/g, '').slice(0, 12)}`,
            email, name, role,
            passwordHash: hashPassword(password),
            createdAt:    new Date().toISOString(),
            mfaEnabled:   false,
            status:       'active',
        };
        this.users.set(u.userId, u);
        this.byEmail.set(email.toLowerCase(), u.userId);
        this.persist();
        return u;
    }

    authenticatePassword(email: string, password: string): UserRecord | null {
        const id = this.byEmail.get(email.toLowerCase());
        if (!id) return null;
        const u = this.users.get(id);
        if (!u || u.status !== 'active') return null;
        if (!verifyPassword(password, u.passwordHash)) return null;
        u.lastLoginAt = new Date().toISOString();
        this.persist();
        return u;
    }

    authenticateApiKey(raw: string): UserRecord | null {
        const keyHash = crypto.createHash('sha256').update(raw).digest('hex');
        for (const k of this.apiKeys.values()) {
            if (k.keyHash !== keyHash || k.revoked) continue;
            if (k.expiresAt && new Date(k.expiresAt) < new Date()) continue;
            k.lastUsedAt = new Date().toISOString();
            this.persist();
            return this.users.get(k.userId) ?? null;
        }
        return null;
    }

    getUser(userId: string): UserRecord | undefined { return this.users.get(userId); }
    getUserByEmail(email: string): UserRecord | undefined {
        const id = this.byEmail.get(email.toLowerCase());
        return id ? this.users.get(id) : undefined;
    }
    listUsers(): UserRecord[] { return [...this.users.values()]; }

    updateUser(userId: string, patch: Partial<Pick<UserRecord, 'name' | 'avatarUrl' | 'role' | 'status' | 'mfaEnabled'>>): UserRecord | null {
        const u = this.users.get(userId);
        if (!u) return null;
        Object.assign(u, patch);
        this.persist();
        return u;
    }

    changePassword(userId: string, password: string): boolean {
        const u = this.users.get(userId);
        if (!u) return false;
        u.passwordHash = hashPassword(password);
        this.persist();
        return true;
    }

    ensureDemoUser(): void {
        if (this.byEmail.has('demo@agent-ide.local')) return;
        this.createUser('demo@agent-ide.local', 'Demo User', 'demo', 'admin');
    }

    // ─ Agent Identities ───────────────────────────────────────────────────────

    createAgent(data: Omit<AgentIdentity, 'id' | 'createdAt' | 'updatedAt'>): AgentIdentity {
        const a: AgentIdentity = {
            ...data,
            id:        `agt_${uuidv4().replace(/-/g, '').slice(0, 12)}`,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        };
        this.agents.set(a.id, a);
        this.persist();
        return a;
    }

    getAgent(id: string): AgentIdentity | undefined { return this.agents.get(id); }
    listAgents(ownerId?: string): AgentIdentity[] {
        const all = [...this.agents.values()];
        return ownerId ? all.filter(a => a.ownerId === ownerId) : all;
    }

    updateAgent(id: string, patch: Partial<Pick<AgentIdentity, 'name' | 'description' | 'model' | 'status' | 'capabilities'>>): AgentIdentity | null {
        const a = this.agents.get(id);
        if (!a) return null;
        Object.assign(a, patch, { updatedAt: new Date().toISOString() });
        this.persist();
        return a;
    }

    deleteAgent(id: string): boolean {
        const ok = this.agents.delete(id);
        if (ok) this.persist();
        return ok;
    }

    // ─ API Keys ───────────────────────────────────────────────────────────────

    createApiKey(userId: string, name: string, scopes: string[], agentId?: string, expiresAt?: string): { key: ApiKey; raw: string } {
        const { raw, prefix, keyHash } = generateApiKey();
        const key: ApiKey = {
            id:        `key_${uuidv4().replace(/-/g, '').slice(0, 12)}`,
            name, prefix, keyHash, userId,
            agentId, scopes,
            expiresAt,
            createdAt: new Date().toISOString(),
            revoked:   false,
        };
        this.apiKeys.set(key.id, key);
        this.persist();
        return { key, raw };
    }

    listApiKeys(userId: string): ApiKey[] {
        return [...this.apiKeys.values()].filter(k => k.userId === userId && !k.revoked);
    }

    revokeApiKey(id: string, userId: string): boolean {
        const k = this.apiKeys.get(id);
        if (!k || k.userId !== userId) return false;
        k.revoked = true;
        this.persist();
        return true;
    }
}

export const identityStore = new IdentityStore();
