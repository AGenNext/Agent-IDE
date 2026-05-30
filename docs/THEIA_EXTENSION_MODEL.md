# Theia Extension Model

## How to add a new panel

### 1. Create the command constant

Add to `extensions/agent-ide-core/src/browser/agent-ide-commands.ts`:

```typescript
export const MyPanelCommand: Command = {
    id: 'agentIde.openMyPanel',
    label: 'Open My Panel',
    category: 'Agent IDE',
};
```

### 2. Create the widget file

Create `extensions/agent-ide-core/src/browser/panels/my-panel-widget.tsx`:

```typescript
import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { MyPanelCommand } from '../agent-ide-commands';

function MyPanelView() {
    return <div>My Panel content</div>;
}

@injectable()
export class MyPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:mypanel';
    static readonly LABEL = 'My Panel';

    @postConstruct()
    protected init(): void {
        this.id = MyPanelWidget.ID;
        this.title.label = MyPanelWidget.LABEL;
        this.title.caption = 'Description of my panel';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-your-icon';
        this.update();
    }

    protected render(): React.ReactNode {
        return <MyPanelView />;
    }
}

@injectable()
export class MyPanelContribution extends AbstractViewContribution<MyPanelWidget> {
    constructor() {
        super({
            widgetId: MyPanelWidget.ID,
            widgetName: MyPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'left' },  // 'left' | 'right' | 'bottom' | 'main'
            toggleCommandId: MyPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(MyPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
```

### 3. Register in frontend-module.ts

Add import and `bindPanel` call:

```typescript
import { MyPanelWidget, MyPanelContribution } from './panels/my-panel-widget';

// Inside ContainerModule:
bindPanel(bind, MyPanelWidget, MyPanelContribution);
```

### 4. Add to menus (optional)

In `agent-ide-menus.ts`, add:

```typescript
menus.registerMenuAction(group, { commandId: MyPanelCommand.id, order: '11' });
```

---

## Widget lifecycle

```
constructor()
    → InversifyJS creates instance
    → super() called by @injectable() class

@postConstruct() init()
    → Set this.id (must match Widget.ID)
    → Set this.title.label, caption, closable, iconClass
    → Call this.update() to trigger first render

render(): React.ReactNode
    → Called by ReactWidget base class when update() is invoked
    → Returns the React element to render inside the panel
    → Must be pure — any state should live in React hooks

dispose()
    → Called when widget is closed
    → ReactWidget handles React unmounting automatically
```

## InversifyJS rules

1. **Always `@injectable()`** on both Widget and Contribution classes.
2. **Never `new Widget()`** — always inject through InversifyJS.
3. **`bindPanel()`** handles `bindViewContribution` + Widget singleton + WidgetFactory binding.
4. **`@postConstruct()`** is the constructor equivalent — Theia calls it after DI is complete.
5. **Singletons** — all widgets are singletons; `createWidget` returns the same instance every time.
6. **No circular deps** — Widget must not inject Contribution or vice versa.

## React in Theia

Theia ships React 18 via `@theia/core`. Always import as:

```typescript
import * as React from 'react';
```

State: use `React.useState`, `React.useEffect`, `React.useRef`.

After state changes triggered outside React (e.g., from a service), call `this.update()` to re-render. Inside a component, normal React state management applies.

## Codicon icon classes

Common Theia icon classes for `title.iconClass`:
- `codicon codicon-dashboard` — dashboard
- `codicon codicon-robot` — agents
- `codicon codicon-checklist` — tasks
- `codicon codicon-database` — knowledge
- `codicon codicon-archive` — artifacts
- `codicon codicon-play-circle` — runs
- `codicon codicon-debug-rerun` — replay
- `codicon codicon-shield` — governance
- `codicon codicon-plug` — MCP/tools
- `codicon codicon-circuit-board` — builder
- `codicon codicon-server` — platform
- `codicon codicon-lightbulb` — optimize
