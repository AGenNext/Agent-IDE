import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { PanelLayout } from '../components/panel-layout';
import { ReplayPanelCommand } from '../agent-ide-commands';

@injectable()
export class ReplayPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:replay';
    static readonly LABEL = 'Replay';

    @postConstruct()
    protected init(): void {
        this.id = ReplayPanelWidget.ID;
        this.title.label = ReplayPanelWidget.LABEL;
        this.title.caption = 'Step through past agent runs';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-history';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <PanelLayout
                title="Replay"
                subtitle="Inspect and replay any past agent run step-by-step"
                purpose="
                    The Replay panel provides a trace-level debugger for agent runs.
                    Select a run from the Runs panel to load its trace.
                    Navigate forwards and backwards through each thought, action,
                    tool call, and observation step. Inspect tool inputs/outputs
                    at each step and compare against expected behavior.
                "
                emptyState="No run selected for replay."
                nextAction="
                    Open the Runs panel, click a completed run, then select
                    'Open in Replay' to begin step-through inspection.
                "
                icon="codicon-history"
            />
        );
    }
}

@injectable()
export class ReplayPanelContribution extends AbstractViewContribution<ReplayPanelWidget> {
    constructor() {
        super({
            widgetId: ReplayPanelWidget.ID,
            widgetName: ReplayPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'bottom', rank: 200 },
            toggleCommandId: ReplayPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(ReplayPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
