import { spawn, ChildProcess } from 'child_process';
import * as readline from 'readline';
import * as fs from 'fs/promises';
import * as path from 'path';

export type McpTransport = 'stdio' | 'sse' | 'websocket';
export type McpStatus = 'connected' | 'connecting' | 'disconnected' | 'error';

export interface McpServerConfig {
    id: string;
    name: string;
    transport: McpTransport;
    command?: string;            // stdio: full shell command
    endpoint?: string;           // sse/ws: URL
    env?: Record<string, string>;
    autoConnect?: boolean;
}

export interface McpTool {
    serverId: string;
    serverName: string;
    name: string;
    description: string;
    inputSchema: Record<string, unknown>;
}

export interface McpServerState extends McpServerConfig {
    status: McpStatus;
    tools: McpTool[];
    error?: string;
    connectedAt?: string;
    toolCount: number;
}

interface PendingRequest {
    resolve: (v: unknown) => void;
    reject:  (e: Error) => void;
    timer:   ReturnType<typeof setTimeout>;
}

interface Connection {
    config: McpServerConfig;
    status: McpStatus;
    process?: ChildProcess;
    tools: McpTool[];
    error?: string;
    connectedAt?: string;
    pending: Map<number, PendingRequest>;
    nextId: number;
    buffer: string;
}

const CONFIG_PATH = path.join(process.env.WORKSPACE_ROOT ?? process.cwd(), '.mcp.json');
const DEFAULT_SERVERS: McpServerConfig[] = [
    { id: 'filesystem',   name: 'filesystem',   transport: 'stdio', command: 'npx -y @modelcontextprotocol/server-filesystem .', autoConnect: false },
    { id: 'brave-search', name: 'brave-search', transport: 'stdio', command: 'npx -y @modelcontextprotocol/server-brave-search', env: { BRAVE_API_KEY: '${BRAVE_API_KEY}' }, autoConnect: false },
    { id: 'postgres',     name: 'postgres',     transport: 'stdio', command: 'npx -y @modelcontextprotocol/server-postgres ${DATABASE_URL}', autoConnect: false },
    { id: 'puppeteer',    name: 'puppeteer',     transport: 'stdio', command: 'npx -y @modelcontextprotocol/server-puppeteer', autoConnect: false },
    { id: 'github',       name: 'github',        transport: 'stdio', command: 'npx -y @modelcontextprotocol/server-github', env: { GITHUB_TOKEN: '${GITHUB_TOKEN}' }, autoConnect: false },
];

class McpManager {
    private connections = new Map<string, Connection>();
    private configs = new Map<string, McpServerConfig>();

    async loadConfig(): Promise<void> {
        let servers: McpServerConfig[] = DEFAULT_SERVERS;
        try {
            const raw = await fs.readFile(CONFIG_PATH, 'utf-8');
            const parsed = JSON.parse(raw) as { servers: McpServerConfig[] };
            if (Array.isArray(parsed.servers)) servers = parsed.servers;
        } catch {
            // No config file yet — use defaults, write them out
            await this.saveConfig(DEFAULT_SERVERS);
        }
        for (const s of servers) this.configs.set(s.id, s);
        // Auto-connect servers marked as such
        for (const s of servers) {
            if (s.autoConnect) this.connect(s.id).catch(() => {/* log silently */});
        }
    }

    async saveConfig(servers?: McpServerConfig[]): Promise<void> {
        const list = servers ?? [...this.configs.values()];
        await fs.mkdir(path.dirname(CONFIG_PATH), { recursive: true });
        await fs.writeFile(CONFIG_PATH, JSON.stringify({ servers: list }, null, 2));
    }

    listServers(): McpServerState[] {
        return [...this.configs.values()].map(cfg => {
            const conn = this.connections.get(cfg.id);
            return {
                ...cfg,
                status: conn?.status ?? 'disconnected',
                tools: conn?.tools ?? [],
                toolCount: conn?.tools.length ?? 0,
                error: conn?.error,
                connectedAt: conn?.connectedAt,
            };
        });
    }

    getServer(id: string): McpServerState | undefined {
        const cfg = this.configs.get(id);
        if (!cfg) return undefined;
        const conn = this.connections.get(id);
        return { ...cfg, status: conn?.status ?? 'disconnected', tools: conn?.tools ?? [], toolCount: conn?.tools.length ?? 0, error: conn?.error, connectedAt: conn?.connectedAt };
    }

    getAllTools(): McpTool[] {
        const tools: McpTool[] = [];
        for (const conn of this.connections.values()) {
            if (conn.status === 'connected') tools.push(...conn.tools);
        }
        return tools;
    }

    async addServer(cfg: McpServerConfig): Promise<void> {
        this.configs.set(cfg.id, cfg);
        await this.saveConfig();
    }

    async removeServer(id: string): Promise<void> {
        await this.disconnect(id);
        this.configs.delete(id);
        await this.saveConfig();
    }

    async connect(id: string): Promise<McpServerState> {
        const cfg = this.configs.get(id);
        if (!cfg) throw new Error(`Unknown MCP server: ${id}`);
        if (cfg.transport !== 'stdio') throw new Error(`Transport ${cfg.transport} not yet supported in backend bridge (use direct connection)`);
        if (!cfg.command) throw new Error(`No command configured for server ${id}`);

        // Disconnect first if already connected
        await this.disconnect(id);

        const conn: Connection = { config: cfg, status: 'connecting', tools: [], pending: new Map(), nextId: 1, buffer: '' };
        this.connections.set(id, conn);

        try {
            // Expand env var references like ${BRAVE_API_KEY}
            const resolvedEnv: Record<string, string> = {};
            for (const [k, v] of Object.entries(cfg.env ?? {})) {
                resolvedEnv[k] = v.replace(/\$\{(\w+)\}/g, (_, name) => process.env[name] ?? '');
            }

            const child = spawn('sh', ['-c', cfg.command], {
                env: { ...process.env, ...resolvedEnv },
                stdio: ['pipe', 'pipe', 'pipe'],
            });
            conn.process = child;

            child.on('error', (err) => {
                conn.status = 'error';
                conn.error = err.message;
                this.drainPending(conn, err);
            });

            child.on('exit', (code) => {
                if (conn.status === 'connected' || conn.status === 'connecting') {
                    conn.status = 'error';
                    conn.error = `Process exited with code ${code}`;
                    this.drainPending(conn, new Error(conn.error));
                }
            });

            // Read stdout line by line (JSON-RPC responses)
            const rl = readline.createInterface({ input: child.stdout! });
            rl.on('line', (line) => this.handleLine(conn, line));

            // MCP initialization sequence
            await this.rpc(conn, 'initialize', {
                protocolVersion: '2024-11-05',
                capabilities: { tools: {} },
                clientInfo: { name: 'agent-ide-backend', version: '0.1.0' },
            });

            // Send initialized notification
            child.stdin!.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');

            // Discover tools
            const toolsResult = await this.rpc(conn, 'tools/list', {}) as { tools: Array<{ name: string; description?: string; inputSchema?: unknown }> };
            conn.tools = (toolsResult.tools ?? []).map(t => ({
                serverId: id,
                serverName: cfg.name,
                name: t.name,
                description: t.description ?? '',
                inputSchema: (t.inputSchema ?? {}) as Record<string, unknown>,
            }));

            conn.status = 'connected';
            conn.connectedAt = new Date().toISOString();
            conn.error = undefined;

        } catch (err: unknown) {
            conn.status = 'error';
            conn.error = err instanceof Error ? err.message : String(err);
            conn.process?.kill();
        }

        return this.getServer(id)!;
    }

    async disconnect(id: string): Promise<void> {
        const conn = this.connections.get(id);
        if (!conn) return;
        conn.status = 'disconnected';
        this.drainPending(conn, new Error('Disconnected'));
        conn.process?.stdin?.end();
        conn.process?.kill('SIGTERM');
        this.connections.delete(id);
    }

    async callTool(serverId: string, toolName: string, args: Record<string, unknown>): Promise<unknown> {
        const conn = this.connections.get(serverId);
        if (!conn || conn.status !== 'connected') throw new Error(`Server ${serverId} is not connected`);
        const result = await this.rpc(conn, 'tools/call', { name: toolName, arguments: args });
        return result;
    }

    private handleLine(conn: Connection, line: string): void {
        if (!line.trim()) return;
        try {
            const msg = JSON.parse(line) as { id?: number; result?: unknown; error?: { message: string } };
            if (msg.id !== undefined) {
                const pending = conn.pending.get(msg.id);
                if (pending) {
                    clearTimeout(pending.timer);
                    conn.pending.delete(msg.id);
                    if (msg.error) pending.reject(new Error(msg.error.message));
                    else pending.resolve(msg.result ?? {});
                }
            }
        } catch { /* ignore malformed lines */ }
    }

    private rpc(conn: Connection, method: string, params: unknown): Promise<unknown> {
        return new Promise((resolve, reject) => {
            const id = conn.nextId++;
            const timer = setTimeout(() => {
                conn.pending.delete(id);
                reject(new Error(`MCP RPC timeout: ${method}`));
            }, 15000);
            conn.pending.set(id, { resolve, reject, timer });
            const msg = JSON.stringify({ jsonrpc: '2.0', id, method, params });
            conn.process?.stdin?.write(msg + '\n');
        });
    }

    private drainPending(conn: Connection, err: Error): void {
        for (const { reject, timer } of conn.pending.values()) {
            clearTimeout(timer);
            reject(err);
        }
        conn.pending.clear();
    }
}

export const mcpManager = new McpManager();
