import * as crypto from 'crypto';
import * as fs from 'fs/promises';
import * as path from 'path';

export interface AuditEntry {
    id:         string;
    tenantId:   string;
    timestamp:  string;
    event:      string;           // 'tool:call' | 'tool:blocked' | 'tool:approved' | 'tool:rejected' | 'run:started' | 'run:completed' | 'run:failed' | 'auth:login' | 'approval:requested'
    agentId?:   string;
    runId?:     string;
    toolId?:    string;
    decision?:  string;
    policyId?:  string;
    metadata:   Record<string, unknown>;
}

const LOG_PATH  = path.join(process.env.WORKSPACE_ROOT ?? process.cwd(), '.audit.jsonl');
const MAX_MEM   = 2000;

class AuditLog {
    private entries: AuditEntry[] = [];
    private loaded = false;

    async load(): Promise<void> {
        try {
            const raw = await fs.readFile(LOG_PATH, 'utf-8');
            const lines = raw.trim().split('\n').filter(Boolean);
            this.entries = lines.map(l => JSON.parse(l) as AuditEntry);
            if (this.entries.length > MAX_MEM) this.entries = this.entries.slice(-MAX_MEM);
        } catch { /* no file yet */ }
        this.loaded = true;
    }

    async append(entry: Omit<AuditEntry, 'id' | 'timestamp'>): Promise<AuditEntry> {
        const full: AuditEntry = {
            ...entry,
            id: crypto.randomBytes(6).toString('hex'),
            timestamp: new Date().toISOString(),
        };
        this.entries.push(full);
        if (this.entries.length > MAX_MEM) this.entries.shift();
        // Append to file (non-blocking)
        fs.mkdir(path.dirname(LOG_PATH), { recursive: true })
            .then(() => fs.appendFile(LOG_PATH, JSON.stringify(full) + '\n'))
            .catch(() => { /* ignore */ });
        return full;
    }

    list(tenantId: string, opts: { event?: string; toolId?: string; runId?: string; limit?: number } = {}): AuditEntry[] {
        let results = this.entries.filter(e => e.tenantId === tenantId || tenantId === '*');
        if (opts.event)  results = results.filter(e => e.event === opts.event);
        if (opts.toolId) results = results.filter(e => e.toolId === opts.toolId);
        if (opts.runId)  results = results.filter(e => e.runId === opts.runId);
        return results.slice(-(opts.limit ?? 200)).reverse();
    }

    count(tenantId: string): number {
        return this.entries.filter(e => e.tenantId === tenantId).length;
    }
}

export const auditLog = new AuditLog();
