import { RunRecord, RunStatus, TraceStepRecord } from './types';

class RunStore {
    private runs = new Map<string, RunRecord>();

    create(record: RunRecord): void {
        this.runs.set(record.runId, record);
    }

    get(runId: string): RunRecord | undefined {
        return this.runs.get(runId);
    }

    list(): RunRecord[] {
        return [...this.runs.values()].sort(
            (a, b) => new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime()
        );
    }

    appendStep(runId: string, step: TraceStepRecord): void {
        const run = this.runs.get(runId);
        if (!run) return;
        run.steps.push(step);
        run.totalInputTokens += step.inputTokens;
        run.totalOutputTokens += step.outputTokens;
    }

    setStatus(runId: string, status: RunStatus, extra?: Partial<RunRecord>): void {
        const run = this.runs.get(runId);
        if (!run) return;
        run.status = status;
        if (extra) Object.assign(run, extra);
        if (status === 'completed' || status === 'failed' || status === 'cancelled') {
            run.completedAt = new Date().toISOString();
        }
    }

    cancel(runId: string): boolean {
        const run = this.runs.get(runId);
        if (!run || run.status !== 'running') return false;
        this.setStatus(runId, 'cancelled');
        return true;
    }

    prune(maxAge = 3600000): void {
        const cutoff = Date.now() - maxAge;
        for (const [id, run] of this.runs) {
            if (new Date(run.startedAt).getTime() < cutoff) {
                this.runs.delete(id);
            }
        }
    }
}

export const runStore = new RunStore();
