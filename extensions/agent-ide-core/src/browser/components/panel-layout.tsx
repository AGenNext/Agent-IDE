import * as React from 'react';

export interface PanelLayoutProps {
    title: string;
    subtitle: string;
    purpose: string;
    emptyState: string;
    nextAction: string;
    icon: string;
    children?: React.ReactNode;
}

/**
 * Shared layout for all Agent IDE panel widgets.
 * Extension point: replace children with real data views as runtime capabilities are added.
 */
export const PanelLayout: React.FC<PanelLayoutProps> = ({
    title,
    subtitle,
    purpose,
    emptyState,
    nextAction,
    icon,
    children,
}) => (
    <div className="agent-ide-panel">
        <div className="agent-ide-panel__header">
            <span className={`codicon ${icon} agent-ide-panel__icon`} />
            <div className="agent-ide-panel__heading">
                <h2 className="agent-ide-panel__title">{title}</h2>
                <p className="agent-ide-panel__subtitle">{subtitle}</p>
            </div>
        </div>

        <div className="agent-ide-panel__purpose">
            <p>{purpose}</p>
        </div>

        {children ? (
            <div className="agent-ide-panel__content">{children}</div>
        ) : (
            <div className="agent-ide-panel__empty">
                <div className="agent-ide-panel__empty-icon">
                    <span className={`codicon ${icon}`} />
                </div>
                <p className="agent-ide-panel__empty-message">{emptyState}</p>
                <div className="agent-ide-panel__next-action">
                    <span className="codicon codicon-arrow-right" />
                    <span>{nextAction}</span>
                </div>
            </div>
        )}
    </div>
);
