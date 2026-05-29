import { ContainerModule } from '@theia/core/shared/inversify';
import { MenuContribution } from '@theia/core/lib/common';
import { FrontendApplicationContribution, WidgetFactory, bindViewContribution } from '@theia/core/lib/browser';

import { AgentIdeMenuContribution } from './agent-ide-menus';
import { AgentDashboardWidget } from './agent-dashboard-widget';
import { AgentDashboardContribution } from './agent-dashboard-contribution';
import { AgentsPanelWidget, AgentsPanelContribution } from './panels/agents-panel-widget';
import { TasksPanelWidget, TasksPanelContribution } from './panels/tasks-panel-widget';
import { KnowledgePanelWidget, KnowledgePanelContribution } from './panels/knowledge-panel-widget';
import { ArtifactsPanelWidget, ArtifactsPanelContribution } from './panels/artifacts-panel-widget';
import { RunsPanelWidget, RunsPanelContribution } from './panels/runs-panel-widget';
import { ReplayPanelWidget, ReplayPanelContribution } from './panels/replay-panel-widget';
import { GovernancePanelWidget, GovernancePanelContribution } from './panels/governance-panel-widget';
import { AgentBuilderWidget, AgentBuilderContribution } from './agent-builder/agent-builder-widget';

export default new ContainerModule(bind => {
    // Menus
    bind(MenuContribution).to(AgentIdeMenuContribution).inSingletonScope();

    // Dashboard — opens in main area on application start
    bindViewContribution(bind, AgentDashboardContribution);
    bind(FrontendApplicationContribution).toService(AgentDashboardContribution);
    bind(AgentDashboardWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: AgentDashboardWidget.ID,
        createWidget: () => ctx.container.get<AgentDashboardWidget>(AgentDashboardWidget),
    })).inSingletonScope();

    // Agents panel — left sidebar
    bindViewContribution(bind, AgentsPanelContribution);
    bind(AgentsPanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: AgentsPanelWidget.ID,
        createWidget: () => ctx.container.get<AgentsPanelWidget>(AgentsPanelWidget),
    })).inSingletonScope();

    // Tasks panel — left sidebar
    bindViewContribution(bind, TasksPanelContribution);
    bind(TasksPanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: TasksPanelWidget.ID,
        createWidget: () => ctx.container.get<TasksPanelWidget>(TasksPanelWidget),
    })).inSingletonScope();

    // Knowledge panel — left sidebar
    bindViewContribution(bind, KnowledgePanelContribution);
    bind(KnowledgePanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: KnowledgePanelWidget.ID,
        createWidget: () => ctx.container.get<KnowledgePanelWidget>(KnowledgePanelWidget),
    })).inSingletonScope();

    // Artifacts panel — left sidebar
    bindViewContribution(bind, ArtifactsPanelContribution);
    bind(ArtifactsPanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: ArtifactsPanelWidget.ID,
        createWidget: () => ctx.container.get<ArtifactsPanelWidget>(ArtifactsPanelWidget),
    })).inSingletonScope();

    // Runs panel — bottom area
    bindViewContribution(bind, RunsPanelContribution);
    bind(RunsPanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: RunsPanelWidget.ID,
        createWidget: () => ctx.container.get<RunsPanelWidget>(RunsPanelWidget),
    })).inSingletonScope();

    // Replay panel — bottom area
    bindViewContribution(bind, ReplayPanelContribution);
    bind(ReplayPanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: ReplayPanelWidget.ID,
        createWidget: () => ctx.container.get<ReplayPanelWidget>(ReplayPanelWidget),
    })).inSingletonScope();

    // Governance panel — right sidebar
    bindViewContribution(bind, GovernancePanelContribution);
    bind(GovernancePanelWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: GovernancePanelWidget.ID,
        createWidget: () => ctx.container.get<GovernancePanelWidget>(GovernancePanelWidget),
    })).inSingletonScope();

    // Agent Builder — main area canvas
    bindViewContribution(bind, AgentBuilderContribution);
    bind(AgentBuilderWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: AgentBuilderWidget.ID,
        createWidget: () => ctx.container.get<AgentBuilderWidget>(AgentBuilderWidget),
    })).inSingletonScope();

    // TODO: auth service binding (Phase 4)
    // TODO: agent runtime service binding (Phase 2)
    // TODO: MCP gateway service binding (Phase 3)
});
