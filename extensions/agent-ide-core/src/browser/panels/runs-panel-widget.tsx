import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { PanelLayout } from '../components/panel-layout';
import { RunsPanelCommand } from '../agent-ide-commands';

@injectable()
export class RunsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:runs';
    static readonly LABEL = 'Runs';

    @postConstruct()
    protected init(): void {
        this.id = RunsPanelWidget.ID;
        this.title.label = RunsPanelWidget.LABEL;
        this.title.caption = 'Agent execution run history';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-play-circle';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <PanelLayout
                title="Runs"
                subtitle="Agent execution history and live status"
                purpose="
                    The Runs panel shows every agent run — past and present.
                    Each run entry shows the agent, task, status, duration,
                    and number of trace steps. Click a run to open it in Replay.
                "
                emptyState="No runs recorded yet."
                nextAction="
                    Assign an agent to a task and execute it. The run will
                    appear here in real time as it executes.
                "
                icon="codicon-play-circle"
            />
        );
    }
}

@injectable()
export class RunsPanelContribution extends AbstractViewContribution<RunsPanelWidget> {
    constructor() {
        super({
            widgetId: RunsPanelWidget.ID,
            widgetName: RunsPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'bottom', rank: 100 },
            toggleCommandId: RunsPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(RunsPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
