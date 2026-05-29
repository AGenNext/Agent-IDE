import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser/widgets/react-widget';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import * as React from 'react';
import { AgentBuilderCommand } from '../agent-ide-commands';

@injectable()
export class AgentBuilderWidget extends ReactWidget {
    static readonly ID = 'agent-ide:builder';
    static readonly LABEL = 'Agent Builder';

    @postConstruct()
    protected init(): void {
        this.id = AgentBuilderWidget.ID;
        this.title.label = AgentBuilderWidget.LABEL;
        this.title.caption = 'Visual agent workflow designer';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-circuit-board';
        this.update();
    }

    protected render(): React.ReactNode {
        return <AgentBuilderCanvas />;
    }
}

@injectable()
export class AgentBuilderContribution extends AbstractViewContribution<AgentBuilderWidget> {
    constructor() {
        super({
            widgetId: AgentBuilderWidget.ID,
            widgetName: AgentBuilderWidget.LABEL,
            defaultWidgetOptions: { area: 'main' },
            toggleCommandId: AgentBuilderCommand.id,
        });
    }

    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(AgentBuilderCommand, {
            execute: () => this.openView({ activate: true }),
        });
    }
}

// ---------------------------------------------------------------------------
// Canvas component — placeholder graph renderer.
// TODO: Replace SVG canvas with @xyflow/react <ReactFlow> when integrating
//       the XYFlow graph editor. Node/edge data shapes already match
//       WorkspaceGraphNode / WorkspaceGraphEdge from @agennext/agent-ide-types.
// ---------------------------------------------------------------------------

interface GNode { id: string; label: string; type: string; x: number; y: number; }
interface GEdge { id: string; source: string; target: string; }

const DEMO_NODES: GNode[] = [
    { id: '1', label: 'User Input',   type: 'input',  x: 40,  y: 120 },
    { id: '2', label: 'Planner',      type: 'agent',  x: 240, y: 120 },
    { id: '3', label: 'Web Search',   type: 'tool',   x: 440, y: 60  },
    { id: '4', label: 'Code Writer',  type: 'agent',  x: 440, y: 140 },
    { id: '5', label: 'Artifact Out', type: 'output', x: 640, y: 120 },
];

const DEMO_EDGES: GEdge[] = [
    { id: 'e1', source: '1', target: '2' },
    { id: 'e2', source: '2', target: '3' },
    { id: 'e3', source: '2', target: '4' },
    { id: 'e4', source: '3', target: '5' },
    { id: 'e5', source: '4', target: '5' },
];

const NODE_COLORS: Record<string, string> = {
    input:  '#1a5c8a',
    agent:  '#4a2d8a',
    tool:   '#1a6e3c',
    output: '#6e1a1a',
};

const NODE_W = 120;
const NODE_H = 40;

const AgentBuilderCanvas: React.FC = () => (
    <div className="agent-builder">
        <div className="agent-builder__header">
            <h2 className="agent-builder__title">Agent Builder</h2>
            <p className="agent-builder__subtitle">
                Visual agent workflow designer. XYFlow integration planned for Phase 2.
            </p>
        </div>

        <div
            className="agent-builder__canvas-wrap"
            style={{ background: '#1a1a2e', borderRadius: 8, padding: 16, marginTop: 12 }}
        >
            <svg
                width="100%"
                height={240}
                viewBox="0 0 800 240"
                xmlns="http://www.w3.org/2000/svg"
            >
                {/* Edges */}
                {DEMO_EDGES.map(edge => {
                    const src = DEMO_NODES.find(n => n.id === edge.source);
                    const tgt = DEMO_NODES.find(n => n.id === edge.target);
                    if (!src || !tgt) return null;
                    const x1 = src.x + NODE_W;
                    const y1 = src.y + NODE_H / 2;
                    const x2 = tgt.x;
                    const y2 = tgt.y + NODE_H / 2;
                    const mx = (x1 + x2) / 2;
                    return (
                        <path
                            key={edge.id}
                            d={`M ${x1} ${y1} C ${mx} ${y1} ${mx} ${y2} ${x2} ${y2}`}
                            stroke="#4e9eff"
                            strokeWidth={1.5}
                            fill="none"
                            opacity={0.7}
                        />
                    );
                })}

                {/* Nodes */}
                {DEMO_NODES.map(node => (
                    <g key={node.id} transform={`translate(${node.x}, ${node.y})`}>
                        <rect
                            width={NODE_W}
                            height={NODE_H}
                            rx={6}
                            fill={NODE_COLORS[node.type] ?? '#444'}
                        />
                        <text
                            x={NODE_W / 2}
                            y={NODE_H / 2 + 5}
                            textAnchor="middle"
                            fill="white"
                            fontSize={12}
                            fontFamily="system-ui, sans-serif"
                        >
                            {node.label}
                        </text>
                    </g>
                ))}
            </svg>
        </div>

        <p style={{ color: '#666', fontSize: 11, marginTop: 8 }}>
            // TODO: Replace with <code>@xyflow/react</code> &lt;ReactFlow&gt; for interactive
            drag-and-drop editing. Node shape maps to WorkspaceGraphNode, edges to
            WorkspaceGraphEdge.
        </p>
    </div>
);
