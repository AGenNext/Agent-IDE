import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { PanelLayout } from '../components/panel-layout';
import { TasksPanelCommand } from '../agent-ide-commands';

@injectable()
export class TasksPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:tasks';
    static readonly LABEL = 'Tasks';

    @postConstruct()
    protected init(): void {
        this.id = TasksPanelWidget.ID;
        this.title.label = TasksPanelWidget.LABEL;
        this.title.caption = 'Plan and track agent tasks';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-checklist';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <PanelLayout
                title="Tasks"
                subtitle="Work items assigned to agents"
                purpose="
                    The Tasks panel shows all tasks in the workspace — pending,
                    in progress, completed, or failed. Tasks can be decomposed
                    into subtasks and linked to artifacts produced during execution.
                "
                emptyState="No tasks in this workspace yet."
                nextAction="
                    Create a task via the command palette:
                    Agent IDE: New Task, or use the CLI:
                    `agent-ide task create --title 'My Task'`
                "
                icon="codicon-checklist"
            />
        );
    }
}

@injectable()
export class TasksPanelContribution extends AbstractViewContribution<TasksPanelWidget> {
    constructor() {
        super({
            widgetId: TasksPanelWidget.ID,
            widgetName: TasksPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'left', rank: 200 },
            toggleCommandId: TasksPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(TasksPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
