import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { PanelLayout } from '../components/panel-layout';
import { AgentsPanelCommand } from '../agent-ide-commands';

@injectable()
export class AgentsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:agents';
    static readonly LABEL = 'Agents';

    @postConstruct()
    protected init(): void {
        this.id = AgentsPanelWidget.ID;
        this.title.label = AgentsPanelWidget.LABEL;
        this.title.caption = 'Manage and monitor AI agents';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-robot';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <PanelLayout
                title="Agents"
                subtitle="AI agents registered in this workspace"
                purpose="
                    The Agents panel lists every agent deployed to this workspace.
                    Each agent has a name, version, skill set, and tool list.
                    From here you can inspect agent manifests, start/stop agents,
                    and view their current status.
                "
                emptyState="No agents registered yet."
                nextAction="
                    Define an agent manifest and deploy it via the CLI:
                    `agent-ide agent deploy --manifest ./agents/my-agent.yaml`
                "
                icon="codicon-robot"
            />
        );
    }
}

@injectable()
export class AgentsPanelContribution extends AbstractViewContribution<AgentsPanelWidget> {
    constructor() {
        super({
            widgetId: AgentsPanelWidget.ID,
            widgetName: AgentsPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'left', rank: 100 },
            toggleCommandId: AgentsPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(AgentsPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
