# Agent IDE

**Agent Workspace OS** — an open-source, browser-based IDE built on Eclipse Theia for building, running, and governing AI agents.

Live agent execution · Multi-model · MCP gateway · Semantic memory · Governance · Identity · Multi-agent orchestration · One-command deploy

---

## Deploy

```bash
# Docker
docker compose -f agent-compose.yml up -d

# k3s
./deploy.sh

# Full pipeline
make build push deploy
```

---

## Architecture

| Layer | Description |
|---|---|
| **Theia IDE** (port 3000) | Browser-based workbench — 16 agent panels, Monaco editor |
| **Backend** (port 3001) | Express + WebSocket — agent loop, tool proxy, governance, knowledge, auth |
| **Agent loop** | ReAct pattern: thought → tool → observation → result; streams every step over WebSocket |
| **MCP gateway** | stdio bridge to any MCP server |
| **Orchestrator** | Multi-agent graph: PM → execute (parallel) → review → deliver |
| **Knowledge store** | Semantic search — n-gram embeddings + OpenAI `text-embedding-3-small` |
| **Governance engine** | Policy evaluation + human-in-the-loop approval gate on every tool call |
| **Identity** | Users, agent identities, API keys, orgs, teams |

---

## Panels

| Panel | Location |
|---|---|
| Dashboard | Main (auto-opens) |
| Agent Builder | Main |
| Orchestrate | Main |
| Platform | Main |
| Research | Main |
| Bench | Main |
| Agents | Left sidebar |
| Tasks | Left sidebar |
| Knowledge | Left sidebar |
| Artifacts | Left sidebar |
| MCP / Tools | Left sidebar |
| Workspaces | Left sidebar |
| Identity | Left sidebar |
| Runs | Bottom |
| Replay | Bottom |
| Governance | Right sidebar |
| Optimize | Right sidebar |

---

## Tools

Built-in tools, each gated through the policy engine before execution:

| Tool | Description |
|---|---|
| `file_rw` | Read / write / list / delete workspace files |
| `shell` | Execute shell commands (`ALLOW_SHELL=true`) |
| `http_client` | Outbound HTTP requests |
| `browser` | Fetch URL, strip HTML, return text |
| `web_search` | Brave Search API |
| `vector_search` | Semantic search over knowledge store |
| `code_exec` | Run Python / JS in a sandbox |
| `db_query` | SQL query against a database |
| `openhands_task` | Delegate to OpenHandS agent |

---

## SDKs & Integrations

**LLM providers**
- Anthropic (Claude)
- OpenAI (GPT-4, o-series)
- Google Gemini
- Ollama / vLLM (local, via `OLLAMA_BASE_URL`)

**MCP servers** (stdio bridge, auto-configured)
- `@modelcontextprotocol/server-github`
- `@modelcontextprotocol/server-brave-search`
- `@modelcontextprotocol/server-postgres`
- `@modelcontextprotocol/server-puppeteer`
- `@modelcontextprotocol/server-filesystem`

**Agent integrations**
- OpenHandS (via `OPENHANDS_URL`)

---

## API

Backend REST + WebSocket on port 3001.

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | Uptime check |
| `GET` | `/api/config` | Feature flags |
| `POST` | `/api/runs` | Start agent run → `runId` |
| `GET` | `/api/runs` | List runs |
| `GET` | `/api/runs/:id` | Full trace |
| `WS` | `/ws/:runId` | Stream run steps |
| `POST` | `/api/orchestrate` | Start multi-agent orchestration |
| `GET` | `/api/orchestrate/:id` | Orchestration status |
| `*` | `/api/mcp/*` | MCP gateway |
| `*` | `/api/knowledge/*` | Knowledge store |
| `*` | `/api/governance/*` | Policies, audit, approvals |
| `*` | `/api/identity/*` | Users, agents, keys, orgs |
| `*` | `/api/workspaces/*` | Workspaces |
| `*` | `/api/auth/*` | Auth |

---

## Environment

```bash
# LLM keys
ANTHROPIC_API_KEY=
OPENAI_API_KEY=
BRAVE_API_KEY=

# Local LLM (Ollama / vLLM)
OLLAMA_BASE_URL=http://localhost:11434/v1

# Integrations
OPENHANDS_URL=http://localhost:3000

# Flags
ALLOW_SHELL=false
AUTH_ENABLED=false
```

---

## Run locally

```bash
yarn install
yarn start:backend   # backend only, no keys needed (demo mode)
yarn start:dev       # full IDE + backend
```

---

## Make

```bash
make build     # compile TypeScript + build container
make push      # push image to GHCR
make deploy    # deploy to k3s or docker compose
make all       # build → push → deploy
```
