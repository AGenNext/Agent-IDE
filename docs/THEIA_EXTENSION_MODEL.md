# Theia Extension Model

This document explains how `extensions/agent-ide-core` is structured as a
Theia extension and how to add new panels or features.

## How Theia Extensions Work

A Theia extension is an npm package that:
1. Declares a `theiaExtensions` array in its `package.json`
2. Each entry points to a compiled `ContainerModule` that binds services
3. The application (`browser-app`) lists the extension as a dependency
4. `@theia/cli` (`theia build`) reads all `theiaExtensions` and generates
   the application bundle, merging all `ContainerModule` instances

## Extension Registration Pattern

All bindings live in `src/browser/frontend-module.ts`:

```typescript
import { ContainerModule } from '@theia/core/shared/inversify';
import { bindViewContribution, WidgetFactory } from '@theia/core/lib/browser';

export default new ContainerModule(bind => {
    bindViewContribution(bind, MyPanelContribution);
    bind(MyPanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: MyPanelWidget.ID,
        createWidget: () => ctx.container.get(MyPanelWidget),
    })).inSingletonScope();
});
```

`bindViewContribution` registers the contribution as:
- `CommandContribution` (registers the toggle command)
- `MenuContribution` (registers menu items)
- `AbstractViewContribution<W>` (provides `openView`, `closeView`, etc.)

## Adding a New Panel

1. Create `extensions/agent-ide-core/src/browser/panels/my-panel-widget.tsx`
2. Export `MyPanelWidget` (extends `ReactWidget`) and `MyPanelContribution`
   (extends `AbstractViewContribution<MyPanelWidget>`)
3. Add a command constant to `agent-ide-commands.ts`
4. Register in `frontend-module.ts` (copy the pattern for existing panels)
5. Add menu item to `agent-ide-menus.ts`

## Widget Lifecycle

```typescript
@injectable()
export class MyPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:my-panel';   // unique across all extensions
    static readonly LABEL = 'My Panel';

    @postConstruct()
    protected init(): void {
        this.id = MyPanelWidget.ID;
        this.title.label = MyPanelWidget.LABEL;
        this.title.iconClass = 'codicon codicon-...';
        this.update();  // triggers initial render
    }

    protected render(): React.ReactNode {
        return <div>...</div>;
    }
}
```

`ReactWidget.update()` schedules a React re-render. Call it whenever state changes.

## Inversify Rules

- **Always** import decorators from `@theia/core/shared/inversify`
- **Never** import from bare `inversify` — container identity mismatch will
  cause "class is not decorated" errors at runtime
- Use `@injectable()` on every class registered in the DI container
- Use `@postConstruct()` for initialization logic instead of constructors
  (constructors run before injection)

## Application Shell Areas

| Area | Description |
|------|-------------|
| `'main'` | Central editor area (tabs) |
| `'left'` | Left sidebar (file explorer, etc.) |
| `'right'` | Right sidebar |
| `'bottom'` | Bottom panel (terminal, problems, etc.) |

Set the area in `AbstractViewContribution` options:
```typescript
defaultWidgetOptions: { area: 'left', rank: 100 }
```
Higher `rank` = appears lower in the sidebar.
