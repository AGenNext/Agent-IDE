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
import { PlatformPanelWidget, PlatformPanelContribution } from './panels/platform-panel-widget';
import { ResearchPanelWidget, ResearchPanelContribution } from './panels/research-panel-widget';
import { BenchPanelWidget, BenchPanelContribution } from './panels/bench-panel-widget';
import { OptimizePanelWidget, OptimizePanelContribution } from './panels/optimize-panel-widget';
import { McpPanelWidget, McpPanelContribution } from './panels/mcp-panel-widget';
import { WorkspacesPanelWidget, WorkspacesPanelContribution } from './panels/workspaces-panel-widget';
import { IdentityPanelWidget, IdentityPanelContribution } from './panels/identity-panel-widget';
import { OrchestratePanelWidget, OrchestratePanelContribution } from './panels/orchestrate-panel-widget';
import { OpenHandsPanelWidget, OpenHandsPanelContribution } from './panels/openhands-panel-widget';
import { MetaPanelWidget, MetaPanelContribution } from './panels/meta-panel-widget';

function bindPanel<W, C>(bind: any, Widget: any, Contribution: any): void {
    bindViewContribution(bind, Contribution);
    bind(Widget).toSelf();
    bind(WidgetFactory).toDynamicValue((ctx: any) => ({
        id: Widget.ID,
        createWidget: () => ctx.container.get(Widget),
    })).inSingletonScope();
}

export default new ContainerModule(bind => {
    bind(MenuContribution).to(AgentIdeMenuContribution).inSingletonScope();

    // Dashboard — auto-opens on start
    bindViewContribution(bind, AgentDashboardContribution);
    bind(FrontendApplicationContribution).toService(AgentDashboardContribution);
    bind(AgentDashboardWidget).toSelf();
    bind(WidgetFactory).toDynamicValue(ctx => ({
        id: AgentDashboardWidget.ID,
        createWidget: () => ctx.container.get<AgentDashboardWidget>(AgentDashboardWidget),
    })).inSingletonScope();

    // Left sidebar panels
    bindPanel(bind, AgentsPanelWidget,    AgentsPanelContribution);
    bindPanel(bind, TasksPanelWidget,     TasksPanelContribution);
    bindPanel(bind, KnowledgePanelWidget, KnowledgePanelContribution);
    bindPanel(bind, ArtifactsPanelWidget, ArtifactsPanelContribution);
    bindPanel(bind, McpPanelWidget,        McpPanelContribution);
    bindPanel(bind, WorkspacesPanelWidget, WorkspacesPanelContribution);
    bindPanel(bind, IdentityPanelWidget,   IdentityPanelContribution);

    // Bottom panels
    bindPanel(bind, RunsPanelWidget,   RunsPanelContribution);
    bindPanel(bind, ReplayPanelWidget, ReplayPanelContribution);

    // Right sidebar
    bindPanel(bind, GovernancePanelWidget, GovernancePanelContribution);
    bindPanel(bind, OptimizePanelWidget,   OptimizePanelContribution);

    // Main area panels — AgentBuilder is the default landing view
    bindPanel(bind, AgentBuilderWidget,  AgentBuilderContribution);
    bind(FrontendApplicationContribution).toService(AgentBuilderContribution);
    bindPanel(bind, PlatformPanelWidget, PlatformPanelContribution);
    bindPanel(bind, ResearchPanelWidget, ResearchPanelContribution);
    bindPanel(bind, BenchPanelWidget,        BenchPanelContribution);
    bindPanel(bind, OrchestratePanelWidget,  OrchestratePanelContribution);
    bindPanel(bind, OpenHandsPanelWidget,    OpenHandsPanelContribution);
    bindPanel(bind, MetaPanelWidget,         MetaPanelContribution);

});
