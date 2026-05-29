import { injectable } from '@theia/core/shared/inversify';
import { AbstractViewContribution, FrontendApplicationContribution } from '@theia/core/lib/browser';
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

    async onStart(): Promise<void> {
        // Open the dashboard automatically on first load.
        // TODO: gate on a user preference (e.g. showDashboardOnStart).
        await this.openView({ activate: true });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(AgentDashboardCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
