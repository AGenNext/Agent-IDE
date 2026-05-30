# Agent IDE

Agent IDE is an AGenNext developer workspace built on Eclipse Theia. It is designed for agent-first software work: live agent execution, multi-model support, governed tool use, semantic memory, MCP server integration, and enterprise identity management.

## What's inside

| Layer | What it does |
|---|---|
| **Theia browser IDE** (port 3000) | VS Code-like workbench, Monaco editor, 16 agent panels in the left sidebar |
| **Express + WebSocket backend** (port 3001) | Live agent loop, tool proxy, governance, knowledge store, auth |
| **Agent loop** | ReAct pattern — thought → tool call → observation → result; streams every step over WebSocket |
| **Tool proxy** | 9 built-in tools with policy + approval gate before every execution |
| **MCP gateway** | stdio bridge to any MCP server; GitHub, Brave Search, Postgres, Puppeteer, Filesystem pre-configured |
| **Knowledge store** | n-gram hash embeddings (no deps) + OpenAI `text-embedding-3-small`; cosine similarity search; `.knowledge.json` persistence |
| **Governance engine** | Tenant-scoped policies, audit log, human-in-the-loop approval gate with 5-min timeout |
| **Auth & Workspaces** | HMAC-SHA256 JWT, multi-tenant workspaces, demo password login |

## Panels (16)

Dashboard · Agents · Tasks · Knowledge · Artifacts · Runs · Replay · Governance · Agent Builder · Platform · Research · Bench · Optimize · MCP/Tools · Workspaces · *(Identity — coming)*

## Built-in tools

| Tool | Description |
|---|---|
| `file_rw` | Read / write / list / delete workspace files |
| `shell` | Execute shell commands (`ALLOW_SHELL=true` required) |
| `http_client` | Outbound HTTP requests |
| `browser` | Fetch a URL, strip HTML, return text |
| `web_search` | Brave Search API (`BRAVE_API_KEY` required) |
| `vector_search` | Semantic search over the knowledge store |
| `code_exec` | Execute Python or JavaScript (`ALLOW_SHELL=true` required) |
| `db_query` | SQL queries (`DATABASE_URL` required) |
| `repo_index` | Walk a local repo, chunk source files, ingest into knowledge store for cheap semantic code search |

All tool calls are evaluated against tenant policies before execution. `shell`, `code_exec`, and `db_query` require human approval by default (built-in Tool Safety policy).

## MCP servers

Configured in `.mcp.json` (auto-created on first start). Pre-configured servers:

| Server | Command |
|---|---|
| `filesystem` | `npx -y @modelcontextprotocol/server-filesystem .` |
| `brave-search` | `npx -y @modelcontextprotocol/server-brave-search` |
| `postgres` | `npx -y @modelcontextprotocol/server-postgres $DATABASE_URL` |
| `puppeteer` | `npx -y @modelcontextprotocol/server-puppeteer` |
| `github` | `npx -y @modelcontextprotocol/server-github` (`GITHUB_TOKEN` required) |

Connected MCP tools are automatically surfaced to the agent loop, namespaced as `mcp__<serverId>__<toolName>`.

## Supported models

Any OpenAI-compatible endpoint. Tested with:

- **Anthropic** — `claude-opus-4-8`, `claude-sonnet-4-6`, `claude-haiku-4-5`
- **OpenAI** — `gpt-4o`, `gpt-4o-mini`
- **Google** — `gemini-1.5-pro` (via OpenAI-compatible endpoint)

Set `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`. If neither is set the backend runs in **offline/demo mode** (synthetic traces, no API calls).

## Quick start

```bash
corepack enable
yarn install
yarn build
yarn start          # Theia IDE on port 3000
```

Start the backend separately:

```bash
cd packages/agent-ide-backend
npx ts-node src/server.ts    # backend on port 3001
```

### Environment variables

```env
# LLM
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...

# Tools
ALLOW_SHELL=true
BRAVE_API_KEY=...
DATABASE_URL=postgres://...

# MCP
GITHUB_TOKEN=ghp_...

# Auth
AUTH_ENABLED=true
JWT_SECRET=change-in-production
DEMO_EMAIL=you@example.com
DEMO_PASSWORD=yourpassword

# Paths
WORKSPACE_ROOT=/path/to/workspace
PORT=3001
```

## Docker

```bash
docker build -t agennext/agent-ide:local .
docker run --rm -p 3000:3000 -p 3001:3001 \
  -e OPENAI_API_KEY=sk-... \
  -e ALLOW_SHELL=true \
  agennext/agent-ide:local
```

## Repository layout

```
.
├── applications/browser-app/          # Theia browser application
├── extensions/agent-ide-core/         # Frontend extension (16 panels, backend client)
│   └── src/browser/
│       ├── panels/                    # All panel widgets
│       └── runtime/                   # backend-client.ts, session-store.ts
├── packages/
│   ├── agent-ide-backend/             # Express + WebSocket server
│   │   └── src/
│   │       ├── server.ts              # All REST routes
│   │       ├── agent-loop.ts          # ReAct loop, LLM calls, MCP routing
│   │       ├── tool-proxy.ts          # 9 tools + policy/approval gate
│   │       ├── mcp-manager.ts         # stdio MCP bridge
│   │       ├── knowledge-store.ts     # Vector store + chunking + embeddings
│   │       ├── governance/            # policy-engine, audit-log, approval-gate
│   │       ├── auth.ts                # JWT, requireAuth middleware
│   │       └── workspace-manager.ts   # Multi-tenant workspace CRUD
│   └── agent-ide-types/               # Shared TypeScript types
├── docs/ROADMAP.md                    # Phase-by-phase progress
├── Dockerfile
└── package.json                       # Yarn workspace root
```

## REST API (port 3001)

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | Server health + uptime |
| `GET` | `/api/config` | Feature flags and counts |
| `POST` | `/api/runs` | Submit an agent task |
| `GET` | `/api/runs` | List recent runs |
| `GET` | `/api/runs/:id` | Full run trace |
| `DELETE` | `/api/runs/:id` | Cancel a running run |
| `GET` | `/api/tools` | List available tools |
| `POST` | `/api/tools/:id/invoke` | Invoke a tool directly |
| `GET` | `/api/mcp/servers` | List MCP servers with status |
| `POST` | `/api/mcp/servers/:id/connect` | Connect an MCP server |
| `GET` | `/api/mcp/tools` | All tools from connected servers |
| `POST` | `/api/auth/login` | Password login → JWT |
| `GET` | `/api/auth/me` | Current user |
| `GET` | `/api/workspaces` | List workspaces |
| `GET` | `/api/knowledge` | List knowledge chunks |
| `POST` | `/api/knowledge/search` | Semantic search |
| `POST` | `/api/knowledge/ingest` | Ingest text or URL |
| `GET` | `/api/governance/policies` | List policies |
| `GET` | `/api/governance/audit` | Audit log |
| `GET` | `/api/governance/approvals` | Pending approvals |
| `POST` | `/api/governance/approvals/:id/approve` | Approve a tool call |
| `POST` | `/api/governance/approvals/:id/reject` | Reject a tool call |

WebSocket: `ws://localhost:3001/ws/:runId` — streams `run:started`, `run:step`, `run:completed`, `run:failed`, `governance:approval-request`, `governance:approval-resolved`.

## Roadmap

See [docs/ROADMAP.md](docs/ROADMAP.md) for phase-by-phase progress.

**Up next:**
- Identity & lifecycle management (users, agent identities, orgs, teams, sessions, MFA, API keys)
- LangGraph-style multi-agent orchestration with reviewer and project manager agents
- Open LLM support (Ollama, vLLM, OpenRouter)
- Replay panel reading real persisted traces
- WebSocket fan-out to multiple subscribers
