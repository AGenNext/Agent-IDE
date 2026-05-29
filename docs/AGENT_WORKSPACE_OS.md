# Agent Workspace OS — Vision and Runtime Design

## What is Agent Workspace OS?

Agent Workspace OS is the product category this IDE represents.
It is an **operating system for AI agent teams** — a platform where:

- Human operators define goals, agents decompose and execute them
- Multiple specialized agents collaborate with shared knowledge and tools
- Governance policies constrain what agents can do without approval
- Every action is auditable via trace replay
- The workspace graph shows who produced what, how, and when

This is **not** a coding assistant. It is an agent orchestration and
governance platform that happens to run inside an IDE shell (Theia) to
give operators a familiar, extensible, offline-capable interface.

## Design Principles

1. **Operator-first**: The human is in control. Agents request, operators approve.
2. **Auditable by default**: Every trace step is stored; nothing is lost.
3. **Governance-native**: Policies are first-class citizens, not afterthoughts.
4. **Extensible runtime**: The agent executor is pluggable (LangGraph, custom, etc.).
5. **Offline-capable**: The IDE shell runs locally; cloud sync is optional.

## Phase Roadmap

| Phase | Capability | Status |
|-------|-----------|--------|
| 1 | Theia IDE shell, panels, types, governance UI stubs | **Done** |
| 2 | Agent runtime (LangGraph), task execution, live trace | Planned |
| 3 | MCP gateway, tool registry, tool approval flow | Planned |
| 4 | Auth, multi-user, role-based governance | Planned |
| 5 | Knowledge store, vector search, artifact promotion | Planned |
| 6 | XYFlow builder, workspace graph visualization | Planned |
| 7 | Cloud sync, remote workspaces, team collaboration | Planned |

## Workspace Object Model

```
Workspace
 ├── Agents[]          each has Skills[] and Tools[]
 ├── Tasks[]           each assigned to an Agent
 ├── Runs[]            each linked to a Task and Agent
 │    └── TraceSteps[]  ordered log of the run
 ├── Artifacts[]       each produced by a Run
 ├── Knowledge[]       shared retrieval store
 └── GovernancePolicies[] applied at workspace/agent/tool scope
```

## Governance Model

Governance policies evaluate every agent action before execution:

```
AgentAction  →  GovernanceEngine.evaluate(action, policies)
                     ├── allow          →  execute immediately
                     ├── audit_only     →  execute + log
                     ├── require_approval →  pause, notify operator
                     └── deny           →  block, log reason
```

All governance decisions are stored as audit events in the run trace.
