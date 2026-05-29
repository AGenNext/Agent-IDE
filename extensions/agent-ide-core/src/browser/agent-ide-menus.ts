import { injectable } from '@theia/core/shared/inversify';
import { MenuContribution, MenuModelRegistry } from '@theia/core/lib/common';
import { CommonMenus } from '@theia/core/lib/browser';
import {
    AgentDashboardCommand,
    AgentsPanelCommand,
    TasksPanelCommand,
    KnowledgePanelCommand,
    ArtifactsPanelCommand,
    RunsPanelCommand,
    ReplayPanelCommand,
    GovernancePanelCommand,
    AgentBuilderCommand,
} from './agent-ide-commands';

@injectable()
export class AgentIdeMenuContribution implements MenuContribution {
    registerMenus(menus: MenuModelRegistry): void {
        const panels = [
            AgentDashboardCommand,
            AgentsPanelCommand,
            TasksPanelCommand,
            KnowledgePanelCommand,
            ArtifactsPanelCommand,
            RunsPanelCommand,
            ReplayPanelCommand,
            GovernancePanelCommand,
            AgentBuilderCommand,
        ];

        panels.forEach((cmd, i) => {
            menus.registerMenuAction(CommonMenus.VIEW_VIEWS, {
                commandId: cmd.id,
                label: cmd.label,
                order: String(i).padStart(2, '0'),
            });
        });
    }
}
