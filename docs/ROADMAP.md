# Roadmap

## Phase 1 — Foundation (Current)

**Goal**: Working Theia IDE shell with all Agent IDE panels stubbed out.

- [x] Yarn monorepo (applications, extensions, packages)
- [x] TypeScript base config, ESLint, Prettier, EditorConfig
- [x] `packages/agent-ide-types` — full domain type definitions
- [x] `extensions/agent-ide-core` — Theia extension with all panels
- [x] Dashboard widget (main area, auto-opens on start)
- [x] Panels: Agents, Tasks, Knowledge, Artifacts, Runs, Replay, Governance
- [x] Agent Builder (SVG canvas placeholder)
- [x] Command palette entries for all panels
- [x] View menu entries for all panels
- [x] Governance panel UI stub
- [x] Production Dockerfile (Node 22, corepack, port 3000)
- [x] GitHub Actions CI
- [x] Architecture docs
- [x] AGENTS.md (coding agent rules)

## Phase 2 — Agent Runtime

**Goal**: Run real agent tasks with live trace.

- [ ] Backend service: agent executor (LangGraph or custom)
- [ ] WebSocket bridge: frontend ↔ backend run events
- [ ] Runs panel: live run list with status
- [ ] Replay panel: step-through trace viewer
- [ ] Dashboard: live counters (agents, tasks, runs)

## Phase 3 — MCP Gateway

**Goal**: Connect agents to external tools via Model Context Protocol.

- [ ] MCP server registry
- [ ] Tool approval flow (Governance integration)
- [ ] Tool call trace steps in Replay
- [ ] MCP server health monitoring

## Phase 4 — Governance Engine

**Goal**: Enforce policies before every agent action.

- [ ] GovernancePolicyEditor widget
- [ ] Policy evaluation engine (backend)
- [ ] Approval request / approval queue UI
- [ ] Audit log viewer
- [ ] Auth / identity provider integration

## Phase 5 — Knowledge Store

**Goal**: Persistent, queryable knowledge base for agents.

- [ ] Knowledge ingest pipeline
- [ ] Vector search backend
- [ ] Knowledge panel: browse and search
- [ ] Artifact → Knowledge promotion flow

## Phase 6 — Visual Builder

**Goal**: Interactive drag-and-drop agent workflow designer.

- [ ] Replace SVG canvas with `@xyflow/react`
- [ ] Node palette (agent, tool, input, output, condition, loop)
- [ ] Edge labels and type selector
- [ ] Export workflow to agent manifest YAML
- [ ] Import manifest into builder

## Phase 7 — Cloud & Collaboration

**Goal**: Multi-user, remote workspace support.

- [ ] Cloud sync backend
- [ ] Remote workspace management
- [ ] Team governance (role-based policies)
- [ ] Workspace sharing / export
