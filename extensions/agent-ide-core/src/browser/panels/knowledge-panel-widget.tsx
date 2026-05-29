import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { PanelLayout } from '../components/panel-layout';
import { KnowledgePanelCommand } from '../agent-ide-commands';

@injectable()
export class KnowledgePanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:knowledge';
    static readonly LABEL = 'Knowledge';

    @postConstruct()
    protected init(): void {
        this.id = KnowledgePanelWidget.ID;
        this.title.label = KnowledgePanelWidget.LABEL;
        this.title.caption = 'Workspace knowledge base';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-book';
        this.update();
    }

    protected render(): React.ReactNode {
        return (
            <PanelLayout
                title="Knowledge"
                subtitle="Shared knowledge available to all agents"
                purpose="
                    The Knowledge panel is the workspace-level knowledge base.
                    Documents, embeddings, and structured data stored here can
                    be retrieved by any agent in the workspace via vector search
                    or structured queries.
                "
                emptyState="Knowledge base is empty."
                nextAction="
                    Ingest documents using the CLI:
                    `agent-ide knowledge ingest --source ./docs/`
                    or connect an external knowledge source in workspace settings.
                "
                icon="codicon-book"
            />
        );
    }
}

@injectable()
export class KnowledgePanelContribution extends AbstractViewContribution<KnowledgePanelWidget> {
    constructor() {
        super({
            widgetId: KnowledgePanelWidget.ID,
            widgetName: KnowledgePanelWidget.LABEL,
            defaultWidgetOptions: { area: 'left', rank: 300 },
            toggleCommandId: KnowledgePanelCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(KnowledgePanelCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}
