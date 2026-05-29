# Agent IDE Roadmap

## Phase 1 — Theia Foundation ✅

- [x] Yarn workspaces monorepo (`packages/`, `extensions/`, `applications/`)
- [x] `packages/agent-ide-types` — full domain type library
- [x] `extensions/agent-ide-core` — Theia frontend extension
- [x] `applications/browser-app` — Theia browser app shell (port 3000)
- [x] InversifyJS DI wiring, command/menu/widget registration
- [x] CI workflow (Node 22, typecheck, lint)
- [x] Dockerfile (multi-stage, Node 22, corepack)

## Phase 2 — Agent Workspace OS ✅

- [x] Agent Dashboard (stat cards, activity log, panel grid)
- [x] Agents panel (5-section form: Overview/Model/Prompt/Tools/Skills/Sub-agents)
- [x] Tasks panel (list, filters, inline add, detail drawer)
- [x] Knowledge panel (browse / semantic search / ingest)
- [x] Artifacts panel (12 artifact types, type-filter sidebar, content viewer)
- [x] Runs panel (Trace / Token Flow / Performance tabs with AgentBench)
- [x] Replay panel (step-through trace replay, play/pause/scrub)
- [x] Governance panel (Policies / Audit Log / Approval Queue)
- [x] Agent Builder (SVG drag-and-drop workflow graph)
- [x] Platform panel (FinOps / Traces / Monitor / Sources)
- [x] Research panel (multi-source research orchestration)
- [x] Bench panel (AgentBench evaluation harness)
- [x] Optimize panel (prompt, cost, latency tuning)
- [x] MCP/Tools panel (tool registry, MCP servers, invocation logs)
- [x] Runtime stubs (agent-runtime, orchestrator, tool-registry, message-bus, persistence)

## Phase 3 — Live Agent Execution ✅ (initial)

- [x] `packages/agent-ide-backend` — Express + WebSocket server (port 3001)
- [x] POST `/api/runs` — submit agent task, returns runId immediately
- [x] WebSocket `/ws/:runId` — real-time trace step streaming
- [x] GET `/api/runs`, GET `/api/runs/:id`, DELETE `/api/runs/:id`
- [x] POST `/api/tools/:id/invoke` — direct tool invocation endpoint
- [x] GET `/api/tools`, GET `/api/config`, GET `/health`
- [x] Live agent loop: ReAct pattern with real OpenAI/Anthropic API calls
- [x] Offline/demo mode — synthetic trace when no API key configured
- [x] Tool proxy: `file_rw`, `shell`, `http_client`, `browser`, `web_search`, `vector_search`, `code_exec`, `db_query`
- [x] `backend-client.ts` — frontend HTTP+WebSocket client for backend
- [x] Runs panel — "⚡ Live Run" button when backend detected, streams trace via WebSocket
- [ ] Tool execution approval flow (Governance → hold tool call until approved)
- [ ] Replay panel reading real persisted traces from persistence layer
- [ ] WebSocket fan-out to multiple subscribers

## Phase 4 — MCP Gateway

- [ ] Real MCP server connections (stdio via backend bridge)
- [ ] MCP panel: live server connect/disconnect with status polling
- [ ] Tool call routing through governance approval gate
- [ ] MCP server configuration persistence (`.mcp.json` in workspace)
- [ ] Tool schema discovery from live MCP servers

## Phase 5 — Authentication & Workspaces ✅ (initial)

- [x] JWT auth middleware (`auth.ts`) — HMAC-SHA256 tokens, demo password login, `AUTH_ENABLED` flag
- [x] `requireAuth` middleware attached to all routes — threads `req.user` (tenantId) through every handler
- [x] `workspace-manager.ts` — tenant-scoped workspace CRUD with `.workspaces.json` persistence
- [x] Workspace REST API: create, list, get, rename (PATCH), delete, activate (switch active workspace)
- [x] `/api/auth/me`, `/api/auth/login`, `/api/auth/logout` endpoints
- [x] `session-store.ts` — localStorage session persistence (token + userId + activeWorkspaceId)
- [x] `backend-client.ts` — auth and workspace API methods (login, getMe, CRUD, activate)
- [x] **Workspaces panel** (15th panel) — Account tab (avatar, user info, sign in/out) + Workspaces tab (list, create, rename, activate, delete)
- [ ] Clerk/Auth0 integration (set CLERK_PUBLISHABLE_KEY or AUTH0_DOMAIN to enable)
- [ ] Per-user workspace provisioning via isolated Docker containers
- [ ] Tenant-scoped MCP server configs (per-workspace `.mcp.json`)

## Phase 6 — Knowledge & Memory ✅ (initial)

- [x] `knowledge-store.ts` — in-memory vector store with cosine similarity search; 128-dim n-gram hash embedding (no deps) + OpenAI `text-embedding-3-small` when key is set; `.knowledge.json` persistence
- [x] Knowledge REST API: list, search (POST with query + topK), ingest text, ingest URL, get, delete
- [x] Knowledge ingest pipeline: `chunkText()` splits at word boundaries with overlap; `ingestText()` / `ingestUrl()` (fetches, strips HTML, chunks, embeds)
- [x] Agent memory: completed runs automatically store their final result as a knowledge chunk (`run:<runId>` source, tenanted to the agent user)
- [x] `backend-client.ts` — listKnowledge, searchKnowledge, ingestText, ingestUrl, deleteKnowledgeChunk
- [x] Knowledge panel wired to backend: Browse lists real chunks with delete; Search calls `/api/knowledge/search`; Ingest POSTs text or URL; live/demo fallback
- [ ] pgvector integration (replace in-memory store when DATABASE_URL is set)
- [ ] Workspace graph visualization (provenance graph across runs/agents/artifacts)

## Phase 7 — Governance Engine ✅ (initial)

- [x] `policy-engine.ts` — tenant-scoped policy CRUD with versioning; rule evaluation (exact tool match + `*` wildcard); built-in "Tool Safety" policy seeds on first run (shell/code_exec/db_query → require-approval, everything else → allow); `.policies.json` persistence
- [x] `audit-log.ts` — append-only JSONL audit log (`.audit.jsonl`); in-memory circular buffer (2000 entries); list with event/tool/run filters
- [x] `approval-gate.ts` — real execution hold: `request()` suspends the tool-proxy via a Promise; `resolve()` unblocks it; broadcasts `governance:approval-request` and `governance:approval-resolved` over WebSocket; 5-min timeout auto-rejects
- [x] `tool-proxy.ts` — policy check before every tool call: deny → blocked result; require-approval → suspends until human resolves; audit entry written for every outcome
- [x] Governance REST API: CRUD on policies, audit log query, approvals list, approve/reject endpoints
- [x] Governance panel wired: Policies tab shows real policies with enable/disable/delete; Audit tab streams real log entries with filter; Approvals tab polls every 5s, renders pending approvals with full input JSON, resolves via real endpoint
- [ ] OPA/OpenFGA integration (replace in-process evaluator)
- [ ] Policy version history and rollback

## Phase 8 — Collaboration & Scale

- [ ] Multi-user workspace sharing with RBAC
- [ ] Agent handoff protocol (structured task pass-off between agents)
- [ ] Workspace graph: provenance tracking across agent runs
- [ ] Desktop packaging (Electron) after browser IDE stabilizes
- [ ] Kubernetes-native workspace provisioning
