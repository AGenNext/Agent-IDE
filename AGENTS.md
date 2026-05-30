# AGENTS.md — Coding Agent Rules for Agent IDE

## Monorepo layout

```
packages/agent-ide-types/     pure TypeScript types, no Theia dependency
extensions/agent-ide-core/    Theia frontend extension — all panels, widgets, runtime
applications/browser-app/     Theia browser application shell
docs/                         Architecture, extension model, roadmap docs
```

## Build order

Always build in dependency order:
1. `packages/agent-ide-types` — no deps
2. `extensions/agent-ide-core` — depends on types
3. `applications/browser-app` — depends on extension

## Theia extension rules

- All widgets must `extend ReactWidget` and be `@injectable()`.
- Every widget needs a matching `AbstractViewContribution` subclass.
- Both must be exported from the same file.
- Both must be bound in `frontend-module.ts` via `bindPanel()`.
- Every command must be declared in `agent-ide-commands.ts` before use.
- `bindViewContribution` is the only correct way to register a Contribution class.
- Never use `new` to instantiate widgets — always use InversifyJS DI.

## TypeScript rules

- All new domain types go in `packages/agent-ide-types/src/index.ts`.
- No `any` in types package; liberal `any` is OK in extension UI code.
- `emitDecoratorMetadata: true` is required — never remove it from tsconfig.
- JSX pragma: React is imported as `import * as React from 'react'`.

## Style

- Dark-theme color palette: backgrounds `#0a0a0a`–`#1e1e1e`, accents `#7ab4ff` (blue), `#60d060` (green), `#d0a030` (amber), `#c080ff` (purple).
- All inline styles — no external CSS files or CSS-in-JS libraries.
- Simulation/demo data lives inside the widget file, not in a separate data file.

## Panels reference

| Widget ID                | Area    | Command                     |
|--------------------------|---------|----------------------------|
| `agent-ide:dashboard`    | main    | `agentIde.openDashboard`   |
| `agent-ide:agents`       | left    | `agentIde.openAgents`      |
| `agent-ide:tasks`        | left    | `agentIde.openTasks`       |
| `agent-ide:knowledge`    | left    | `agentIde.openKnowledge`   |
| `agent-ide:artifacts`    | left    | `agentIde.openArtifacts`   |
| `agent-ide:runs`         | bottom  | `agentIde.openRuns`        |
| `agent-ide:replay`       | bottom  | `agentIde.openReplay`      |
| `agent-ide:governance`   | right   | `agentIde.openGovernance`  |
| `agent-ide:optimize`     | right   | `agentIde.openOptimize`    |
| `agent-ide:builder`      | main    | `agentIde.openBuilder`     |
| `agent-ide:platform`     | main    | `agentIde.openPlatform`    |
| `agent-ide:research`     | main    | `agentIde.openResearch`    |
| `agent-ide:bench`        | main    | `agentIde.openBench`       |
| `agent-ide:mcp`          | left    | `agentIde.openMcp`         |
