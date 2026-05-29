import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { KnowledgePanelCommand } from '../agent-ide-commands';

type DocType = 'pdf' | 'markdown' | 'web' | 'code' | 'json' | 'text';
type KnowledgeTab = 'browse' | 'search' | 'ingest';

interface KnowledgeDoc {
    id: string;
    title: string;
    type: DocType;
    source: string;
    chunks: number;
    tokens: number;
    embedding: string;
    indexedAt: string;
    tags: string[];
}

const DOCS: KnowledgeDoc[] = [
    { id: 'k1', title: 'Eclipse Theia Extension Guide', type: 'pdf', source: 'https://theia-ide.org/docs', chunks: 48, tokens: 38400, embedding: 'text-embedding-3-large', indexedAt: '2026-05-25 09:10', tags: ['theia', 'extensions', 'inversify'] },
    { id: 'k2', title: 'LangGraph Multi-Agent Patterns', type: 'pdf', source: 'https://langchain.com/langgraph', chunks: 62, tokens: 54100, embedding: 'text-embedding-3-large', indexedAt: '2026-05-24 14:22', tags: ['langgraph', 'agents', 'graphs'] },
    { id: 'k3', title: 'AgentBench Evaluation Framework', type: 'pdf', source: 'Liu et al. 2023', chunks: 31, tokens: 26200, embedding: 'text-embedding-3-large', indexedAt: '2026-05-23 11:05', tags: ['benchmark', 'evaluation', 'agents'] },
    { id: 'k4', title: 'REALM-Bench Planning Metrics', type: 'pdf', source: 'Geng & Chang 2025', chunks: 24, tokens: 19800, embedding: 'text-embedding-3-large', indexedAt: '2026-05-22 16:30', tags: ['planning', 'metrics', 'benchmark'] },
    { id: 'k5', title: 'OpenTelemetry Tracing Spec v1.26', type: 'web', source: 'opentelemetry.io', chunks: 19, tokens: 15400, embedding: 'text-embedding-3-large', indexedAt: '2026-05-21 10:00', tags: ['otel', 'tracing', 'spans'] },
    { id: 'k6', title: 'Anthropic Prompt Engineering Guide', type: 'markdown', source: 'docs.anthropic.com', chunks: 37, tokens: 29600, embedding: 'text-embedding-3-large', indexedAt: '2026-05-20 08:45', tags: ['prompting', 'claude', 'anthropic'] },
    { id: 'k7', title: 'Agent Runtime Architecture', type: 'markdown', source: 'docs/ARCHITECTURE.md', chunks: 12, tokens: 9800, embedding: 'text-embedding-3-large', indexedAt: '2026-05-29 07:59', tags: ['architecture', 'runtime', 'internal'] },
    { id: 'k8', title: 'Workspace TypeScript Types', type: 'code', source: 'packages/agent-ide-types/src/index.ts', chunks: 8, tokens: 6200, embedding: 'text-embedding-3-large', indexedAt: '2026-05-29 07:59', tags: ['types', 'typescript', 'internal'] },
];

const TYPE_ICONS: Record<DocType, { icon: string; color: string }> = {
    pdf:      { icon: '📄', color: '#f06040' },
    markdown: { icon: '📝', color: '#7ab4ff' },
    web:      { icon: '🌐', color: '#40c090' },
    code:     { icon: '⚙', color: '#60d060' },
    json:     { icon: '{}', color: '#a0d020' },
    text:     { icon: '▤', color: '#a0a0a0' },
};

const SEARCH_RESULTS = [
    { docId: 'k1', chunk: 3, score: 0.934, snippet: '...ContainerModule binds all widgets and contributions via InversifyJS. Each widget must be bound as a singleton and exported via WidgetFactory...' },
    { docId: 'k7', chunk: 1, score: 0.891, snippet: '...The frontend-module.ts file is the InversifyJS ContainerModule entry point. It wires all Theia contributions — menus, commands, widget factories...' },
    { docId: 'k6', chunk: 12, score: 0.847, snippet: '...When building agentic systems with Claude, decompose tasks into clear sub-steps. Each step should have a defined tool call or reasoning action...' },
];

function TypeBadge({ type }: { type: DocType }) {
    const m = TYPE_ICONS[type];
    return <span style={{ fontSize: 13 }}>{m.icon}</span>;
}

function TagChip({ tag }: { tag: string }) {
    return <span style={{ background: '#1a2a3a', color: '#7ab4ff', padding: '1px 6px', borderRadius: 10, fontSize: 10 }}>{tag}</span>;
}

function DocDetail({ doc }: { doc: KnowledgeDoc }) {
    return (
        <div style={{ width: 300, borderLeft: '1px solid #1e1e1e', display: 'flex', flexDirection: 'column', background: '#0d0d0d', padding: 14, overflow: 'auto' }}>
            <div style={{ fontSize: 13, fontWeight: 700, color: '#e0e0e0', marginBottom: 10 }}>{doc.title}</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 7, fontSize: 11, marginBottom: 12 }}>
                <Row label="Type"><span style={{ color: TYPE_ICONS[doc.type].color }}>{doc.type.toUpperCase()}</span></Row>
                <Row label="Source"><span style={{ color: '#888', wordBreak: 'break-all' }}>{doc.source}</span></Row>
                <Row label="Chunks"><span style={{ color: '#c080ff' }}>{doc.chunks}</span></Row>
                <Row label="Tokens"><span style={{ color: '#d0a030' }}>{doc.tokens.toLocaleString()}</span></Row>
                <Row label="Embedding"><span style={{ color: '#60d060', fontFamily: 'monospace', fontSize: 10 }}>{doc.embedding}</span></Row>
                <Row label="Indexed"><span style={{ color: '#888' }}>{doc.indexedAt}</span></Row>
            </div>
            <div style={{ fontSize: 10, color: '#555', marginBottom: 6 }}>TAGS</div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                {doc.tags.map(t => <TagChip key={t} tag={t} />)}
            </div>
        </div>
    );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
    return (
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 8 }}>
            <span style={{ color: '#555', flexShrink: 0 }}>{label}</span>
            {children}
        </div>
    );
}

function BrowseTab({ onSelect, selected }: { onSelect: (d: KnowledgeDoc | null) => void; selected: KnowledgeDoc | null }) {
    const totalTokens = DOCS.reduce((s, d) => s + d.tokens, 0);
    const totalChunks = DOCS.reduce((s, d) => s + d.chunks, 0);
    return (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            <div style={{ display: 'flex', gap: 16, padding: '8px 12px', borderBottom: '1px solid #1a1a1a', fontSize: 11, color: '#555' }}>
                <span>{DOCS.length} documents</span>
                <span>{totalChunks} chunks</span>
                <span>{(totalTokens / 1000).toFixed(0)}K tokens indexed</span>
            </div>
            <div style={{ flex: 1, overflow: 'auto' }}>
                {DOCS.map(doc => (
                    <div
                        key={doc.id}
                        onClick={() => onSelect(selected?.id === doc.id ? null : doc)}
                        style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 12px', borderBottom: '1px solid #1a1a1a', cursor: 'pointer', background: selected?.id === doc.id ? '#111a2a' : 'transparent' }}
                    >
                        <TypeBadge type={doc.type} />
                        <div style={{ flex: 1, minWidth: 0 }}>
                            <div style={{ fontSize: 12, color: '#d0d0d0', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{doc.title}</div>
                            <div style={{ display: 'flex', gap: 8, marginTop: 2 }}>
                                <span style={{ fontSize: 10, color: '#555' }}>{doc.chunks} chunks</span>
                                <span style={{ fontSize: 10, color: '#555' }}>{doc.tokens.toLocaleString()} tok</span>
                                <span style={{ fontSize: 10, color: '#555' }}>{doc.indexedAt}</span>
                            </div>
                        </div>
                    </div>
                ))}
            </div>
        </div>
    );
}

function SearchTab() {
    const [query, setQuery] = React.useState('');
    const [results, setResults] = React.useState<typeof SEARCH_RESULTS | null>(null);
    const [loading, setLoading] = React.useState(false);

    function search() {
        if (!query.trim()) return;
        setLoading(true);
        setTimeout(() => { setResults(SEARCH_RESULTS); setLoading(false); }, 600);
    }

    return (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', padding: 12, gap: 10, overflow: 'auto' }}>
            <div style={{ display: 'flex', gap: 8 }}>
                <input
                    value={query}
                    onChange={e => setQuery(e.target.value)}
                    onKeyDown={e => e.key === 'Enter' && search()}
                    placeholder="Semantic search across knowledge base…"
                    style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', borderRadius: 4, padding: '7px 10px', fontSize: 12, outline: 'none' }}
                />
                <button onClick={search} disabled={loading}
                    style={{ padding: '7px 14px', background: loading ? '#111' : '#1a2a3a', border: '1px solid #2a4a6a', color: loading ? '#444' : '#7ab4ff', borderRadius: 4, cursor: loading ? 'default' : 'pointer', fontSize: 12 }}>
                    {loading ? '…' : 'Search'}
                </button>
            </div>
            {results && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                    <div style={{ fontSize: 11, color: '#555' }}>{results.length} results for "{query}"</div>
                    {results.map((r, i) => {
                        const doc = DOCS.find(d => d.id === r.docId)!;
                        return (
                            <div key={i} style={{ background: '#111', border: '1px solid #1e1e1e', borderRadius: 6, padding: 12 }}>
                                <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                                    <TypeBadge type={doc.type} />
                                    <span style={{ fontSize: 12, color: '#d0d0d0', fontWeight: 600 }}>{doc.title}</span>
                                    <span style={{ marginLeft: 'auto', fontSize: 11, color: '#60d060', fontFamily: 'monospace' }}>score: {r.score.toFixed(3)}</span>
                                </div>
                                <div style={{ fontSize: 11, color: '#888', lineHeight: 1.5, fontStyle: 'italic' }}>{r.snippet}</div>
                                <div style={{ marginTop: 6, fontSize: 10, color: '#555' }}>chunk #{r.chunk}</div>
                            </div>
                        );
                    })}
                </div>
            )}
            {!results && !loading && (
                <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#444', fontSize: 12 }}>
                    Enter a query and press Search or Enter.
                </div>
            )}
        </div>
    );
}

function IngestTab() {
    const [url, setUrl] = React.useState('');
    const [status, setStatus] = React.useState<string | null>(null);

    function ingest() {
        if (!url.trim()) return;
        setStatus('Fetching and chunking…');
        setTimeout(() => setStatus('Embedding with text-embedding-3-large…'), 1200);
        setTimeout(() => setStatus(`✓ Indexed ${Math.floor(Math.random() * 30 + 10)} chunks (${Math.floor(Math.random() * 8000 + 4000)} tokens)`), 2800);
    }

    return (
        <div style={{ flex: 1, padding: 16, display: 'flex', flexDirection: 'column', gap: 12, overflow: 'auto' }}>
            <div style={{ fontSize: 12, color: '#888', lineHeight: 1.6 }}>
                Ingest a URL, file path, or paste raw text to add it to the knowledge base.
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
                <input
                    value={url}
                    onChange={e => setUrl(e.target.value)}
                    placeholder="https://… or /workspace/path/to/file.md"
                    style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', borderRadius: 4, padding: '7px 10px', fontSize: 12, outline: 'none' }}
                />
                <button onClick={ingest}
                    style={{ padding: '7px 14px', background: '#1a3a1a', border: '1px solid #3a7a3a', color: '#60d060', borderRadius: 4, cursor: 'pointer', fontSize: 12 }}>
                    Ingest
                </button>
            </div>
            {status && (
                <div style={{ fontSize: 12, color: status.startsWith('✓') ? '#60d060' : '#d0a030', fontFamily: 'monospace' }}>{status}</div>
            )}
            <div style={{ marginTop: 8 }}>
                <div style={{ fontSize: 11, color: '#555', marginBottom: 8 }}>EMBEDDING MODEL</div>
                <select style={{ background: '#1a1a1a', border: '1px solid #333', color: '#ddd', borderRadius: 4, padding: '5px 8px', fontSize: 11 }}>
                    <option>text-embedding-3-large (3072 dims)</option>
                    <option>text-embedding-3-small (1536 dims)</option>
                    <option>text-embedding-ada-002 (1536 dims)</option>
                </select>
            </div>
        </div>
    );
}

function KnowledgeView() {
    const [tab, setTab] = React.useState<KnowledgeTab>('browse');
    const [selected, setSelected] = React.useState<KnowledgeDoc | null>(null);

    return (
        <div style={{ display: 'flex', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                    {(['browse', 'search', 'ingest'] as KnowledgeTab[]).map(t => (
                        <button key={t} onClick={() => { setTab(t); setSelected(null); }}
                            style={{ padding: '8px 14px', background: 'none', border: 'none', borderBottom: tab === t ? '2px solid #7ab4ff' : '2px solid transparent', color: tab === t ? '#7ab4ff' : '#666', cursor: 'pointer', fontSize: 12, fontWeight: tab === t ? 700 : 400, textTransform: 'capitalize' }}>
                            {t}
                        </button>
                    ))}
                </div>
                {tab === 'browse' && <BrowseTab onSelect={setSelected} selected={selected} />}
                {tab === 'search' && <SearchTab />}
                {tab === 'ingest' && <IngestTab />}
            </div>
            {selected && tab === 'browse' && <DocDetail doc={selected} />}
        </div>
    );
}

@injectable()
export class KnowledgePanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:knowledge';
    static readonly LABEL = 'Knowledge';

    @postConstruct()
    protected init(): void {
        this.id = KnowledgePanelWidget.ID;
        this.title.label = KnowledgePanelWidget.LABEL;
        this.title.caption = 'Knowledge base browse, search, and ingest';
        this.title.closable = true;
        this.title.iconClass = 'codicon codicon-database';
        this.update();
    }

    protected render(): React.ReactNode {
        return <KnowledgeView />;
    }
}

@injectable()
export class KnowledgePanelContribution extends AbstractViewContribution<KnowledgePanelWidget> {
    constructor() {
        super({ widgetId: KnowledgePanelWidget.ID, widgetName: KnowledgePanelWidget.LABEL, defaultWidgetOptions: { area: 'left' }, toggleCommandId: KnowledgePanelCommand.id });
    }
    registerCommands(commands: CommandRegistry): void {
        commands.registerCommand(KnowledgePanelCommand, { execute: () => this.openView({ activate: true }) });
    }
}
