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

## Phase 3 — Live Agent Execution

- [ ] Wire `agent-runtime.ts` to real Anthropic/OpenAI API endpoints
- [ ] Backend Express server for proxy, filesystem, DB, and shell tools
- [ ] Live Runs panel — real trace steps streaming from backend
- [ ] Tool execution approval flow (from Governance to tool call)
- [ ] Replay panel reading real persisted traces from `persistence.ts`
- [ ] WebSocket message bus (`message-bus.ts`) connected to backend

## Phase 4 — MCP Gateway

- [ ] Real MCP server connections (stdio via backend bridge)
- [ ] MCP panel: live server connect/disconnect with status polling
- [ ] Tool call routing through governance approval gate
- [ ] MCP server configuration persistence (`.mcp.json` in workspace)
- [ ] Tool schema discovery from live MCP servers

## Phase 5 — Authentication & Workspaces

- [ ] Clerk/Auth0 authentication integration
- [ ] Per-user workspace provisioning (isolated Docker containers)
- [ ] Workspace API (REST) for workspace CRUD
- [ ] Session persistence across browser restarts

## Phase 6 — Knowledge & Memory

- [ ] pgvector integration for real vector search
- [ ] Knowledge ingest pipeline (web → chunk → embed → store)
- [ ] Workspace graph visualization (replacing SVG placeholder)
- [ ] Agent memory: short-term (run context) and long-term (vector store)

## Phase 7 — Governance Engine

- [ ] OPA/OpenFGA policy evaluation engine
- [ ] Real-time policy check on every tool call
- [ ] Audit log persistence to database
- [ ] Approval gate UI connected to real execution hold
- [ ] Policy version control and rollback

## Phase 8 — Collaboration & Scale

- [ ] Multi-user workspace sharing with RBAC
- [ ] Agent handoff protocol (structured task pass-off between agents)
- [ ] Workspace graph: provenance tracking across agent runs
- [ ] Desktop packaging (Electron) after browser IDE stabilizes
- [ ] Kubernetes-native workspace provisioning
