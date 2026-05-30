import * as fs from 'fs/promises';
import * as path from 'path';
import { v4 as uuidv4 } from 'uuid';

export type PolicyDecisionAction = 'allow' | 'deny' | 'require-approval';

export interface PolicyRule {
    id:       string;
    tools:    string[];           // exact tool ids or '*' wildcard
    action:   PolicyDecisionAction;
    reason:   string;
}

export interface Policy {
    id:          string;
    tenantId:    string;
    name:        string;
    description: string;
    enabled:     boolean;
    priority:    number;          // lower = evaluated first
    rules:       PolicyRule[];
    version:     number;
    createdAt:   string;
    updatedAt:   string;
}

export interface PolicyContext {
    toolId:   string;
    agentId:  string;
    runId:    string;
    tenantId: string;
    input:    Record<string, unknown>;
}

export interface PolicyDecision {
    action:    PolicyDecisionAction;
    policyId?: string;
    ruleId?:   string;
    reason:    string;
}

const STORE_PATH = path.join(process.env.WORKSPACE_ROOT ?? process.cwd(), '.policies.json');

const BUILTIN_POLICIES: Omit<Policy, 'tenantId'>[] = [
    {
        id: 'builtin-tool-safety', name: 'Tool Safety', description: 'Shell and code execution require approval; system paths are blocked.',
        enabled: true, priority: 1, version: 1,
        createdAt: new Date().toISOString(), updatedAt: new Date().toISOString(),
        rules: [
            { id: 'r-shell',    tools: ['shell', 'code_exec'], action: 'require-approval', reason: 'Shell and code execution require human approval.' },
            { id: 'r-db',       tools: ['db_query'],           action: 'require-approval', reason: 'Database writes require human approval.' },
            { id: 'r-allow-*',  tools: ['*'],                  action: 'allow',             reason: 'All other tools are allowed by default.' },
        ],
    },
];

class PolicyEngine {
    private policies = new Map<string, Policy>();
    private loaded = false;

    async load(): Promise<void> {
        try {
            const raw = await fs.readFile(STORE_PATH, 'utf-8');
            const list = JSON.parse(raw) as Policy[];
            for (const p of list) this.policies.set(p.id, p);
        } catch { /* no file yet */ }
        this.loaded = true;
    }

    private async persist(): Promise<void> {
        if (!this.loaded) return;
        await fs.mkdir(path.dirname(STORE_PATH), { recursive: true });
        await fs.writeFile(STORE_PATH, JSON.stringify([...this.policies.values()], null, 2));
    }

    // Seed built-in policies for a tenant if they don't have any yet
    async ensureDefaults(tenantId: string): Promise<void> {
        const has = [...this.policies.values()].some(p => p.tenantId === tenantId);
        if (has) return;
        for (const bp of BUILTIN_POLICIES) {
            const policy: Policy = { ...bp, tenantId, id: `${bp.id}-${tenantId.slice(0, 8)}` };
            this.policies.set(policy.id, policy);
        }
        await this.persist();
    }

    list(tenantId: string): Policy[] {
        return [...this.policies.values()]
            .filter(p => p.tenantId === tenantId)
            .sort((a, b) => a.priority - b.priority);
    }

    get(id: string): Policy | undefined { return this.policies.get(id); }

    async create(tenantId: string, data: Omit<Policy, 'id' | 'tenantId' | 'createdAt' | 'updatedAt' | 'version'>): Promise<Policy> {
        const now = new Date().toISOString();
        const policy: Policy = { ...data, id: `policy_${uuidv4().slice(0, 8)}`, tenantId, version: 1, createdAt: now, updatedAt: now };
        this.policies.set(policy.id, policy);
        await this.persist();
        return policy;
    }

    async update(id: string, patch: Partial<Pick<Policy, 'name' | 'description' | 'enabled' | 'priority' | 'rules'>>): Promise<Policy | null> {
        const p = this.policies.get(id);
        if (!p) return null;
        Object.assign(p, patch, { updatedAt: new Date().toISOString(), version: p.version + 1 });
        await this.persist();
        return p;
    }

    async delete(id: string): Promise<boolean> {
        if (!this.policies.has(id)) return false;
        this.policies.delete(id);
        await this.persist();
        return true;
    }

    evaluate(ctx: PolicyContext): PolicyDecision {
        const policies = this.list(ctx.tenantId).filter(p => p.enabled);
        for (const policy of policies) {
            for (const rule of policy.rules) {
                const matches = rule.tools.some(t => t === '*' || t === ctx.toolId);
                if (!matches) continue;
                return { action: rule.action, policyId: policy.id, ruleId: rule.id, reason: rule.reason };
            }
        }
        return { action: 'allow', reason: 'No matching policy rule — default allow.' };
    }
}

export const policyEngine = new PolicyEngine();
