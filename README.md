# Agent IDE — Agent Workspace OS

> An enterprise-grade IDE shell for deploying, orchestrating, monitoring, and governing
> multi-agent AI workflows. Built on Eclipse Theia.

---

## What is Agent IDE?

Agent IDE is not a coding assistant. It is an **Agent Workspace OS** — a platform
where human operators define goals and AI agents decompose, execute, and deliver them.

Every agent action is governed, every run is traceable, every artifact is versioned.

---

## Panels

| Panel | Description |
|-------|-------------|
| **Dashboard** | Workspace overview — agent count, task status, run history, governance status |
| **Agents** | Browse and manage registered AI agents |
| **Tasks** | Create, assign, and track tasks |
| **Knowledge** | Workspace-level knowledge base for agent retrieval |
| **Artifacts** | Files, code, and data produced by agents |
| **Runs** | Live and historical execution log |
| **Replay** | Step-through trace debugger for any past run |
| **Governance** | Policy editor: allow / deny / require-approval / audit |
| **Builder** | Visual agent workflow designer (graph canvas) |

---

## Quick Start

```bash
# Prerequisites: Node.js 22, corepack enable
yarn install
yarn build
yarn start
# Open http://localhost:3000
```

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for Docker and CI setup.

---

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full monorepo layout
and design decisions.

See [docs/AGENT_WORKSPACE_OS.md](docs/AGENT_WORKSPACE_OS.md) for the product vision
and runtime roadmap.

---

## Monorepo Layout

```
agent-ide/
├── applications/browser-app/     # Theia browser application
├── extensions/agent-ide-core/    # Core IDE extension (panels, commands)
├── packages/agent-ide-types/     # Shared domain types
└── docs/                          # Architecture docs
```

---

## Contributing

See [AGENTS.md](AGENTS.md) for coding conventions and architecture constraints.

---

## License

MIT
