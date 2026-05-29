# Agent IDE

Agent IDE is an AGenNext developer workspace built by extending Eclipse Theia. It is designed for agent-first software work: coding, prompt and tool development, task handoff, trace review, governed execution, and enterprise collaboration.

This repository starts with a production-oriented Theia application shell and an Agent IDE extension surface that can grow into a full agent workspace.

## Why Theia

Eclipse Theia gives Agent IDE a VS Code-like workbench, Monaco editor, language server support, terminal integration, extension compatibility patterns, and a browser/desktop-friendly architecture. Agent IDE extends that foundation instead of reinventing the IDE shell.

## Product goals

- Agent-aware workspace for code, tasks, files, prompts, tools, skills, logs, and human review.
- Theia workbench as the stable IDE substrate.
- Agent panels for context, runs, traces, handoffs, memory, skills, and governance.
- Workspace-first collaboration with Git, terminal, editor, preview, and project graph support.
- Governance-native execution: policies, approvals, audit logs, replay, retry, and provenance.
- Deployable as a browser IDE with Docker and Kubernetes-friendly runtime assumptions.

## Repository layout

```text
.
├── applications/browser-app/      # Theia browser application package
├── extensions/agent-ide-core/     # Agent IDE frontend extension and contribution points
├── docs/                          # Architecture and implementation notes
├── .github/workflows/             # CI checks
├── Dockerfile                     # Container image for browser IDE
├── package.json                   # Yarn workspace root
└── theia-apps.json                # Theia app metadata
```

## First implementation scope

The first bootstrap provides:

- A Theia browser application package.
- A custom `agent-ide-core` extension package.
- Agent dashboard command and widget contribution.
- Agent IDE menu/command registration.
- Workspace scripts for build, start, lint, and test.
- Docker image for deployable browser IDE.
- CI workflow for dependency install and build validation.
- Product architecture notes for future agent runtime integrations.

## Local development

```bash
corepack enable
yarn install
yarn build
yarn start
```

The browser app defaults to port `3000`.

## Docker

```bash
docker build -t agennext/agent-ide:local .
docker run --rm -p 3000:3000 agennext/agent-ide:local
```

## Extension model

The custom extension starts small on purpose. It should become the stable integration layer for:

- Agent registry and skill registry views.
- Tool and MCP connection management.
- Agent run console.
- Trace, replay, retry, and handoff inspection.
- Governance policies and approval gates.
- Prompt, workflow, and blueprint authoring.
- Workspace graph and artifact provenance.

## Roadmap

1. Add backend agent runtime bridge for LangGraph/AgentRunner.
2. Add authenticated workspace API and per-user workspace provisioning.
3. Add trace viewer and handoff review UI.
4. Add tool registry/MCP gateway panel.
5. Add policy checks with OPA/OpenFGA-ready abstractions.
6. Add collaborative artifact views and project graph visualization.
7. Add desktop packaging if needed after browser IDE stabilizes.
