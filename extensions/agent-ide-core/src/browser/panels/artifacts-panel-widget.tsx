import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { PanelLayout } from '../components/panel-layout';
import { ArtifactsPanelCommand } from '../agent-ide-commands';

@injectable()
export class ArtifactsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:artifacts';
    static readonly LABEL = 'Artifacts';

    @postConstruct()
    protected init(): void {
        this.id = ArtifactsPanelWidget.ID;
        this.title.label = ArtifactsPanelWidget.LABEL;
        this.title.caption = 'Agent-produced outputs and deliverables';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-package';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <PanelLayout
                title="Artifacts"
                subtitle="Files, code, documents, and data produced by agents"
                purpose="
                    The Artifacts panel tracks every output produced by agents
                    during task execution: source code, reports, data files, models.
                    Artifacts are versioned, linked to the tasks and runs that
                    created them, and can be promoted to the knowledge base.
                "
                emptyState="No artifacts produced yet."
                nextAction="
                    Run a task to generate artifacts, then browse them here.
                    Artifacts can also be imported manually via:
                    `agent-ide artifact import --file ./output.json`
                "
                icon="codicon-package"
            />
        );
    }
}

@injectable()
export class ArtifactsPanelContribution extends AbstractViewContribution<ArtifactsPanelWidget> {
    constructor() {
        super({
            widgetId: ArtifactsPanelWidget.ID,
            widgetName: ArtifactsPanelWidget.LABEL,
            defaultWidgetOptions: { area: 'left', rank: 400 },
            toggleCommandId: ArtifactsPanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(ArtifactsPanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
