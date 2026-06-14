// Fabric.js canvas for peer mesh topology.
// Used by CloudPanel → Mesh face.
// Each peer is a fabric.Group (rounded rect + label).
// Edges are fabric.Line objects connecting them.
// "This instance" is always the center node.

import * as React from 'react';

interface MeshNode {
    id:     string;
    label:  string;
    status: 'online' | 'offline' | 'unknown';
    isSelf?: boolean;
}

interface MeshCanvasProps {
    nodes:  MeshNode[];
    width:  number;
    height: number;
    onNodeClick?: (id: string) => void;
}

const STATUS_COLOR: Record<string, string> = {
    online:  '#40d040',
    offline: '#d04040',
    unknown: '#555555',
};

export function MeshCanvas({ nodes, width, height, onNodeClick }: MeshCanvasProps) {
    const canvasRef = React.useRef<HTMLCanvasElement>(null);
    const fabricRef = React.useRef<any>(null);

    React.useEffect(() => {
        if (!canvasRef.current) return;

        // Lazy-load fabric to avoid SSR issues
        import('fabric').then(({ fabric }) => {
            if (fabricRef.current) { fabricRef.current.dispose(); }

            const canvas = new fabric.Canvas(canvasRef.current!, {
                backgroundColor: '#0f0f0f',
                selection: false,
                renderOnAddRemove: false,
            });
            fabricRef.current = canvas;

            if (nodes.length === 0) {
                canvas.renderAll();
                return;
            }

            // Layout: self in center, peers arranged in a circle
            const cx = width / 2;
            const cy = height / 2;
            const radius = Math.min(width, height) * 0.35;
            const peers = nodes.filter(n => !n.isSelf);
            const selfNode = nodes.find(n => n.isSelf) ?? { id: 'self', label: 'this', status: 'online' as const, isSelf: true };

            // Position map: id → {x, y}
            const positions: Record<string, { x: number; y: number }> = {};
            positions[selfNode.id] = { x: cx, y: cy };
            peers.forEach((p, i) => {
                const angle = (2 * Math.PI * i) / peers.length - Math.PI / 2;
                positions[p.id] = {
                    x: cx + radius * Math.cos(angle),
                    y: cy + radius * Math.sin(angle),
                };
            });

            // Draw edges first (behind nodes)
            peers.forEach(peer => {
                const from = positions[selfNode.id];
                const to   = positions[peer.id];
                const line = new fabric.Line([from.x, from.y, to.x, to.y], {
                    stroke: peer.status === 'online' ? '#1a3a1a' : '#2a1a1a',
                    strokeWidth: 1,
                    strokeDashArray: peer.status === 'unknown' ? [4, 4] : undefined,
                    selectable: false,
                    evented: false,
                });
                canvas.add(line);
            });

            // Draw nodes
            const allNodes = [selfNode, ...peers];
            allNodes.forEach(node => {
                const pos   = positions[node.id];
                const color = STATUS_COLOR[node.status];
                const W = node.isSelf ? 100 : 90;
                const H = node.isSelf ? 44 : 36;

                const rect = new fabric.Rect({
                    width: W, height: H,
                    rx: 6, ry: 6,
                    fill: node.isSelf ? '#0a1a2a' : '#141414',
                    stroke: node.isSelf ? '#4080c0' : color,
                    strokeWidth: node.isSelf ? 2 : 1,
                    originX: 'center', originY: 'center',
                });

                const dot = new fabric.Circle({
                    radius: 4,
                    fill: color,
                    left: -(W / 2) + 10,
                    top: -8,
                    originX: 'center', originY: 'center',
                });

                const label = new fabric.Text(node.label, {
                    fontSize: node.isSelf ? 12 : 10,
                    fill: node.isSelf ? '#80b0e0' : '#c0c0c0',
                    fontFamily: 'monospace',
                    originX: 'center', originY: 'center',
                    top: node.isSelf ? 4 : 2,
                });

                const statusText = new fabric.Text(node.status, {
                    fontSize: 8,
                    fill: color,
                    fontFamily: 'monospace',
                    originX: 'center', originY: 'center',
                    top: node.isSelf ? 16 : 13,
                });

                const group = new fabric.Group([rect, dot, label, statusText], {
                    left: pos.x,
                    top:  pos.y,
                    originX: 'center',
                    originY: 'center',
                    selectable: false,
                    hoverCursor: 'pointer',
                    data: { nodeId: node.id },
                });

                group.on('mousedown', () => {
                    if (onNodeClick && !node.isSelf) onNodeClick(node.id);
                });

                canvas.add(group);
            });

            canvas.renderAll();
        });

        return () => {
            fabricRef.current?.dispose();
            fabricRef.current = null;
        };
    }, [nodes, width, height]);

    return (
        <canvas
            ref={canvasRef}
            width={width}
            height={height}
            style={{ display: 'block', borderRadius: 6, border: '1px solid #1e1e1e' }}
        />
    );
}
