# Deployment Guide

## Backend server

The agent backend runs separately from Theia on **port 3001**.

### Start backend

```bash
# With live LLM (set at least one API key):
OPENAI_API_KEY=sk-…   yarn start:backend
ANTHROPIC_API_KEY=sk-… yarn start:backend

# Offline/demo mode (no API key needed — synthetic traces):
yarn start:backend

# With shell and filesystem tools enabled:
ALLOW_SHELL=true WORKSPACE_ROOT=/path/to/workspace yarn start:backend
```

### Environment variables

| Variable | Description |
|---|---|
| `PORT` | Backend port (default `3001`) |
| `OPENAI_API_KEY` | OpenAI API key for GPT-4o / GPT-4o-mini |
| `ANTHROPIC_API_KEY` | Anthropic API key for Claude models |
| `BRAVE_API_KEY` | Brave Search API key for `web_search` tool |
| `DATABASE_URL` | PostgreSQL URL for `db_query` tool |
| `ALLOW_SHELL` | Set `true` to enable `shell` and `code_exec` tools |
| `WORKSPACE_ROOT` | Root directory for `file_rw` tool (default: `cwd`) |

### API reference

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | Server health and uptime |
| `GET` | `/api/config` | Server config (which keys are set) |
| `POST` | `/api/runs` | Submit a new agent run |
| `GET` | `/api/runs` | List recent runs |
| `GET` | `/api/runs/:id` | Get run with full trace |
| `DELETE` | `/api/runs/:id` | Cancel a running run |
| `GET` | `/api/tools` | List available tools |
| `POST` | `/api/tools/:id/invoke` | Invoke a tool directly |
| `WS` | `/ws/:runId` | Stream trace steps for a run |

---

## Local development

### Prerequisites

- Node.js 22+
- Yarn (via corepack)

```bash
corepack enable
```

### First-time setup

```bash
# 1. Install all workspace dependencies
yarn install

# 2. Build in dependency order
yarn --cwd packages/agent-ide-types build
yarn --cwd extensions/agent-ide-core build

# 3. Build the browser app (long — downloads and webpack-bundles Theia)
yarn --cwd applications/browser-app build

# 4. Start the browser IDE
yarn --cwd applications/browser-app start
# Open http://localhost:3000
```

Or using the workspace root shortcut (after all packages are built once):

```bash
yarn start
```

### Watch mode (rebuild on file change)

```bash
# Terminal 1: types
yarn --cwd packages/agent-ide-types watch

# Terminal 2: extension
yarn --cwd extensions/agent-ide-core watch

# Terminal 3: start IDE (reads from compiled lib/ on each request)
yarn --cwd applications/browser-app start
```

### Typecheck without building

```bash
yarn --cwd packages/agent-ide-types typecheck
yarn --cwd extensions/agent-ide-core typecheck
```

### Lint

```bash
yarn --cwd extensions/agent-ide-core lint
```

---

## Docker

### Build image

```bash
docker build -t agennext/agent-ide:local .
```

### Run container

```bash
docker run --rm -p 3000:3000 agennext/agent-ide:local
# Open http://localhost:3000
```

### Environment variables

| Variable | Description | Default |
|---|---|---|
| `PORT` | Port for Theia backend | `3000` |
| `THEIA_DEFAULT_PLUGINS` | Plugin source (none in production) | — |

---

## CI (GitHub Actions)

The CI workflow at `.github/workflows/ci.yml` runs on every push and PR:

1. Checkout + setup Node 22
2. `corepack enable`
3. `yarn install --frozen-lockfile`
4. Build `packages/agent-ide-types`
5. Build `extensions/agent-ide-core`
6. Typecheck extension
7. Lint extension (non-blocking)

Browser app build is intentionally excluded from CI (takes 5–15 min; Theia downloads ~500 MB of packages).

---

## Troubleshooting

### `@theia/core` not found

Run `yarn install` from the root. The workspace symlinks are not set up until install runs.

### `emitDecoratorMetadata` errors

Ensure `tsconfig.base.json` has `"emitDecoratorMetadata": true` and `"experimentalDecorators": true`.

### Widget not appearing

1. Check the Widget is exported from its file.
2. Check both Widget and Contribution are imported in `frontend-module.ts`.
3. Check `bindPanel(bind, MyWidget, MyContribution)` is called.
4. Check `MyWidget.ID` matches the `widgetId` in the Contribution constructor.

### Panel opens blank

The `render()` method is returning `null` or throwing. Check the browser DevTools console for React errors.

### MCP server not connecting

MCP servers using `stdio` transport require a backend Node.js process to spawn the server subprocess. Browser-only sessions cannot use stdio MCP. WebSocket MCP servers can connect directly from the browser if CORS is configured.
