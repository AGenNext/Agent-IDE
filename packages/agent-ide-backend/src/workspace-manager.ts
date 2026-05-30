import { v4 as uuidv4 } from 'uuid';
import * as fs from 'fs/promises';
import * as path from 'path';

export type WorkspaceStatus = 'active' | 'inactive' | 'provisioning' | 'error';

export interface WorkspaceRecord {
    id:          string;
    tenantId:    string;    // userId — the owning tenant
    name:        string;
    status:      WorkspaceStatus;
    createdAt:   string;
    updatedAt:   string;
    rootPath?:   string;    // Docker: isolated mount point (future)
    mcpConfig?:  string;    // path to workspace-scoped .mcp.json (future)
}

const STORE_PATH = path.join(process.env.WORKSPACE_ROOT ?? process.cwd(), '.workspaces.json');

class WorkspaceManager {
    private workspaces = new Map<string, WorkspaceRecord>();
    private loaded = false;

    async load(): Promise<void> {
        try {
            const raw = await fs.readFile(STORE_PATH, 'utf-8');
            const list = JSON.parse(raw) as WorkspaceRecord[];
            for (const w of list) this.workspaces.set(w.id, w);
        } catch { /* no file yet */ }
        this.loaded = true;
    }

    private async persist(): Promise<void> {
        if (!this.loaded) return;
        await fs.mkdir(path.dirname(STORE_PATH), { recursive: true });
        await fs.writeFile(STORE_PATH, JSON.stringify([...this.workspaces.values()], null, 2));
    }

    list(tenantId: string): WorkspaceRecord[] {
        return [...this.workspaces.values()].filter(w => w.tenantId === tenantId);
    }

    listAll(): WorkspaceRecord[] {
        return [...this.workspaces.values()];
    }

    get(id: string): WorkspaceRecord | undefined {
        return this.workspaces.get(id);
    }

    async create(tenantId: string, name: string): Promise<WorkspaceRecord> {
        const id = `ws_${uuidv4().replace(/-/g, '').slice(0, 12)}`;
        const workspace: WorkspaceRecord = {
            id,
            tenantId,
            name,
            status: 'active',
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            rootPath: path.join(process.env.WORKSPACE_ROOT ?? process.cwd(), 'tenants', tenantId, id),
        };
        this.workspaces.set(id, workspace);
        await this.persist();
        return workspace;
    }

    async rename(id: string, name: string): Promise<WorkspaceRecord | null> {
        const w = this.workspaces.get(id);
        if (!w) return null;
        w.name = name;
        w.updatedAt = new Date().toISOString();
        await this.persist();
        return w;
    }

    async setStatus(id: string, status: WorkspaceStatus): Promise<WorkspaceRecord | null> {
        const w = this.workspaces.get(id);
        if (!w) return null;
        w.status = status;
        w.updatedAt = new Date().toISOString();
        await this.persist();
        return w;
    }

    async delete(id: string): Promise<boolean> {
        if (!this.workspaces.has(id)) return false;
        this.workspaces.delete(id);
        await this.persist();
        return true;
    }

    // Ensure each tenant always has at least one workspace
    async ensureDefaultWorkspace(tenantId: string, name: string): Promise<WorkspaceRecord> {
        const existing = this.list(tenantId);
        if (existing.length > 0) return existing[0];
        return this.create(tenantId, name);
    }
}

export const workspaceManager = new WorkspaceManager();
