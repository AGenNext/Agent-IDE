import { WebSocket, WebSocketServer } from 'ws';
import { WsMessage } from './types';

let wss: WebSocketServer | null = null;

export function attachWebSocket(server: import('http').Server): void {
    wss = new WebSocketServer({ server, path: '/ws' });

    wss.on('connection', (ws: WebSocket, req) => {
        const url = req.url ?? '';
        const runId = url.split('/').pop() ?? '';

        ws.on('message', (data) => {
            try {
                const msg = JSON.parse(data.toString()) as { type: string; runId?: string };
                if (msg.type === 'subscribe' && msg.runId) {
                    (ws as WebSocket & { runId?: string }).runId = msg.runId;
                }
            } catch { /* ignore malformed messages */ }
        });

        // Tag the socket with runId from URL path if provided
        if (runId && runId !== 'ws') {
            (ws as WebSocket & { runId?: string }).runId = runId;
        }

        ws.send(JSON.stringify({ type: 'connected', ts: new Date().toISOString() }));
    });
}

export function broadcast(msg: WsMessage): void {
    if (!wss) return;
    const payload = JSON.stringify(msg);
    wss.clients.forEach((client) => {
        if (client.readyState !== WebSocket.OPEN) return;
        const tagged = client as WebSocket & { runId?: string };
        // Send to subscribers of this run, or to clients subscribed to all runs
        if (!tagged.runId || tagged.runId === msg.runId || tagged.runId === '*') {
            client.send(payload);
        }
    });
}
