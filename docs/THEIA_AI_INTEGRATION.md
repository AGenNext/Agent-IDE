# Theia AI Integration for Agent IDE

Agent IDE now aligns with upstream Eclipse Theia and Theia AI instead of maintaining a separate AI workbench layer.

## Direction

Agent IDE uses Eclipse Theia as the IDE substrate and Theia AI as the AI-native workbench layer. AGenNext owns the enterprise agent layer on top: governance, identity, audit, agent runtime, MCP routing, workspace policy, and production deployment.

## Upstream Theia AI packages enabled in the browser app

The browser application is pinned to the Theia 1.73 line and includes the following AI packages:

- `@theia/ai-core`
- `@theia/ai-core-ui`
- `@theia/ai-chat`
- `@theia/ai-chat-ui`
- `@theia/ai-ide`
- `@theia/ai-terminal`
- `@theia/ai-mcp`
- `@theia/ai-mcp-ui`
- `@theia/ai-openai`
- `@theia/ai-anthropic`
- `@theia/ai-google`
- `@theia/ai-ollama`

## Target architecture

```text
Eclipse Theia Workbench
  - editor
  - terminal
  - navigator
  - SCM
  - debug/task/search

Theia AI Layer
  - AI chat UX
  - agent UX
  - terminal assistance
  - MCP UI
  - provider integrations
  - prompt/model settings

AGenNext Agent IDE Layer
  - Agent Builder
  - Orchestrate
  - Runs
  - Replay
  - Governance
  - Identity
  - Knowledge
  - Workspaces

AGenNext Backend
  - agent execution API
  - MCP gateway
  - policy engine
  - approval gate
  - audit log
  - identity store
  - workspace manager
```

## Production integration rules

1. Do not fork Theia unless upstream extension points are insufficient.
2. Keep Theia and Theia AI package versions pinned together.
3. Let Theia AI own native AI workbench UX: chat, AI terminal, MCP UI, provider adapters, prompt/model settings.
4. Let AGenNext own enterprise semantics: policy, identity, approval, audit, runtime, traces, replay, registry, and workspace contracts.
5. Every AI tool call must pass through AGenNext policy/audit before execution.
6. Every generated change must be reviewable before apply.
7. Local/offline execution must remain supported through Ollama.
8. Cloud model execution must be provider-neutral: OpenAI, Anthropic, Google, and future providers.

## Implementation phases

### Phase 1 — Dependency alignment

Status: started.

- Upgrade browser app Theia packages to `1.73.0`.
- Add Theia AI packages to the browser app.
- Keep the custom `@agennext/agent-ide-core` extension as the AGenNext domain layer.

### Phase 2 — Compile and compatibility pass

- Run `yarn install`.
- Update lockfile.
- Run `yarn build`.
- Fix breaking Theia 1.44 to 1.73 API changes.
- Run `yarn typecheck`.
- Run `yarn lint`.

### Phase 3 — Theia AI workbench activation

- Confirm AI chat is visible in the workbench.
- Confirm model/provider settings are visible.
- Confirm OpenAI, Anthropic, Google, and Ollama provider wiring.
- Confirm MCP UI is visible and can discover MCP tools.
- Confirm terminal assistant integration.

### Phase 4 — AGenNext policy gate

- Route all tool calls through `/api/tools/:id/invoke` or `/api/mcp/tools/:serverId/:toolName/call`.
- Require policy evaluation before execution.
- Write audit events for every model/tool/context action.
- Create review gates for changes suggested by AI agents.

### Phase 5 — Agent IDE domain agents

Create AGenNext domain agents on top of Theia AI:

- Product Manager Agent
- Architect Agent
- Coder Agent
- Reviewer Agent
- Governance Agent
- Research Agent
- Deployment Agent

Each agent must have:

- explicit purpose
- allowed context sources
- allowed tools
- policy profile
- audit trail
- approval requirements

### Phase 6 — Production release

- Build container.
- Push GHCR image.
- Deploy with Docker Compose and k3s.
- Verify health endpoint.
- Verify Theia workbench loads.
- Verify Theia AI chat loads.
- Verify MCP tool discovery.
- Verify AGenNext governance gates.

## Acceptance checklist

- [ ] Browser app compiles on Theia 1.73.
- [ ] Theia AI packages resolve through Yarn.
- [ ] AI chat opens in Agent IDE.
- [ ] Provider configuration works for OpenAI, Anthropic, Google, and Ollama.
- [ ] MCP tools are discoverable from Theia AI and AGenNext backend.
- [ ] All tool execution is policy-gated.
- [ ] All tool execution is audited.
- [ ] AI-generated edits are staged/reviewable before apply.
- [ ] Docker image builds.
- [ ] k3s deployment rolls out.

## Notes

The current change is the first dependency-level integration. It intentionally does not claim production completion until the lockfile, compile pass, runtime wiring, and governance gate are verified.
