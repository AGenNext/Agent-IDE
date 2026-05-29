import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { PanelLayout } from '../components/panel-layout';
import { GovernancePanelCommand } from '../agent-ide-commands';

@injectable()
export class GovernancePanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:governance';
    static readonly LABEL = 'Governance';

    @postConstruct()
    protected init(): void {
        this.id = GovernancePanelWidget.ID;
        this.title.label = GovernancePanelWidget.LABEL;
        this.title.caption = 'Agent policies and guardrails';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-shield';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <PanelLayout
                title="Governance"
                subtitle="Policies, guardrails, and audit controls"
                purpose="
                    The Governance panel manages the policies that constrain
                    agent behavior at the workspace level. Policies can allow,
                    deny, require approval for, or audit-only specific agent
                    actions — by agent, tool, task scope, or globally.
                    All policy decisions are logged for audit.
                "
                emptyState="No governance policies defined."
                nextAction="
                    Create your first policy via the manifest editor.
                    A minimal policy that requires approval for all tool calls
                    is a safe starting point for new workspaces.
                "
                icon="codicon-shield"
            />
        );
    }
}

@injectable()
export class GovernancePanelContribution extends AbstractViewContribution<GovernancePanelWidget> {
    constructor() {
        super({
            widgetId: GovernancePanelWidget.ID,
            widgetName: GovernancePanelWidget.LABEL,
            defaultWidgetOptions: { area: 'right', rank: 100 },
            toggleCommandId: GovernancePanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(GovernancePanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
