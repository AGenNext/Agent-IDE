import { injectable } from '@theia/core/shared/inversify';
import { MenuContribution, MenuModelRegistry } from '@theia/core/lib/common';
import {
    AgentDashboardCommand, AgentsPanelCommand, TasksPanelCommand, KnowledgePanelCommand,
    ArtifactsPanelCommand, RunsPanelCommand, ReplayPanelCommand, GovernancePanelCommand,
    AgentBuilderCommand, PlatformPanelCommand, ResearchPanelCommand, BenchPanelCommand,
    OptimizePanelCommand, McpPanelCommand, WorkspacesPanelCommand, IdentityPanelCommand,
    OrchestratePanelCommand, OpenHandsPanelCommand,
} from './agent-ide-commands';

const AGENT_IDE_MENU = ['menubar', 'agentide'];

@injectable()
export class AgentIdeMenuContribution implements MenuContribution {
    registerMenus(menus: MenuModelRegistry): void {
        menus.registerSubmenu(AGENT_IDE_MENU, 'Agent IDE');

        const group = [...AGENT_IDE_MENU, '1_panels'];
        menus.registerMenuAction(group, { commandId: AgentDashboardCommand.id,  order: '01' });
        menus.registerMenuAction(group, { commandId: AgentBuilderCommand.id,    order: '02' });
        menus.registerMenuAction(group, { commandId: AgentsPanelCommand.id,     order: '03' });
        menus.registerMenuAction(group, { commandId: TasksPanelCommand.id,      order: '04' });
        menus.registerMenuAction(group, { commandId: KnowledgePanelCommand.id,  order: '05' });
        menus.registerMenuAction(group, { commandId: ArtifactsPanelCommand.id,  order: '06' });
        menus.registerMenuAction(group, { commandId: RunsPanelCommand.id,       order: '07' });
        menus.registerMenuAction(group, { commandId: ReplayPanelCommand.id,     order: '08' });
        menus.registerMenuAction(group, { commandId: GovernancePanelCommand.id, order: '09' });
        menus.registerMenuAction(group, { commandId: McpPanelCommand.id,        order: '10' });
        menus.registerMenuAction(group, { commandId: WorkspacesPanelCommand.id, order: '11' });
        menus.registerMenuAction(group, { commandId: IdentityPanelCommand.id,   order: '12' });

        const evalGroup = [...AGENT_IDE_MENU, '2_eval'];
        menus.registerMenuAction(evalGroup, { commandId: PlatformPanelCommand.id,  order: '01' });
        menus.registerMenuAction(evalGroup, { commandId: ResearchPanelCommand.id,  order: '02' });
        menus.registerMenuAction(evalGroup, { commandId: BenchPanelCommand.id,     order: '03' });
        menus.registerMenuAction(evalGroup, { commandId: OptimizePanelCommand.id,     order: '04' });
        menus.registerMenuAction(evalGroup, { commandId: OrchestratePanelCommand.id,  order: '05' });
        menus.registerMenuAction(evalGroup, { commandId: OpenHandsPanelCommand.id,    order: '06' });
    }
}
