import * as crypto from 'crypto';
import { broadcast } from './websocket';

export type ApprovalStatus = 'pending' | 'approved' | 'rejected' | 'timeout';

export interface PendingApproval {
    id:           string;
    tenantId:     string;
    runId:        string;
    agentId:      string;
    toolId:       string;
    input:        Record<string, unknown>;
    policyId?:    string;
    reason:       string;
    requestedAt:  string;
    status:       ApprovalStatus;
    resolvedAt?:  string;
    resolvedBy?:  string;
}

interface Waiter {
    resolve: (approved: boolean) => void;
    timer:   ReturnType<typeof setTimeout>;
}

const TIMEOUT_MS = 5 * 60 * 1000; // 5 minutes

class ApprovalGate {
    private approvals = new Map<string, PendingApproval>();
    private waiters   = new Map<string, Waiter>();

    list(tenantId: string): PendingApproval[] {
        return [...this.approvals.values()]
            .filter(a => a.tenantId === tenantId)
            .sort((a, b) => b.requestedAt.localeCompare(a.requestedAt));
    }

    get(id: string): PendingApproval | undefined { return this.approvals.get(id); }

    // Called by tool-proxy: suspends until a human approves or rejects.
    async request(ctx: {
        tenantId: string; runId: string; agentId: string;
        toolId: string; input: Record<string, unknown>;
        policyId?: string; reason: string;
    }): Promise<boolean> {
        const id = `appr_${crypto.randomBytes(6).toString('hex')}`;
        const approval: PendingApproval = {
            ...ctx, id,
            requestedAt: new Date().toISOString(),
            status: 'pending',
        };
        this.approvals.set(id, approval);

        // Notify frontend via WebSocket
        broadcast({ type: 'governance:approval-request', runId: ctx.runId, payload: approval });

        return new Promise<boolean>((resolve) => {
            const timer = setTimeout(() => {
                this.waiters.delete(id);
                const a = this.approvals.get(id);
                if (a) { a.status = 'timeout'; a.resolvedAt = new Date().toISOString(); }
                resolve(false);
            }, TIMEOUT_MS);
            this.waiters.set(id, { resolve, timer });
        });
    }

    resolve(id: string, approved: boolean, resolvedBy = 'operator'): boolean {
        const waiter = this.waiters.get(id);
        if (!waiter) return false;
        clearTimeout(waiter.timer);
        this.waiters.delete(id);
        const a = this.approvals.get(id);
        if (a) {
            a.status = approved ? 'approved' : 'rejected';
            a.resolvedAt = new Date().toISOString();
            a.resolvedBy = resolvedBy;
        }
        waiter.resolve(approved);
        broadcast({ type: 'governance:approval-resolved', runId: a?.runId ?? '', payload: a });
        return true;
    }

    // Clean up old resolved approvals (keep last 200)
    prune(): void {
        const all = [...this.approvals.entries()]
            .sort(([, a], [, b]) => b.requestedAt.localeCompare(a.requestedAt));
        const resolved = all.filter(([, a]) => a.status !== 'pending');
        for (const [id] of resolved.slice(200)) {
            this.approvals.delete(id);
        }
    }
}

export const approvalGate = new ApprovalGate();
