import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget, AbstractViewContribution } from '@theia/core/lib/browser';
import { CommandRegistry } from '@theia/core/lib/common';
import { KnowledgePanelCommand } from '../agent-ide-commands';
import {
    isBackendReachable, listKnowledge, searchKnowledge, ingestText, ingestUrl,
    deleteKnowledgeChunk, KnowledgeChunkSummary, KnowledgeSearchResult,
} from '../runtime/backend-client';
import { getSession } from '../runtime/session-store';

type KnowledgeTab = 'browse' | 'search' | 'ingest';

// ─── Source icon ──────────────────────────────────────────────────────────────

function SourceIcon({ source }: { source: string }) {
    let icon = '▤';
    let color = '#a0a0a0';
    if (source.startsWith('http')) { icon = '🌐'; color = '#40c090'; }
    else if (source.startsWith('run:')) { icon = '⚡'; color = '#d0a030'; }
    else if (source.endsWith('.md')) { icon = '📝'; color = '#7ab4ff'; }
    else if (source.endsWith('.ts') || source.endsWith('.js')) { icon = '⚙'; color = '#60d060'; }
    return <span title={source} style={{ fontSize: 13, color }}>{icon}</span>;
}

// ─── Browse tab ───────────────────────────────────────────────────────────────

function BrowseTab({ chunks, liveBackend, onDelete }: {
    chunks: KnowledgeChunkSummary[];
    liveBackend: boolean;
    onDelete: (id: string) => void;
}) {
    const [selected, setSelected] = React.useState<KnowledgeChunkSummary | null>(null);

    return (
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
            {/* List */}
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                <div style={{ display: 'flex', gap: 16, padding: '8px 12px', borderBottom: '1px solid #1a1a1a', fontSize: 11, color: '#555', background: '#0d0d0d' }}>
                    <span>{chunks.length} chunk{chunks.length !== 1 ? 's' : ''}</span>
                    {!liveBackend && <span style={{ color: '#444' }}>○ demo data</span>}
                    {liveBackend && <span style={{ color: '#40a040' }}>● live backend</span>}
                </div>
                <div style={{ flex: 1, overflow: 'auto' }}>
                    {chunks.length === 0 && (
                        <div style={{ padding: 24, textAlign: 'center', color: '#444', fontSize: 12 }}>
                            No knowledge chunks yet. Use the Ingest tab to add content.
                        </div>
                    )}
                    {chunks.map(c => (
                        <div key={c.id}
                            onClick={() => setSelected(selected?.id === c.id ? null : c)}
                            style={{ display: 'flex', alignItems: 'flex-start', gap: 10, padding: '8px 12px', borderBottom: '1px solid #1a1a1a', cursor: 'pointer', background: selected?.id === c.id ? '#111a2a' : 'transparent' }}>
                            <SourceIcon source={c.source} />
                            <div style={{ flex: 1, minWidth: 0 }}>
                                <div style={{ fontSize: 12, color: '#d0d0d0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.title}</div>
                                <div style={{ fontSize: 10, color: '#555', marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                                    {c.contentPreview.slice(0, 80)}…
                                </div>
                                <div style={{ fontSize: 10, color: '#444', marginTop: 2 }}>{new Date(c.createdAt).toLocaleString()}</div>
                            </div>
                            {liveBackend && (
                                <button onClick={e => { e.stopPropagation(); onDelete(c.id); }}
                                    title="Delete chunk"
                                    style={{ background: 'none', border: 'none', color: '#444', cursor: 'pointer', fontSize: 12, padding: '0 4px', flexShrink: 0 }}>
                                    ✕
                                </button>
                            )}
                        </div>
                    ))}
                </div>
            </div>
            {/* Detail pane */}
            {selected && (
                <div style={{ width: 280, borderLeft: '1px solid #1e1e1e', background: '#0d0d0d', padding: 14, overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 10 }}>
                    <div style={{ fontSize: 13, fontWeight: 700, color: '#e0e0e0' }}>{selected.title}</div>
                    <div style={{ fontSize: 10, color: '#555' }}>SOURCE</div>
                    <div style={{ fontSize: 10, color: '#888', fontFamily: 'monospace', wordBreak: 'break-all' }}>{selected.source}</div>
                    <div style={{ fontSize: 10, color: '#555' }}>ID</div>
                    <div style={{ fontSize: 10, color: '#666', fontFamily: 'monospace' }}>{selected.id}</div>
                    <div style={{ fontSize: 10, color: '#555' }}>PREVIEW</div>
                    <div style={{ fontSize: 11, color: '#aaa', lineHeight: 1.5, background: '#111', padding: 8, borderRadius: 4 }}>
                        {selected.contentPreview}
                    </div>
                </div>
            )}
        </div>
    );
}

// ─── Search tab ───────────────────────────────────────────────────────────────

function SearchTab({ liveBackend, token }: { liveBackend: boolean; token: string | null }) {
    const [query, setQuery]     = React.useState('');
    const [results, setResults] = React.useState<KnowledgeSearchResult[] | null>(null);
    const [loading, setLoading] = React.useState(false);
    const [err, setErr]         = React.useState('');

    async function search() {
        if (!query.trim()) return;
        setLoading(true); setErr(''); setResults(null);
        try {
            if (liveBackend) {
                const r = await searchKnowledge(query, 8, token ?? undefined);
                setResults(r);
            } else {
                // Demo fallback
                await new Promise(r => setTimeout(r, 600));
                setResults([
                    { score: 0.934, id: 'demo1', title: 'Eclipse Theia Extension Guide', source: 'https://theia-ide.org/docs', contentPreview: '…ContainerModule binds all widgets and contributions via InversifyJS. Each widget must be a singleton exported via WidgetFactory…', createdAt: new Date().toISOString() },
                    { score: 0.847, id: 'demo2', title: 'Anthropic Prompt Engineering', source: 'https://docs.anthropic.com', contentPreview: '…When building agentic systems, decompose tasks into clear sub-steps. Each step should have a defined tool call or reasoning action…', createdAt: new Date().toISOString() },
                ]);
            }
        } catch (ex) { setErr(String(ex)); }
        setLoading(false);
    }

    return (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', padding: 12, gap: 10, overflow: 'auto' }}>
            <div style={{ display: 'flex', gap: 8 }}>
                <input value={query} onChange={e => setQuery(e.target.value)} onKeyDown={e => e.key === 'Enter' && search()}
                    placeholder="Semantic search across knowledge base…"
                    style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', color: '#ddd', borderRadius: 4, padding: '7px 10px', fontSize: 12, outline: 'none' }} />
                <button onClick={search} disabled={loading}
                    style={{ padding: '7px 14px', background: loading ? '#111' : '#1a2a3a', border: '1px solid #2a4a6a', color: loading ? '#444' : '#7ab4ff', borderRadius: 4, cursor: loading ? 'default' : 'pointer', fontSize: 12 }}>
                    {loading ? '…' : 'Search'}
                </button>
            </div>
            {err && <div style={{ fontSize: 11, color: '#c04040' }}>{err}</div>}
            {results !== null && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                    <div style={{ fontSize: 11, color: '#555' }}>{results.length} result{results.length !== 1 ? 's' : ''} for "{query}"</div>
                    {results.length === 0 && <div style={{ fontSize: 12, color: '#444' }}>No matching chunks found.</div>}
                    {results.map((r, i) => (
                        <div key={r.id ?? i} style={{ background: '#111', border: '1px solid #1e1e1e', borderRadius: 6, padding: 12 }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                                <SourceIcon source={r.source} />
                                <span style={{ fontSize: 12, color: '#d0d0d0', fontWeight: 600, flex: 1 }}>{r.title}</span>
                                <span style={{ fontSize: 11, color: '#60d060', fontFamily: 'monospace' }}>{r.score.toFixed(3)}</span>
                            </div>
                            <div style={{ fontSize: 11, color: '#888', lineHeight: 1.5, fontStyle: 'italic' }}>{r.contentPreview}</div>
                        </div>
                    ))}
                </div>
            )}
            {results === null && !loading && (
                <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#444', fontSize: 12 }}>
                    Enter a query and press Search or Enter.
                </div>
            )}
        </div>
    );
}

// ─── Ingest tab ───────────────────────────────────────────────────────────────

function IngestTab({ liveBackend, token, onIngested }: { liveBackend: boolean; token: string | null; onIngested: () => void }) {
    const [mode, setMode]       = React.useState<'text' | 'url'>('text');
    const [title, setTitle]     = React.useState('');
    const [content, setContent] = React.useState('');
    const [url, setUrl]         = React.useState('');
    const [status, setStatus]   = React.useState('');
    const [loading, setLoading] = React.useState(false);

    async function handleIngest(e: React.FormEvent) {
        e.preventDefault();
        setLoading(true); setStatus('');
        try {
            if (!liveBackend) {
                await new Promise(r => setTimeout(r, 1000));
                setStatus(`✓ Demo: would ingest "${mode === 'url' ? url : title}" — start backend for real ingestion.`);
                setLoading(false); return;
            }
            if (mode === 'url') {
                setStatus('Fetching URL…');
                const r = await ingestUrl(url, token ?? undefined);
                setStatus(`✓ Indexed ${r.chunks} chunk${r.chunks !== 1 ? 's' : ''} from URL`);
                setUrl('');
            } else {
                setStatus('Chunking and embedding…');
                const r = await ingestText(title || 'Untitled', content, token ?? undefined);
                setStatus(`✓ Indexed ${r.chunks} chunk${r.chunks !== 1 ? 's' : ''}`);
                setTitle(''); setContent('');
            }
            onIngested();
        } catch (ex) { setStatus(`Error: ${String(ex)}`); }
        setLoading(false);
    }

    const inputStyle: React.CSSProperties = { width: '100%', boxSizing: 'border-box', background: '#1a1a1a', border: '1px solid #333', color: '#ddd', borderRadius: 4, padding: '7px 10px', fontSize: 12, outline: 'none' };

    return (
        <div style={{ flex: 1, padding: 16, overflow: 'auto' }}>
            <div style={{ display: 'flex', gap: 8, marginBottom: 14 }}>
                {(['text', 'url'] as const).map(m => (
                    <button key={m} onClick={() => setMode(m)}
                        style={{ padding: '5px 14px', background: mode === m ? '#1a2a3a' : 'none', border: `1px solid ${mode === m ? '#2a5a8a' : '#2a2a2a'}`, color: mode === m ? '#7ab4ff' : '#666', borderRadius: 4, cursor: 'pointer', fontSize: 11, fontWeight: mode === m ? 700 : 400 }}>
                        {m === 'text' ? 'Paste text' : 'From URL'}
                    </button>
                ))}
            </div>
            <form onSubmit={handleIngest} style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                {mode === 'text' ? (
                    <>
                        <div>
                            <div style={{ fontSize: 10, color: '#777', marginBottom: 4 }}>Title</div>
                            <input value={title} onChange={e => setTitle(e.target.value)} placeholder="Document title…" style={inputStyle} />
                        </div>
                        <div>
                            <div style={{ fontSize: 10, color: '#777', marginBottom: 4 }}>Content</div>
                            <textarea value={content} onChange={e => setContent(e.target.value)} placeholder="Paste text to add to the knowledge base…" rows={10}
                                style={{ ...inputStyle, resize: 'vertical', lineHeight: 1.5 }} />
                        </div>
                    </>
                ) : (
                    <div>
                        <div style={{ fontSize: 10, color: '#777', marginBottom: 4 }}>URL</div>
                        <input value={url} onChange={e => setUrl(e.target.value)} placeholder="https://…" style={inputStyle} />
                    </div>
                )}
                <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
                    <button type="submit" disabled={loading || (mode === 'text' ? !content.trim() : !url.trim())}
                        style={{ padding: '7px 16px', background: '#1a3a1a', border: '1px solid #3a7a3a', color: loading ? '#555' : '#60d060', borderRadius: 4, cursor: loading ? 'default' : 'pointer', fontSize: 12, fontWeight: 700 }}>
                        {loading ? 'Ingesting…' : 'Ingest'}
                    </button>
                    <span style={{ fontSize: 11, color: status.startsWith('✓') ? '#60d060' : status.startsWith('Error') ? '#c04040' : '#d0a030' }}>{status}</span>
                </div>
            </form>
            <div style={{ marginTop: 20, padding: 12, background: '#0d0d0d', border: '1px solid #1a1a1a', borderRadius: 4, fontSize: 11, color: '#555', lineHeight: 1.6 }}>
                <div style={{ fontWeight: 700, color: '#666', marginBottom: 4 }}>Embedding</div>
                {liveBackend && process.env.NODE_ENV !== 'test'
                    ? '● OpenAI text-embedding-3-small (if OPENAI_API_KEY set) · fallback: 128-dim n-gram hash'
                    : '○ n-gram hash embedding (128 dims) — set OPENAI_API_KEY for real embeddings'}
            </div>
        </div>
    );
}

// ─── Root view ────────────────────────────────────────────────────────────────

function KnowledgeView() {
    const [tab, setTab]           = React.useState<KnowledgeTab>('browse');
    const [liveBackend, setLive]  = React.useState(false);
    const [chunks, setChunks]     = React.useState<KnowledgeChunkSummary[]>([]);
    const [loadingChunks, setLc]  = React.useState(true);
    const token                   = getSession()?.token ?? null;

    async function loadChunks() {
        if (!liveBackend) { setLc(false); return; }
        try { setChunks(await listKnowledge(token ?? undefined)); } catch { /* ignore */ }
        setLc(false);
    }

    React.useEffect(() => {
        isBackendReachable().then(r => {
            setLive(r);
            if (r) listKnowledge(token ?? undefined).then(setChunks).catch(() => {}).finally(() => setLc(false));
            else setLc(false);
        });
    }, []);

    async function handleDelete(id: string) {
        await deleteKnowledgeChunk(id, token ?? undefined);
        setChunks(prev => prev.filter(c => c.id !== id));
    }

    return (
        <div style={{ display: 'flex', height: '100%', background: '#0a0a0a', color: '#d0d0d0', fontFamily: 'var(--theia-font-family, monospace)' }}>
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                <div style={{ display: 'flex', borderBottom: '1px solid #1e1e1e', background: '#0f0f0f' }}>
                    {(['browse', 'search', 'ingest'] as KnowledgeTab[]).map(t => (
                        <button key={t} onClick={() => setTab(t)}
                            style={{ padding: '8px 14px', background: 'none', border: 'none', borderBottom: tab === t ? '2px solid #7ab4ff' : '2px solid transparent', color: tab === t ? '#7ab4ff' : '#666', cursor: 'pointer', fontSize: 12, fontWeight: tab === t ? 700 : 400, textTransform: 'capitalize' }}>
                            {t === 'browse' ? `Browse${loadingChunks ? '' : ` (${chunks.length})`}` : t === 'search' ? 'Search' : 'Ingest'}
                        </button>
                    ))}
                </div>
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                    {tab === 'browse' && <BrowseTab chunks={chunks} liveBackend={liveBackend} onDelete={handleDelete} />}
                    {tab === 'search' && <SearchTab liveBackend={liveBackend} token={token} />}
                    {tab === 'ingest' && <IngestTab liveBackend={liveBackend} token={token} onIngested={loadChunks} />}
                </div>
            </div>
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
