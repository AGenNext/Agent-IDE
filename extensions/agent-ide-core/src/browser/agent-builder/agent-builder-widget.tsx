import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { AgentBuilderCommand } from '../agent-ide-commands';

type NodeType = 'input' | 'llm' | 'tool' | 'agent' | 'branch' | 'loop' | 'memory' | 'output';

interface CNode { id: string; type: NodeType; label: string; x: number; y: number; config: Record<string, string>; }
interface CEdge { id: string; sourceId: string; targetId: string; }

const NW = 118, NH = 40, VBW = 860, VBH = 440;

const DEFS: Record<NodeType, { label: string; icon: string; bg: string; fg: string }> = {
    input:  { label: 'Input',   icon: '▶', bg: '#0d2238', fg: '#60b0ff' },
    llm:    { label: 'LLM',     icon: '◈', bg: '#1e0d38', fg: '#c080ff' },
    tool:   { label: 'Tool',    icon: '⚙', bg: '#0d2218', fg: '#60d080' },
    agent:  { label: 'Agent',   icon: '◉', bg: '#131338', fg: '#8080ff' },
    branch: { label: 'Branch',  icon: '③', bg: '#241808', fg: '#d08040' },
    loop:   { label: 'Loop',    icon: '↺', bg: '#1e0d1e', fg: '#d060d0' },
    memory: { label: 'Memory',  icon: '▣', bg: '#0d2222', fg: '#40c0c0' },
    output: { label: 'Output',  icon: '◼', bg: '#220d0d', fg: '#d06060' },
};

let _seq = 10;
function uid() { return `n${++_seq}`; }
function eid() { return `e${++_seq}`; }

const INIT_NODES: CNode[] = [
    { id: 'n1', type: 'input',  label: 'User Query',  x: 30,  y: 200, config: {} },
    { id: 'n2', type: 'llm',    label: 'Planner',     x: 200, y: 200, config: { model: 'claude-opus-4-8', prompt: 'You are a planner.' } },
    { id: 'n3', type: 'tool',   label: 'Web Search',  x: 390, y: 110, config: { toolId: 'web_search' } },
    { id: 'n4', type: 'agent',  label: 'CoderAgent',  x: 390, y: 240, config: { agentId: 'coder-agent-01' } },
    { id: 'n5', type: 'llm',    label: 'Synthesizer', x: 580, y: 200, config: { model: 'claude-sonnet-4-6', prompt: 'Synthesize results into a final answer.' } },
    { id: 'n6', type: 'output', label: 'Result',      x: 740, y: 200, config: {} },
];

const INIT_EDGES: CEdge[] = [
    { id: 'e1', sourceId: 'n1', targetId: 'n2' },
    { id: 'e2', sourceId: 'n2', targetId: 'n3' },
    { id: 'e3', sourceId: 'n2', targetId: 'n4' },
    { id: 'e4', sourceId: 'n3', targetId: 'n5' },
    { id: 'e5', sourceId: 'n4', targetId: 'n5' },
    { id: 'e6', sourceId: 'n5', targetId: 'n6' },
];

function toSvg(e: React.MouseEvent<SVGSVGElement>): [number, number] {
    const r = e.currentTarget.getBoundingClientRect();
    return [(e.clientX - r.left) * (VBW / r.width), (e.clientY - r.top) * (VBH / r.height)];
}

function EdgePath({ src, tgt }: { src: CNode; tgt: CNode }) {
    const x1 = src.x + NW, y1 = src.y + NH / 2;
    const x2 = tgt.x,      y2 = tgt.y + NH / 2;
    const mx = (x1 + x2) / 2;
    return (
        <path
            d={`M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2},${y2}`}
            stroke="#3a5a8a" strokeWidth={2} fill="none" opacity={0.8}
            markerEnd="url(#arrow)"
        />
    );
}

function NodeRect({
    node, selected, connecting, onMouseDown, onClick
}: {
    node: CNode; selected: boolean; connecting: boolean;
    onMouseDown: (e: React.MouseEvent, id: string) => void;
    onClick: (id: string) => void;
}) {
    const d = DEFS[node.type];
    const stroke = selected ? '#fff' : connecting ? '#ffcc00' : `${d.fg}66`;
    return (
        <g
            style={{ cursor: 'grab' }}
            onMouseDown={e => onMouseDown(e, node.id)}
            onClick={e => { e.stopPropagation(); onClick(node.id); }}
        >
            <rect x={node.x} y={node.y} width={NW} height={NH} rx={6}
                fill={d.bg} stroke={stroke} strokeWidth={selected ? 2 : 1} />
            <text x={node.x + 10} y={node.y + NH / 2 + 1} fill={d.fg}
                fontSize={11} fontFamily="monospace" dominantBaseline="middle">
                {d.icon}
            </text>
            <text x={node.x + 26} y={node.y + NH / 2 + 1} fill="#ddd"
                fontSize={11} fontFamily="system-ui" dominantBaseline="middle"
                style={{ userSelect: 'none' }}>
                {node.label.length > 13 ? node.label.slice(0, 12) + '…' : node.label}
            </text>
            <text x={node.x + NW - 6} y={node.y + NH / 2 + 1} fill={`${d.fg}88`}
                fontSize={9} fontFamily="monospace" dominantBaseline="middle" textAnchor="end">
                {d.label}
            </text>
            {/* left port */}
            <circle cx={node.x} cy={node.y + NH / 2} r={4} fill={d.fg} opacity={0.6} />
            {/* right port */}
            <circle cx={node.x + NW} cy={node.y + NH / 2} r={4} fill={d.fg} opacity={0.6} />
        </g>
    );
}

const CONFIG_FIELDS: Record<NodeType, { key: string; label: string; type: 'text' | 'select' | 'textarea'; options?: string[] }[]> = {
    input:  [{ key: 'name', label: 'Input name', type: 'text' }],
    output: [{ key: 'name', label: 'Output name', type: 'text' }],
    llm:    [
        { key: 'model', label: 'Model', type: 'select', options: ['claude-opus-4-8','claude-sonnet-4-6','gpt-4o','gpt-4o-mini','gemini-1.5-pro'] },
        { key: 'prompt', label: 'System prompt', type: 'textarea' },
    ],
    tool:   [{ key: 'toolId', label: 'Tool', type: 'select', options: ['web_search','browser','code_exec','file_rw','http_client','vector_search','db_query','shell'] }],
    agent:  [{ key: 'agentId', label: 'Agent ID', type: 'text' }],
    branch: [{ key: 'condition', label: 'Condition expr', type: 'text' }],
    loop:   [{ key: 'maxIter', label: 'Max iterations', type: 'text' }],
    memory: [{ key: 'memoryType', label: 'Memory type', type: 'select', options: ['vector','sql','graph','key-value'] }],
};

function Inspector({ node, onChange, onDelete }: { node: CNode; onChange: (n: CNode) => void; onDelete: (id: string) => void }) {
    const d = DEFS[node.type];
    const fields = CONFIG_FIELDS[node.type] ?? [];
    return (
        <div style={{ width: 200, borderLeft: '1px solid #1e1e1e', background: '#0d0d0d', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            <div style={{ padding: '8px 10px', borderBottom: '1px solid #1e1e1e', display: 'flex', alignItems: 'center', gap: 6 }}>
                <span style={{ color: d.fg, fontSize: 14 }}>{d.icon}</span>
                <span style={{ fontSize: 12, fontWeight: 700, color: d.fg }}>{d.label}</span>
                <button onClick={() => onDelete(node.id)} style={{ marginLeft: 'auto', background: 'none', border: 'none', color: '#804040', cursor: 'pointer', fontSize: 14 }}>×</button>
            </div>
            <div style={{ flex: 1, overflow: 'auto', padding: 10, display: 'flex', flexDirection: 'column', gap: 10 }}>
                <label style={{ fontSize: 10, color: '#888' }}>Label
                    <input value={node.label} onChange={e => onChange({ ...node, label: e.target.value })}
                        style={{ display: 'block', marginTop: 3, width: '100%', background: '#1a1a1a', border: '1px solid #2a2a2a', color: '#ddd', padding: '4px 6px', borderRadius: 3, fontSize: 12, boxSizing: 'border-box' }} />
                </label>
                {fields.map(f => (
                    <label key={f.key} style={{ fontSize: 10, color: '#888' }}>{f.label}
                        {f.type === 'select' ? (
                            <select value={node.config[f.key] ?? ''} onChange={e => onChange({ ...node, config: { ...node.config, [f.key]: e.target.value } })}
                                style={{ display: 'block', marginTop: 3, width: '100%', background: '#1a1a1a', border: '1px solid #2a2a2a', color: '#ddd', padding: '4px 6px', borderRadius: 3, fontSize: 12 }}>
                                <option value="">-- select --</option>
                                {f.options!.map(o => <option key={o} value={o}>{o}</option>)}
                            </select>
                        ) : f.type === 'textarea' ? (
                            <textarea value={node.config[f.key] ?? ''} onChange={e => onChange({ ...node, config: { ...node.config, [f.key]: e.target.value } })} rows={4}
                                style={{ display: 'block', marginTop: 3, width: '100%', background: '#1a1a1a', border: '1px solid #2a2a2a', color: '#ddd', padding: '4px 6px', borderRadius: 3, fontSize: 11, resize: 'vertical', boxSizing: 'border-box' }} />
                        ) : (
                            <input value={node.config[f.key] ?? ''} onChange={e => onChange({ ...node, config: { ...node.config, [f.key]: e.target.value } })}
                                style={{ display: 'block', marginTop: 3, width: '100%', background: '#1a1a1a', border: '1px solid #2a2a2a', color: '#ddd', padding: '4px 6px', borderRadius: 3, fontSize: 12, boxSizing: 'border-box' }} />
                        )}
                    </label>
                ))}
            </div>
        </div>
    );
}

function AgentBuilderCanvas() {
    const [nodes, setNodes] = React.useState<CNode[]>(INIT_NODES);
    const [edges, setEdges] = React.useState<CEdge[]>(INIT_EDGES);
    const [selectedId, setSelectedId] = React.useState<string | null>(null);
    const [dragging, setDragging] = React.useState<{ id: string; ox: number; oy: number } | null>(null);
    const [connecting, setConnecting] = React.useState<string | null>(null); // sourceId
    const [showExport, setShowExport] = React.useState(false);

    const selectedNode = nodes.find(n => n.id === selectedId) ?? null;

    const addNode = (type: NodeType) => {
        const id = uid();
        const scatter = Math.random() * 40;
        setNodes(ns => [...ns, { id, type, label: DEFS[type].label, x: 300 + scatter, y: 180 + scatter, config: {} }]);
        setSelectedId(id);
    };

    const deleteNode = (id: string) => {
        setNodes(ns => ns.filter(n => n.id !== id));
        setEdges(es => es.filter(e => e.sourceId !== id && e.targetId !== id));
        if (selectedId === id) setSelectedId(null);
    };

    const onNodeMouseDown = (e: React.MouseEvent<Element>, id: string) => {
        e.stopPropagation();
        if (connecting !== null) return;
        const [sx, sy] = toSvg(e as unknown as React.MouseEvent<SVGSVGElement>);
        const node = nodes.find(n => n.id === id)!;
        setDragging({ id, ox: sx - node.x, oy: sy - node.y });
    };

    const onSvgMouseMove = (e: React.MouseEvent<SVGSVGElement>) => {
        if (!dragging) return;
        const [sx, sy] = toSvg(e);
        setNodes(ns => ns.map(n => n.id === dragging.id
            ? { ...n, x: Math.max(0, Math.min(VBW - NW, sx - dragging.ox)), y: Math.max(0, Math.min(VBH - NH, sy - dragging.oy)) }
            : n));
    };

    const onNodeClick = (id: string) => {
        if (connecting !== null) {
            if (connecting !== id && !edges.find(e => e.sourceId === connecting && e.targetId === id)) {
                setEdges(es => [...es, { id: eid(), sourceId: connecting, targetId: id }]);
            }
            setConnecting(null);
        } else {
            setSelectedId(id);
        }
    };

    const exportJson = JSON.stringify({
        version: '0.1.0',
        nodes: nodes.map(({ id, type, label, x, y, config }) => ({ id, type, label, x, y, config })),
        edges: edges.map(({ id, sourceId, targetId }) => ({ id, sourceId, targetId })),
        exportedAt: new Date().toISOString(),
    }, null, 2);

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0d0d0d', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            {/* Toolbar */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 12px', borderBottom: '1px solid #1e1e1e', background: '#111' }}>
                <span style={{ fontWeight: 700, fontSize: 13, color: '#c080ff' }}>◈ Agent Builder</span>
                <button
                    onClick={() => { setConnecting(c => c !== null ? null : ''); }}
                    style={{ padding: '3px 10px', background: connecting !== null ? '#1a2a4a' : '#1a1a1a', border: `1px solid ${connecting !== null ? '#60b0ff' : '#333'}`, color: connecting !== null ? '#60b0ff' : '#888', borderRadius: 3, cursor: 'pointer', fontSize: 11 }}>
                    {connecting !== null ? (connecting ? `→ click target` : '→ click source') : '○ Connect'}
                </button>
                <button onClick={() => setShowExport(s => !s)}
                    style={{ padding: '3px 10px', background: '#1a1a1a', border: '1px solid #333', color: '#888', borderRadius: 3, cursor: 'pointer', fontSize: 11 }}>
                    {'{ } Export'}
                </button>
                <button onClick={() => { setNodes(INIT_NODES); setEdges(INIT_EDGES); setSelectedId(null); }}
                    style={{ padding: '3px 10px', background: '#1a1a1a', border: '1px solid #333', color: '#888', borderRadius: 3, cursor: 'pointer', fontSize: 11 }}>
                    Reset
                </button>
                <span style={{ marginLeft: 'auto', fontSize: 10, color: '#444' }}>{nodes.length} nodes · {edges.length} edges</span>
            </div>

            <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
                {/* Palette */}
                <div style={{ width: 80, borderRight: '1px solid #1e1e1e', overflow: 'auto', paddingTop: 8 }}>
                    {(Object.keys(DEFS) as NodeType[]).map(type => {
                        const d = DEFS[type];
                        return (
                            <div key={type} onClick={() => addNode(type)}
                                style={{ padding: '7px 6px', cursor: 'pointer', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2, borderBottom: '1px solid #181818' }}>
                                <span style={{ fontSize: 16, color: d.fg }}>{d.icon}</span>
                                <span style={{ fontSize: 9, color: '#666' }}>{d.label}</span>
                            </div>
                        );
                    })}
                </div>

                {/* Canvas */}
                <div style={{ flex: 1, overflow: 'hidden', position: 'relative' }}>
                    <svg
                        viewBox={`0 0 ${VBW} ${VBH}`}
                        width="100%" height="100%"
                        style={{ background: '#0a0a14', cursor: connecting !== null ? 'crosshair' : 'default' }}
                        onMouseMove={onSvgMouseMove}
                        onMouseUp={() => setDragging(null)}
                        onClick={() => { if (connecting === '') setConnecting(null); setSelectedId(null); }}
                    >
                        <defs>
                            <marker id="arrow" markerWidth="6" markerHeight="6" refX="5" refY="3" orient="auto">
                                <path d="M0,0 L0,6 L6,3 z" fill="#3a5a8a" />
                            </marker>
                            <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
                                <path d="M 40 0 L 0 0 0 40" fill="none" stroke="#151520" strokeWidth="0.5" />
                            </pattern>
                        </defs>
                        <rect width={VBW} height={VBH} fill="url(#grid)" />

                        {edges.map(e => {
                            const src = nodes.find(n => n.id === e.sourceId);
                            const tgt = nodes.find(n => n.id === e.targetId);
                            if (!src || !tgt) return null;
                            return <EdgePath key={e.id} src={src} tgt={tgt} />;
                        })}

                        {nodes.map(n => (
                            <NodeRect
                                key={n.id} node={n}
                                selected={selectedId === n.id}
                                connecting={connecting !== null && connecting === n.id}
                                onMouseDown={(ev, id) => onNodeMouseDown(ev as unknown as React.MouseEvent<Element>, id)}
                                onClick={onNodeClick}
                            />
                        ))}
                    </svg>

                    {showExport && (
                        <div style={{ position: 'absolute', inset: 0, background: '#000a', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
                            onClick={() => setShowExport(false)}>
                            <div style={{ background: '#111', border: '1px solid #333', borderRadius: 6, padding: 16, width: 480, maxHeight: '80%', overflow: 'auto' }}
                                onClick={e => e.stopPropagation()}>
                                <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8 }}>
                                    <span style={{ fontSize: 12, fontWeight: 700, color: '#ccc' }}>Workflow JSON</span>
                                    <button onClick={() => setShowExport(false)} style={{ background: 'none', border: 'none', color: '#888', cursor: 'pointer' }}>×</button>
                                </div>
                                <pre style={{ margin: 0, fontSize: 11, color: '#a0d080', background: '#0a0a0a', padding: 12, borderRadius: 4, overflow: 'auto' }}>{exportJson}</pre>
                            </div>
                        </div>
                    )}
                </div>

                {/* Inspector */}
                {selectedNode && (
                    <Inspector
                        node={selectedNode}
                        onChange={updated => setNodes(ns => ns.map(n => n.id === updated.id ? updated : n))}
                        onDelete={deleteNode}
                    />
                )}
            </div>
        </div>
    );
}

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
