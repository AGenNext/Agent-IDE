# Production Readiness Checklist

This checklist defines what must be true before Agent IDE is treated as production-ready.

## 1. Build integrity

- [ ] `yarn install` completes with no dependency resolution overrides that hide broken packages.
- [ ] `yarn build` succeeds across all workspaces.
- [ ] `yarn typecheck` succeeds across all workspaces.
- [ ] `yarn lint` succeeds across all workspaces.
- [ ] Container build succeeds from a clean checkout.
- [ ] The lockfile is committed after the Theia AI dependency upgrade.

## 2. Theia IDE baseline

- [ ] Workbench starts on port `3000`.
- [ ] Editor opens.
- [ ] Monaco loads.
- [ ] File navigator works.
- [ ] Workspace service works.
- [ ] Terminal works.
- [ ] Search works.
- [ ] SCM view works.
- [ ] Debug/task packages do not break startup.

## 3. Theia AI baseline

- [ ] AI chat UI loads.
- [ ] AI core services initialize.
- [ ] AI IDE agents extension loads.
- [ ] AI terminal assistance is available.
- [ ] MCP UI loads.
- [ ] OpenAI provider can be configured.
- [ ] Anthropic provider can be configured.
- [ ] Google provider can be configured.
- [ ] Ollama provider can be configured for local/offline mode.

## 4. AGenNext enterprise layer

- [ ] Agents panel is connected to live backend data.
- [ ] Runs panel is connected to live traces.
- [ ] Replay panel can replay completed runs.
- [ ] Governance panel can list policies, audit events, and approvals.
- [ ] Identity panel can display current user and API keys.
- [ ] Workspaces panel can create, rename, activate, and delete workspaces.
- [ ] Knowledge panel can ingest and search context.
- [ ] MCP panel can list configured servers and discovered tools.

## 5. Policy and audit

- [ ] Every tool call is evaluated by policy before execution.
- [ ] Every denied call returns a structured error.
- [ ] Every allowed call writes an audit entry.
- [ ] Human approval is required for high-risk tools.
- [ ] Shell execution is disabled by default.
- [ ] Outbound HTTP/browser/search tools are explicitly controlled.
- [ ] API keys are never returned in config responses.

## 6. Runtime and replay

- [ ] Agent runs persist beyond process memory.
- [ ] Run state survives backend restart.
- [ ] Streaming trace events are durable.
- [ ] Replay has deterministic step ordering.
- [ ] Failed runs preserve error context.
- [ ] Cancelled runs stop tool execution.

## 7. Deployment

- [ ] Docker Compose deploy works.
- [ ] k3s deploy works.
- [ ] Health endpoint returns `ok`.
- [ ] Rollout status succeeds.
- [ ] Secrets are mounted from environment or Kubernetes Secret.
- [ ] Volumes persist workspace and runtime data.
- [ ] Container runs as non-root where possible.

## 8. Security

- [ ] Auth is enabled by default for production.
- [ ] CORS is restricted in production.
- [ ] Request body limits are enforced.
- [ ] Tool inputs are validated.
- [ ] Workspace file access is sandboxed to `WORKSPACE_ROOT`.
- [ ] Shell commands are blocked unless explicitly enabled.
- [ ] Audit logs are immutable or append-only in production storage.

## 9. Observability

- [ ] Structured logs are emitted.
- [ ] Health endpoint includes dependency status.
- [ ] Metrics endpoint exists.
- [ ] Run cost, latency, token usage, and tool counts are visible.
- [ ] Errors include correlation IDs.

## 10. Release gate

Agent IDE is production-ready only when all of the following are true:

- Theia 1.73 + Theia AI compile cleanly.
- Theia AI UX is visible and usable.
- AGenNext backend gates every tool call.
- Runtime state is persistent.
- Docker and k3s deployments are verified.
- Auth, audit, policy, and approval are enabled by default for production.
