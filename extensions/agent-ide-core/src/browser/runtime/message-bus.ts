/**
 * Inter-agent message bus — pub/sub with persistent history.
 * Agents subscribe by ID; a sender can target a specific agent or broadcast to all.
 */

export interface AgentMessage {
    id: string;
    fromAgentId: string;
    toAgentId: string; // 'broadcast' for all
    content: string;
    context?: Record<string, unknown>;
    timestamp: string;
    replyToId?: string; // ID of the message this is a reply to
    processed: boolean;
}

export type MessageHandler = (msg: AgentMessage) => void;

let _seq = 0;

export class MessageBus {
    private listeners = new Map<string, MessageHandler[]>();
    private history: AgentMessage[] = [];
    private readonly maxHistory: number;

    constructor(maxHistory = 1000) { this.maxHistory = maxHistory; }

    /** Subscribe to messages addressed to agentId. Returns unsubscribe function. */
    subscribe(agentId: string, handler: MessageHandler): () => void {
        const existing = this.listeners.get(agentId) ?? [];
        this.listeners.set(agentId, [...existing, handler]);
        return () => {
            const handlers = this.listeners.get(agentId) ?? [];
            this.listeners.set(agentId, handlers.filter(h => h !== handler));
        };
    }

    /** Send a message from one agent to another. Returns the created message. */
    send(
        from: string,
        to: string,
        content: string,
        context?: Record<string, unknown>,
        replyToId?: string
    ): AgentMessage {
        const msg: AgentMessage = {
            id: `msg-${++_seq}`,
            fromAgentId: from,
            toAgentId: to,
            content, context, replyToId,
            timestamp: new Date().toISOString(),
            processed: false,
        };
        this.history.push(msg);
        if (this.history.length > this.maxHistory) this.history.shift();

        const handlers = this.listeners.get(to) ?? [];
        handlers.forEach(h => {
            try { h(msg); } catch { /* handler errors must not crash bus */ }
        });
        return msg;
    }

    /** Send to all subscribed agents except the sender. */
    broadcast(from: string, content: string, context?: Record<string, unknown>): void {
        for (const agentId of this.listeners.keys()) {
            if (agentId !== from) this.send(from, agentId, content, context);
        }
    }

    /** Mark a message as processed by its recipient. */
    markProcessed(id: string): void {
        const msg = this.history.find(m => m.id === id);
        if (msg) msg.processed = true;
    }

    /** Get history for a specific agent (sent or received), or all history. */
    getHistory(agentId?: string, limit = 100): AgentMessage[] {
        const all = agentId
            ? this.history.filter(m => m.fromAgentId === agentId || m.toAgentId === agentId || m.toAgentId === 'broadcast')
            : [...this.history];
        return all.slice(-limit);
    }

    getUnprocessed(agentId: string): AgentMessage[] {
        return this.history.filter(m => (m.toAgentId === agentId || m.toAgentId === 'broadcast') && !m.processed);
    }

    clear(): void { this.history = []; }

    get subscriberCount(): number { return this.listeners.size; }
}

export const globalMessageBus = new MessageBus();
