// Semantic validation for the Autonomyx language.
// These rules run after parsing and produce structured diagnostics
// visible in any LSP-compatible editor (Theia, VS Code, Neovim, etc.)

import type { ValidationAcceptor, ValidationChecks } from 'langium';
import type { Agent, Workflow, Tool, Infra, AutonomyxAstType } from './generated/ast.js';

export function registerValidationChecks(registry: { register(checks: ValidationChecks<AutonomyxAstType>): void }): void {
    const checks: ValidationChecks<AutonomyxAstType> = {
        Agent:    [checkAgentHasModel, checkNoApiKeyInSource],
        Workflow: [checkWorkflowHasSteps, checkStepToolsResolved],
        Tool:     [checkToolHasDescription],
        Infra:    [checkInfraHasDomain],
    };
    registry.register(checks);
}

// Agents must declare a model (can be overridden at runtime via env var but
// must be explicit in source so intent is clear).
function checkAgentHasModel(agent: Agent, accept: ValidationAcceptor): void {
    if (!agent.model) {
        accept('warning', `Agent "${agent.name}" has no model declared. Set one or supply LLM_MODEL at runtime.`, {
            node: agent, property: 'name',
        });
    }
}

// Hard security rule: API keys must never be embedded in .ayx source files.
// They belong in env vars or a secrets manager.
function checkNoApiKeyInSource(agent: Agent, accept: ValidationAcceptor): void {
    if (agent.apiKey && agent.apiKey !== '$LLM_API_KEY') {
        accept('error',
            `Do not embed api_key values in source — supply chain risk. Use env vars: api_key: "$LLM_API_KEY"`,
            { node: agent, property: 'apiKey' });
    }
}

// Workflows without steps are stubs — warn so the developer notices.
function checkWorkflowHasSteps(wf: Workflow, accept: ValidationAcceptor): void {
    if (!wf.steps || wf.steps.length === 0) {
        accept('warning', `Workflow "${wf.name}" has no steps defined.`, {
            node: wf, property: 'name',
        });
    }
}

// Each step that references a tool must resolve (cross-reference check).
function checkStepToolsResolved(wf: Workflow, accept: ValidationAcceptor): void {
    for (const step of wf.steps ?? []) {
        if (step.tool && !step.tool.ref) {
            accept('error', `Step "${step.name}": tool "${step.tool.$refText}" is not defined.`, {
                node: step, property: 'tool',
            });
        }
    }
}

// Tools without descriptions can't be auto-selected by the reasoning engine.
function checkToolHasDescription(tool: Tool, accept: ValidationAcceptor): void {
    if (!tool.description) {
        accept('warning', `Tool "${tool.name}" has no description. The reasoning engine uses descriptions for tool selection.`, {
            node: tool, property: 'name',
        });
    }
}

// Infra blocks without a domain won't be reachable from peers.
function checkInfraHasDomain(infra: Infra, accept: ValidationAcceptor): void {
    if (!infra.domain) {
        accept('warning', `Infra "${infra.name}" has no domain configured. Peers won't be able to locate this instance.`, {
            node: infra, property: 'name',
        });
    }
}
