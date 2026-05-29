import { injectable } from '@theia/core/shared/inversify';
import { AbstractViewContribution, FrontendApplicationContribution, FrontendApplication } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { AgentDashboardWidget } from './agent-dashboard-widget';
import { AgentDashboardCommand } from './agent-ide-commands';

@injectable()
export class AgentDashboardContribution
    extends AbstractViewContribution<AgentDashboardWidget>
    implements FrontendApplicationContribution {

    constructor() {
        super({
            widgetId: AgentDashboardWidget.ID,
            widgetName: AgentDashboardWidget.LABEL,
            defaultWidgetOptions: { area: 'main' },
            toggleCommandId: AgentDashboardCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(AgentDashboardCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }

    async onStart(_app: FrontendApplication): Promise<void> {
        this.openView({ activate: false, reveal: true });
    }
}
