import * as fs from 'fs';
import * as path from 'path';
import { v4 as uuidv4 } from 'uuid';

export type OrgRole  = 'owner' | 'admin' | 'member' | 'viewer';
export type TeamRole = 'lead' | 'member';

export interface OrgRecord {
    id:           string;
    name:         string;
    slug:         string;
    ownerId:      string;
    description?: string;
    createdAt:    string;
}

export interface TeamRecord {
    id:           string;
    orgId:        string;
    name:         string;
    description?: string;
    createdAt:    string;
}

export interface OrgMember {
    orgId:    string;
    userId:   string;
    role:     OrgRole;
    joinedAt: string;
}

export interface TeamMember {
    teamId:   string;
    userId:   string;
    role:     TeamRole;
    joinedAt: string;
}

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

class OrgManager {
    private orgs        = new Map<string, OrgRecord>();
    private teams       = new Map<string, TeamRecord>();
    private orgMembers  = new Map<string, OrgMember>();   // key: orgId:userId
    private teamMembers = new Map<string, TeamMember>();  // key: teamId:userId

    constructor() { this.load(); }

    private load(): void {
        for (const o of loadJson<OrgRecord[]>('.identity-orgs.json', []))           this.orgs.set(o.id, o);
        for (const t of loadJson<TeamRecord[]>('.identity-teams.json', []))          this.teams.set(t.id, t);
        for (const m of loadJson<OrgMember[]>('.identity-org-members.json', []))     this.orgMembers.set(`${m.orgId}:${m.userId}`, m);
        for (const m of loadJson<TeamMember[]>('.identity-team-members.json', []))   this.teamMembers.set(`${m.teamId}:${m.userId}`, m);
    }

    private persist(): void {
        saveJson('.identity-orgs.json',         [...this.orgs.values()]);
        saveJson('.identity-teams.json',        [...this.teams.values()]);
        saveJson('.identity-org-members.json',  [...this.orgMembers.values()]);
        saveJson('.identity-team-members.json', [...this.teamMembers.values()]);
    }

    // ─ Orgs ──────────────────────────────────────────────────────────────────

    createOrg(name: string, ownerId: string, description?: string): OrgRecord {
        const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
        const org: OrgRecord = {
            id:   `org_${uuidv4().replace(/-/g, '').slice(0, 12)}`,
            name, slug, ownerId,
            description,
            createdAt: new Date().toISOString(),
        };
        this.orgs.set(org.id, org);
        this.addMember(org.id, ownerId, 'owner');
        this.persist();
        return org;
    }

    getOrg(id: string): OrgRecord | undefined { return this.orgs.get(id); }

    listOrgs(userId: string): OrgRecord[] {
        const memberOrgIds = new Set(
            [...this.orgMembers.values()].filter(m => m.userId === userId).map(m => m.orgId)
        );
        return [...this.orgs.values()].filter(o => memberOrgIds.has(o.id));
    }

    updateOrg(id: string, patch: Partial<Pick<OrgRecord, 'name' | 'description'>>): OrgRecord | null {
        const o = this.orgs.get(id);
        if (!o) return null;
        Object.assign(o, patch);
        this.persist();
        return o;
    }

    // ─ Members ───────────────────────────────────────────────────────────────

    addMember(orgId: string, userId: string, role: OrgRole): OrgMember {
        const m: OrgMember = { orgId, userId, role, joinedAt: new Date().toISOString() };
        this.orgMembers.set(`${orgId}:${userId}`, m);
        this.persist();
        return m;
    }

    removeMember(orgId: string, userId: string): boolean {
        const ok = this.orgMembers.delete(`${orgId}:${userId}`);
        if (ok) this.persist();
        return ok;
    }

    updateMemberRole(orgId: string, userId: string, role: OrgRole): boolean {
        const m = this.orgMembers.get(`${orgId}:${userId}`);
        if (!m) return false;
        m.role = role;
        this.persist();
        return true;
    }

    listMembers(orgId: string): OrgMember[] {
        return [...this.orgMembers.values()].filter(m => m.orgId === orgId);
    }

    getMember(orgId: string, userId: string): OrgMember | undefined {
        return this.orgMembers.get(`${orgId}:${userId}`);
    }

    // ─ Teams ─────────────────────────────────────────────────────────────────

    createTeam(orgId: string, name: string, description?: string): TeamRecord {
        const team: TeamRecord = {
            id:   `team_${uuidv4().replace(/-/g, '').slice(0, 12)}`,
            orgId, name,
            description,
            createdAt: new Date().toISOString(),
        };
        this.teams.set(team.id, team);
        this.persist();
        return team;
    }

    getTeam(id: string): TeamRecord | undefined { return this.teams.get(id); }
    listTeams(orgId: string): TeamRecord[] {
        return [...this.teams.values()].filter(t => t.orgId === orgId);
    }

    addTeamMember(teamId: string, userId: string, role: TeamRole = 'member'): TeamMember {
        const m: TeamMember = { teamId, userId, role, joinedAt: new Date().toISOString() };
        this.teamMembers.set(`${teamId}:${userId}`, m);
        this.persist();
        return m;
    }

    removeTeamMember(teamId: string, userId: string): boolean {
        const ok = this.teamMembers.delete(`${teamId}:${userId}`);
        if (ok) this.persist();
        return ok;
    }

    listTeamMembers(teamId: string): TeamMember[] {
        return [...this.teamMembers.values()].filter(m => m.teamId === teamId);
    }

    ensureDemoOrg(ownerId: string): OrgRecord {
        const existing = this.listOrgs(ownerId);
        if (existing.length > 0) return existing[0]!;
        return this.createOrg('My Organization', ownerId, 'Default organization');
    }
}

export const orgManager = new OrgManager();
