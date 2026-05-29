# Agent IDE Architecture

## Monorepo topology

```
Agent IDE (yarn workspaces)
│
├── packages/
│   └── agent-ide-types/          Domain type library (no Theia dep)
│
├── extensions/
│   └── agent-ide-core/           Theia frontend extension
│       └── src/browser/
│           ├── frontend-module.ts         InversifyJS container
│           ├── agent-ide-commands.ts      All command constants
│           ├── agent-ide-menus.ts         Menu registrations
│           ├── agent-dashboard-widget.tsx Dashboard (auto-opens)
│           ├── agent-dashboard-contribution.ts
│           ├── agent-builder/             Visual workflow canvas
│           ├── panels/                    All sidebar/bottom/main panels
│           └── runtime/                   Agent runtime, bus, tools
│
└── applications/
    └── browser-app/              Theia browser app shell (port 3000)
```

## Build topology

```
agent-ide-types → agent-ide-core → browser-app
       ↑                 ↑
  (no deps)        (@theia/core)
```

## Panel table

| Panel          | Widget ID              | Area    | Purpose |
|----------------|------------------------|---------|---------|
| Dashboard      | agent-ide:dashboard    | main    | Workspace overview, stats, quick links |
| Agent Builder  | agent-ide:builder      | main    | Visual workflow graph editor |
| Platform       | agent-ide:platform     | main    | FinOps, tracing, monitoring |
| Research       | agent-ide:research     | main    | Multi-source research orchestration |
| Bench          | agent-ide:bench        | main    | AgentBench evaluation harness |
| Agents         | agent-ide:agents       | left    | Agent modeling (5-section form) |
| Tasks          | agent-ide:tasks        | left    | Task queue and assignments |
| Knowledge      | agent-ide:knowledge    | left    | Knowledge base browse/search/ingest |
| Artifacts      | agent-ide:artifacts    | left    | 12 artifact types with viewer |
| Runs           | agent-ide:runs         | bottom  | Trace, token flow, performance eval |
| Replay         | agent-ide:replay       | bottom  | Step-through trace replay |
| Governance     | agent-ide:governance   | right   | Policies, audit log, approval queue |
| Optimize       | agent-ide:optimize     | right   | Prompt, cost, latency tuning |

## Extension registration pattern

Every panel follows this pattern:

```typescript
// 1. React component (function)
function MyPanelView() { ... }

// 2. Widget class
@injectable()
export class MyPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:mypanel';
    static readonly LABEL = 'My Panel';
    @postConstruct() protected init(): void { ... }
    protected render(): React.ReactNode { return <MyPanelView />; }
}

// 3. Contribution class
@injectable()
export class MyPanelContribution extends AbstractViewContribution<MyPanelWidget> {
    constructor() {
        super({ widgetId: MyPanelWidget.ID, widgetName: MyPanelWidget.LABEL,
                defaultWidgetOptions: { area: 'left' }, toggleCommandId: MyPanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(MyPanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
```

Both Widget and Contribution are bound in `frontend-module.ts` via `bindPanel()`.

## Runtime layer (Phase 2 placeholder)

`extensions/agent-ide-core/src/browser/runtime/` contains:
- `agent-runtime.ts` — OpenAI-compatible agent execution loop
- `orchestrator.ts` — multi-agent task orchestration
- `tool-registry.ts` — tool registration and dispatch
- `message-bus.ts` — in-process event bus
- `persistence.ts` — workspace state persistence

These are wired but not yet connected to live backends. Phase 2 will bridge them to real LLM APIs.
