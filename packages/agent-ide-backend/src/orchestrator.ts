/**
 * Multi-agent graph orchestrator.
 * Graph: start → plan → execute (parallel sub-agents) → review → deliver
 *
 * PM agent decomposes a goal into tasks.
 * Sub-agents execute individual tasks via the standard agent loop.
 * Reviewer agent evaluates all outputs; may trigger one revision round.
 */

import { v4 as uuidv4 } from 'uuid';
import { callLLM } from './agent-loop';
import { startAgentRun } from './agent-loop';
import { runStore } from './run-store';
import { broadcast } from './websocket';
import type { OAIMessage } from './types';

export type GraphNode = 'start' | 'plan' | 'execute' | 'review' | 'deliver' | 'done' | 'error';

export interface OrchestrationTask {
    id:          string;
    title:       string;
    description: string;
    agentRole:   string;
    model:       string;
    tools:       string[];
    status:      'pending' | 'running' | 'done' | 'failed';
    runId?:      string;
    output?:     string;
}

export interface OrchestrationRun {
    id:           string;
    goal:         string;
    model:        string;
    status:       'running' | 'completed' | 'failed';
    node:         GraphNode;
    tasks:        OrchestrationTask[];
    plan?:        string;
    review?:      string;
    result?:      string;
    startedAt:    string;
    completedAt?: string;
}

const runs = new Map<string, OrchestrationRun>();

export function getOrchestrationRun(id: string): OrchestrationRun | undefined { return runs.get(id); }
export function listOrchestrationRuns(): OrchestrationRun[] {
    return [...runs.values()].sort((a, b) => b.startedAt.localeCompare(a.startedAt));
}

// ─── Simplified LLM call (text only, no tools) ────────────────────────────────

async function llmText(model: string, apiKey: string, system: string, user: string): Promise<string> {
    if (!apiKey) return `[demo mode — no API key] ${user.slice(0, 120)}`;
    try {
        const msgs: OAIMessage[] = [
            { role: 'system', content: system },
            { role: 'user',   content: user },
        ];
        const resp = await callLLM(model, apiKey, msgs, [], 0.3, 2048);
        return resp.choices[0]?.message.content ?? '';
    } catch (err: unknown) {
        return `[LLM error: ${err instanceof Error ? err.message : String(err)}]`;
    }
}

// ─── PM agent — decomposes a goal into tasks ─────────────────────────────────

const PM_SYSTEM = `You are a Project Manager AI. Given a high-level goal, decompose it into 2–4 concrete tasks.

Return ONLY valid JSON with this exact shape (no markdown fences):
{
  "plan": "Brief plan description (1-2 sentences)",
  "tasks": [
    {
      "title": "Short task title",
      "description": "What the agent should do in detail",
      "agentRole": "researcher|coder|analyst|writer",
      "tools": ["web_search","http_client"]
    }
  ]
}

Available tools: web_search, http_client, file_rw, vector_search, code_exec, shell, browser, db_query.
Keep each task focused and independently executable.`;

interface PmPlan {
    plan:  string;
    tasks: Array<{ title: string; description: string; agentRole: string; tools: string[] }>;
}

async function planGoal(goal: string, model: string, apiKey: string): Promise<PmPlan> {
    const raw = await llmText(model, apiKey, PM_SYSTEM, `Goal: ${goal}`);
    try {
        // strip markdown code fences if present
        const json = raw.replace(/^```(?:json)?\n?/m, '').replace(/\n?```$/m, '').trim();
        return JSON.parse(json) as PmPlan;
    } catch {
        return {
            plan: `Execute: ${goal}`,
            tasks: [{ title: goal.slice(0, 60), description: goal, agentRole: 'analyst', tools: ['vector_search'] }],
        };
    }
}

// ─── Reviewer agent ───────────────────────────────────────────────────────────

const REVIEWER_SYSTEM = `You are a Quality Reviewer AI. Given a goal and the outputs from a set of sub-agent tasks, evaluate whether the work is complete and correct.

Return ONLY valid JSON:
{
  "verdict": "APPROVED" | "REVISION_NEEDED",
  "summary": "1-2 sentence evaluation",
  "failedTaskIds": [],
  "feedback": "Specific feedback for revision (empty if approved)"
}`;

interface ReviewVerdict {
    verdict:       'APPROVED' | 'REVISION_NEEDED';
    summary:       string;
    failedTaskIds: string[];
    feedback:      string;
}

async function reviewOutputs(goal: string, tasks: OrchestrationTask[], model: string, apiKey: string): Promise<ReviewVerdict> {
    const taskSummary = tasks.map(t =>
        `Task [${t.id}] "${t.title}": ${t.status === 'done' ? t.output?.slice(0, 400) ?? '(no output)' : `FAILED`}`
    ).join('\n\n');

    const raw = await llmText(model, apiKey, REVIEWER_SYSTEM,
        `Goal: ${goal}\n\nTask outputs:\n${taskSummary}`
    );
    try {
        const json = raw.replace(/^```(?:json)?\n?/m, '').replace(/\n?```$/m, '').trim();
        return JSON.parse(json) as ReviewVerdict;
    } catch {
        const allDone = tasks.every(t => t.status === 'done');
        return { verdict: allDone ? 'APPROVED' : 'REVISION_NEEDED', summary: '', failedTaskIds: [], feedback: '' };
    }
}

// ─── Execute tasks in parallel ────────────────────────────────────────────────

async function executeTasks(orch: OrchestrationRun, apiKey: string): Promise<void> {
    const pending = orch.tasks.filter(t => t.status === 'pending');

    await Promise.all(pending.map(async (task) => {
        task.status = 'running';
        try {
            const runId = await startAgentRun({
                agentId:      task.id,
                agentName:    `${task.agentRole} (${task.title})`,
                model:        task.model,
                systemPrompt: `You are a ${task.agentRole} agent. Complete the assigned task concisely and produce a clear final result.`,
                task:         task.description,
                tools:        task.tools,
                apiKey,
                maxIterations: 6,
                temperature:   0.4,
                maxTokens:     2048,
            });
            task.runId = runId;

            // Poll until the sub-run finishes
            await new Promise<void>((resolve) => {
                const check = setInterval(() => {
                    const r = runStore.get(runId);
                    if (!r || r.status === 'running') return;
                    clearInterval(check);
                    if (r.status === 'completed') {
                        const resultStep = r.steps.findLast(s => s.type === 'result');
                        task.output = resultStep?.content ?? JSON.stringify(r.steps.slice(-1)[0]?.content);
                        task.status = 'done';
                    } else {
                        task.status = 'failed';
                        task.output = r.errorMessage ?? 'Run failed';
                    }
                    resolve();
                }, 500);
                // Safety timeout: 5 min
                setTimeout(() => { clearInterval(check); task.status = 'failed'; resolve(); }, 300_000);
            });
        } catch (err: unknown) {
            task.status = 'failed';
            task.output = err instanceof Error ? err.message : String(err);
        }
    }));
}

// ─── Deliver — synthesize final result ───────────────────────────────────────

const DELIVER_SYSTEM = `You are a synthesis AI. Given a goal and the outputs from completed tasks, produce a clear, well-structured final answer that directly addresses the goal.`;

async function synthesizeResult(goal: string, tasks: OrchestrationTask[], review: string, model: string, apiKey: string): Promise<string> {
    const taskSummary = tasks.map(t => `### ${t.title}\n${t.output ?? '(no output)'}`).join('\n\n');
    return llmText(model, apiKey, DELIVER_SYSTEM,
        `Goal: ${goal}\n\nReview: ${review}\n\nTask outputs:\n${taskSummary}\n\nSynthesize a final answer:`
    );
}

// ─── Main orchestration graph ─────────────────────────────────────────────────

export async function startOrchestration(goal: string, model: string, apiKey: string, defaultTools: string[] = []): Promise<string> {
    const id = uuidv4();
    const orch: OrchestrationRun = {
        id, goal, model,
        status:    'running',
        node:      'start',
        tasks:     [],
        startedAt: new Date().toISOString(),
    };
    runs.set(id, orch);
    broadcast({ type: 'run:started', runId: id, payload: { orchestrationId: id, goal, node: 'start' } });

    (async () => {
        try {
            // ── plan ──────────────────────────────────────────────────────────
            orch.node = 'plan';
            const pmPlan = await planGoal(goal, model, apiKey);
            orch.plan = pmPlan.plan;
            orch.tasks = pmPlan.tasks.map(t => ({
                id:          uuidv4(),
                title:       t.title,
                description: t.description,
                agentRole:   t.agentRole,
                model,
                tools:       t.tools.length > 0 ? t.tools : defaultTools,
                status:      'pending',
            }));
            broadcast({ type: 'run:step', runId: id, payload: { node: 'plan', plan: orch.plan, taskCount: orch.tasks.length } });

            // ── execute ───────────────────────────────────────────────────────
            orch.node = 'execute';
            await executeTasks(orch, apiKey);
            broadcast({ type: 'run:step', runId: id, payload: { node: 'execute', tasks: orch.tasks.map(t => ({ id: t.id, title: t.title, status: t.status })) } });

            // ── review ────────────────────────────────────────────────────────
            orch.node = 'review';
            const verdict = await reviewOutputs(goal, orch.tasks, model, apiKey);
            orch.review = `${verdict.verdict}: ${verdict.summary}`;
            broadcast({ type: 'run:step', runId: id, payload: { node: 'review', verdict: verdict.verdict, summary: verdict.summary } });

            // One revision round
            if (verdict.verdict === 'REVISION_NEEDED' && verdict.failedTaskIds.length > 0) {
                for (const tid of verdict.failedTaskIds) {
                    const t = orch.tasks.find(x => x.id === tid);
                    if (t) { t.status = 'pending'; t.output = undefined; }
                }
                const toRevise = orch.tasks.filter(t => verdict.failedTaskIds.includes(t.id));
                if (toRevise.length > 0) {
                    await executeTasks(orch, apiKey);
                }
            }

            // ── deliver ───────────────────────────────────────────────────────
            orch.node = 'deliver';
            orch.result = await synthesizeResult(goal, orch.tasks, verdict.summary, model, apiKey);
            orch.node = 'done';
            orch.status = 'completed';
            orch.completedAt = new Date().toISOString();
            broadcast({ type: 'run:completed', runId: id, payload: { orchestrationId: id, result: orch.result } });

        } catch (err: unknown) {
            orch.node = 'error';
            orch.status = 'failed';
            orch.completedAt = new Date().toISOString();
            broadcast({ type: 'run:failed', runId: id, payload: { orchestrationId: id, error: err instanceof Error ? err.message : String(err) } });
        }
    })();

    return id;
}
