# AGENTS.md — Agent Workspace OS: Production Rules

This file documents the rules, constraints, and conventions for coding agents (AI or human)
working in this repository.

## Architecture Constraints

### Runtime Boundaries (Phase 1)

The following capabilities are **intentionally deferred** and must NOT be implemented
until their dedicated phases:

| Capability | Deferred Phase | Extension Point |
|---|---|---|
| LangGraph / agent runtime | Phase 2 | `packages/agent-ide-types` → `AgentRun`, `TraceStep` |
| MCP gateway | Phase 3 | `Tool.mcpServerId` field in types |
| Authentication / identity | Phase 4 | TODO in `frontend-module.ts` |
| Persistent database | Phase 5 | Service interfaces in `packages/agent-ide-types` |
| XYFlow graph editor | Phase 2 | `agent-builder/agent-builder-widget.tsx` canvas |

### Theia Extension Rules

1. **Inversify imports**: Always use `@theia/core/shared/inversify`, never bare `inversify`.
   This avoids container identity mismatches at runtime.

2. **React imports**: Always `import * as React from 'react'` in `.tsx` files.

3. **Widget IDs**: Format `agent-ide:<kebab-case-name>` (e.g. `agent-ide:dashboard`).

4. **Command IDs**: Format `agentIde.<camelCase>` (e.g. `agentIde.openDashboard`).

5. **Command constants**: Define all commands in
   `extensions/agent-ide-core/src/browser/agent-ide-commands.ts`.
   Do NOT scatter command definitions across widget files.

6. **Widget registration**: All widgets/contributions must be registered in
   `extensions/agent-ide-core/src/browser/frontend-module.ts`.

7. **theiaExtensions**: The `package.json` of every Theia extension package must include
   a `theiaExtensions` array pointing to the compiled frontend module.

## Code Style

- TypeScript: `strict: false`, `strictNullChecks: true`, `experimentalDecorators: true`
- Formatting: Prettier config in `.prettierrc.json` (100 char width, single quotes, 4-space indent)
- Linting: ESLint with `@typescript-eslint` plugin
- Comments: only when the WHY is non-obvious; no narrating WHAT the code does

## Extension Points

When extending this codebase, search for `// TODO:` comments — these mark planned
extension points that should not be filled in prematurely.

Key entry files:
- `extensions/agent-ide-core/src/browser/frontend-module.ts` — bind new widgets/services
- `extensions/agent-ide-core/src/browser/agent-ide-commands.ts` — add new command constants
- `packages/agent-ide-types/src/index.ts` — add new domain types
- `applications/browser-app/package.json` — add new Theia extension packages

## Commit Message Format

```
<type>(<scope>): <subject>

<body (optional)>
```

Types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`, `ci`
Scopes: `core`, `types`, `browser-app`, `docs`, `ci`, `builder`

## Testing

- Unit tests: `extensions/*/src/__tests__/` (Jest — add config when writing first test)
- Integration tests: `e2e/` (Playwright — not yet configured)
- Manual smoke test: `yarn start` → verify dashboard opens on port 3000
