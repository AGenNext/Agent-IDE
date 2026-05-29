# Architecture — Agent Workspace OS

## Overview

Agent IDE is an **Agent Workspace OS** built on [Eclipse Theia](https://theia-ide.org/).
It is not a chatbot IDE. It is a production-grade platform for deploying, orchestrating,
monitoring, and governing multi-agent AI workflows inside a full-featured IDE shell.

## Monorepo Structure

```
agent-ide/
├── applications/
│   └── browser-app/         # Theia browser application (port 3000)
├── extensions/
│   └── agent-ide-core/      # Core Theia extension: panels, commands, dashboard
├── packages/
│   └── agent-ide-types/     # Shared TypeScript domain types (no Theia dependency)
├── docs/                    # Architecture and design documentation
├── tsconfig.base.json       # Shared TypeScript compiler config
├── .eslintrc.json           # ESLint config
├── .prettierrc.json         # Prettier config
└── AGENTS.md                # Rules for coding agents
```

## Build Topology

Dependency order (topological build):

```
agent-ide-types  →  agent-ide-core  →  browser-app
```

- `agent-ide-types`: compiled by `tsc`, no Theia dependency
- `agent-ide-core`: compiled by `tsc`, produces `lib/browser/` CJS modules
- `browser-app`: bundled by `@theia/cli` webpack, consumes all extensions

## Key Abstractions

### Domain Types (`packages/agent-ide-types`)

Pure TypeScript. No runtime dependencies. Defines the workspace object model:

| Type | Purpose |
|------|--------|
| `Agent` | An AI agent with skills and tools |
| `Skill` | A named capability an agent can invoke |
| `Tool` | An external function/API an agent can call |
| `Task` | A unit of work assigned to an agent |
| `Artifact` | An output produced by an agent during task execution |
| `AgentRun` | A single execution of an agent on a task |
| `TraceStep` | One step in an agent run trace (thought/action/observation) |
| `GovernancePolicy` | A policy constraining agent behavior |
| `GovernanceRule` | One rule within a policy |
| `WorkspaceGraph` | A graph of agents, tasks, artifacts, and their relationships |

### Theia Extension (`extensions/agent-ide-core`)

Implements the IDE shell panels as Theia `ReactWidget` subclasses, each registered
via `AbstractViewContribution`. See `THEIA_EXTENSION_MODEL.md` for the binding pattern.

### Panels

| Panel | Area | Purpose |
|-------|------|---------|
| Dashboard | main | Workspace overview and quick links |
| Agents | left | Agent manifest browser |
| Tasks | left | Task list and decomposition |
| Knowledge | left | Workspace knowledge base |
| Artifacts | left | Agent-produced outputs |
| Runs | bottom | Execution history |
| Replay | bottom | Step-through run debugger |
| Governance | right | Policy editor |
| Builder | main | Visual agent workflow designer |

## Runtime Architecture (Planned)

See `AGENT_WORKSPACE_OS.md` for the full runtime vision and phase plan.

```
[Browser IDE Shell]
    └── Theia Frontend (React widgets, WebSocket)
           │
           ↓ WebSocket / REST
           │
    [Theia Backend (Node.js)]
        ├── Agent Runtime (Phase 2: LangGraph or custom)
        ├── MCP Gateway (Phase 3)
        ├── Knowledge Store (Phase 5)
        ├── Governance Engine (Phase 4)
        └── Auth / Identity (Phase 4)
```
