import * as React from 'react';
import { injectable, postConstruct } from '@theia/core/shared/inversify';
import { ReactWidget } from '@theia/core/lib/browser';
import { ArtifactType } from '@agennext/agent-ide-types';

interface DemoArtifact {
    id: string;
    name: string;
    type: ArtifactType;
    size: string;
    createdAt: string;
    content: string;
}

const DEMO_ARTIFACTS: DemoArtifact[] = [
    {
        id: 'a1', type: 'app', name: 'customer-portal-v2.zip',
        size: '4.2 MB', createdAt: '2025-05-28 14:22',
        content: 'Deployable web application bundle.\nEntrypoint: dist/index.html\nFramework: React 18 + Vite\nRoutes: /dashboard, /agents, /tasks, /settings'
    },
    {
        id: 'a2', type: 'agent', name: 'research-agent.agentdef',
        size: '8.1 KB', createdAt: '2025-05-28 13:55',
        content: JSON.stringify({ id: 'research-agent-01', name: 'ResearchAgent', model: 'claude-opus-4-8', skills: ['web_search', 'summarize'], tools: ['browser', 'pdf_reader'] }, null, 2)
    },
    {
        id: 'a3', type: 'llm', name: 'claude-opus-4-8.modelcard',
        size: '2.3 KB', createdAt: '2025-05-27 09:10',
        content: 'Model: claude-opus-4-8\nProvider: Anthropic\nContext window: 200K tokens\nMax output: 32K tokens\nCapabilities: reasoning, tool_use, vision\nLatency (p50): 1.2s TTFT'
    },
    {
        id: 'a4', type: 'api', name: 'openapi-spec.yaml',
        size: '18.7 KB', createdAt: '2025-05-27 11:30',
        content: 'openapi: "3.1.0"\ninfo:\n  title: Agent Workspace API\n  version: 0.1.0\npaths:\n  /agents:\n    get:\n      summary: List agents\n  /agents/{id}/run:\n    post:\n      summary: Trigger agent run'
    },
    {
        id: 'a5', type: 'package', name: '@agennext/agent-ide-core-0.1.0.tgz',
        size: '312 KB', createdAt: '2025-05-26 17:44',
        content: 'Package: @agennext/agent-ide-core\nVersion: 0.1.0\nFiles: 47\nMain: lib/browser/frontend-module.js\nPeer deps: @theia/core ^1.44.0'
    },
    {
        id: 'a6', type: 'json', name: 'workspace-config.json',
        size: '1.1 KB', createdAt: '2025-05-26 10:05',
        content: JSON.stringify({ workspace: 'agent-ide-dev', agents: 3, defaultModel: 'claude-sonnet-4-6', governance: { mode: 'audit_only' } }, null, 2)
    },
    {
        id: 'a7', type: 'code', name: 'orchestrator.py',
        size: '6.4 KB', createdAt: '2025-05-25 15:20',
        content: 'from typing import List\nimport asyncio\n\nclass Orchestrator:\n    def __init__(self, agents: List[Agent]):\n        self.agents = agents\n\n    async def run(self, task: Task) -> Result:\n        plan = await self.plan(task)\n        return await self.execute(plan)'
    },
    {
        id: 'a8', type: 'data', name: 'evaluation-results.csv',
        size: '24.8 KB', createdAt: '2025-05-25 09:00',
        content: 'run_id,agent,model,duration_ms,tokens,success\nrun-001,ResearchAgent,claude-opus-4-8,3420,8721,true\nrun-002,CoderAgent,claude-sonnet-4-6,1980,4312,true\nrun-003,ResearchAgent,gpt-4o,5100,9840,false'
    },
    {
        id: 'a9', type: 'model', name: 'task-classifier.onnx',
        size: '88.2 MB', createdAt: '2025-05-24 12:33',
        content: 'ONNX Model Artifact\nArchitecture: BERT-base-uncased fine-tuned\nTask: Multi-label task classification\nClasses: 12\nAccuracy: 0.924\nInput: tokenized text (max 512 tokens)'
    },
    {
        id: 'a10', type: 'document', name: 'architecture-decisions.md',
        size: '14.5 KB', createdAt: '2025-05-23 16:11',
        content: '# Architecture Decisions\n\n## ADR-001: Eclipse Theia as IDE shell\n**Status:** Accepted\n**Context:** Need a browser-based IDE foundation with extension model.\n**Decision:** Use Theia 1.44.0 with InversifyJS DI.\n\n## ADR-002: Dummy simulation layer\n**Status:** Accepted\n**Context:** No live LLM runtime yet.\n**Decision:** Token approximation via length/3.8 heuristic.'
    },
    {
        id: 'a11', type: 'report', name: 'performance-evaluation-2025-05.pdf',
        size: '2.1 MB', createdAt: '2025-05-22 11:00',
        content: 'Performance Evaluation Report — May 2025\n\nExecutive Summary:\n  - 3 agents evaluated across 120 test runs\n  - Average task completion rate: 87.3%\n  - Average RTT: 142ms (p50), 380ms (p99)\n  - Tool execution success: 92.1%\n  - Planning efficiency score: 0.78\n\nFrameworks tested: LangGraph, AutoGen, CrewAI'
    }
];

const TYPE_COLORS: Record<ArtifactType, { bg: string; fg: string; label: string }> = {
    app:      { bg: '#1a3a5c', fg: '#60b0ff', label: 'APP' },
    agent:    { bg: '#2a1a5c', fg: '#a070ff', label: 'AGENT' },
    llm:      { bg: '#1a2a5c', fg: '#70c0ff', label: 'LLM' },
    api:      { bg: '#0a3a2a', fg: '#40d090', label: 'API' },
    package:  { bg: '#3a2a0a', fg: '#f0a030', label: 'PKG' },
    json:     { bg: '#2a3a0a', fg: '#a0d020', label: 'JSON' },
    code:     { bg: '#1a3a1a', fg: '#60d060', label: 'CODE' },
    data:     { bg: '#3a1a0a', fg: '#f06040', label: 'DATA' },
    model:    { bg: '#1a1a3a', fg: '#8080ff', label: 'MODEL' },
    document: { bg: '#3a3a1a', fg: '#d0c040', label: 'DOC' },
    report:   { bg: '#2a1a1a', fg: '#d06060', label: 'REPORT' },
    file:     { bg: '#2a2a2a', fg: '#a0a0a0', label: 'FILE' },
};

const ALL_TYPES: ArtifactType[] = ['app','agent','llm','api','package','json','code','data','model','document','report','file'];

function TypeBadge({ type }: { type: ArtifactType }) {
    const c = TYPE_COLORS[type];
    return (
        <span style={{ background: c.bg, color: c.fg, padding: '1px 6px', borderRadius: 3, fontSize: 10, fontWeight: 700, fontFamily: 'monospace' }}>
            {c.label}
        </span>
    );
}

function ContentViewer({ artifact }: { artifact: DemoArtifact }) {
    const isJson = artifact.type === 'json' || artifact.type === 'agent';
    return (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            <div style={{ padding: '8px 12px', borderBottom: '1px solid #2a2a2a', display: 'flex', alignItems: 'center', gap: 8 }}>
                <TypeBadge type={artifact.type} />
                <span style={{ fontWeight: 600, fontSize: 13 }}>{artifact.name}</span>
                <span style={{ color: '#666', fontSize: 11, marginLeft: 'auto' }}>{artifact.size} · {artifact.createdAt}</span>
            </div>
            <pre style={{
                flex: 1, overflow: 'auto', margin: 0, padding: 16,
                background: '#0d0d0d', color: isJson ? '#a0d080' : '#c8c8c8',
                fontSize: 12, lineHeight: 1.6, fontFamily: 'monospace', whiteSpace: 'pre-wrap'
            }}>
                {artifact.content}
            </pre>
        </div>
    );
}

function ArtifactsList({
    artifacts, selected, onSelect
}: {
    artifacts: DemoArtifact[];
    selected: string | null;
    onSelect: (id: string) => void;
}) {
    return (
        <div style={{ flex: 1, overflow: 'auto' }}>
            {artifacts.length === 0 && (
                <div style={{ padding: 24, color: '#555', textAlign: 'center', fontSize: 12 }}>No artifacts for this type.</div>
            )}
            {artifacts.map(a => (
                <div
                    key={a.id}
                    onClick={() => onSelect(a.id)}
                    style={{
                        display: 'flex', alignItems: 'center', gap: 8, padding: '7px 10px',
                        cursor: 'pointer', borderBottom: '1px solid #1e1e1e',
                        background: selected === a.id ? '#1a2a3a' : 'transparent'
                    }}
                >
                    <TypeBadge type={a.type} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                        <div style={{ fontSize: 12, fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{a.name}</div>
                        <div style={{ fontSize: 10, color: '#666' }}>{a.size} · {a.createdAt}</div>
                    </div>
                </div>
            ))}
        </div>
    );
}

function ArtifactsView() {
    const [filter, setFilter] = React.useState<ArtifactType | 'all'>('all');
    const [selected, setSelected] = React.useState<string | null>(DEMO_ARTIFACTS[0].id);

    const filtered = filter === 'all' ? DEMO_ARTIFACTS : DEMO_ARTIFACTS.filter(a => a.type === filter);
    const selectedArtifact = DEMO_ARTIFACTS.find(a => a.id === selected) ?? null;

    return (
        <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#111', color: '#ccc', fontFamily: 'var(--theia-ui-font-family, sans-serif)' }}>
            <div style={{ padding: '10px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <span style={{ fontWeight: 700, fontSize: 13 }}>Artifacts</span>
                <span style={{ color: '#555', fontSize: 11 }}>{DEMO_ARTIFACTS.length} total</span>
            </div>
            <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
                {/* Type filter sidebar */}
                <div style={{ width: 90, borderRight: '1px solid #1e1e1e', overflow: 'auto', paddingTop: 4 }}>
                    <div
                        onClick={() => setFilter('all')}
                        style={{ padding: '5px 8px', cursor: 'pointer', fontSize: 11, background: filter === 'all' ? '#1a2a3a' : 'transparent', color: filter === 'all' ? '#60b0ff' : '#888', fontWeight: filter === 'all' ? 700 : 400 }}
                    >
                        All ({DEMO_ARTIFACTS.length})
                    </div>
                    {ALL_TYPES.map(t => {
                        const count = DEMO_ARTIFACTS.filter(a => a.type === t).length;
                        if (count === 0) return null;
                        const c = TYPE_COLORS[t];
                        return (
                            <div
                                key={t}
                                onClick={() => setFilter(t)}
                                style={{
                                    padding: '5px 8px', cursor: 'pointer', fontSize: 11,
                                    background: filter === t ? '#1a2a3a' : 'transparent',
                                    color: filter === t ? c.fg : '#777',
                                    fontWeight: filter === t ? 700 : 400
                                }}
                            >
                                {c.label} ({count})
                            </div>
                        );
                    })}
                </div>
                {/* List + viewer */}
                <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                    <div style={{ flex: '0 0 200px', borderBottom: '1px solid #1e1e1e', overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                        <ArtifactsList artifacts={filtered} selected={selected} onSelect={setSelected} />
                    </div>
                    {selectedArtifact && <ContentViewer artifact={selectedArtifact} />}
                </div>
            </div>
        </div>
    );
}

@injectable()
export class ArtifactsPanelWidget extends ReactWidget {
    static readonly ID = 'agent-ide:artifacts';
    static readonly LABEL = 'Artifacts';

    @postConstruct()
    protected init(): void {
        this.id = ArtifactsPanelWidget.ID;
        this.title.label = ArtifactsPanelWidget.LABEL;
        this.title.caption = ArtifactsPanelWidget.LABEL;
        this.title.closable = true;
        this.title.iconClass = 'fa fa-archive';
        this.update();
    }

    protected render(): React.ReactNode {
        return <ArtifactsView />;
    }
}
