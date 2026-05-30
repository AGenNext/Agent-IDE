import * as crypto from 'crypto';
import * as fs from 'fs/promises';
import * as path from 'path';

export interface KnowledgeChunk {
    id:        string;
    tenantId:  string;
    title:     string;
    content:   string;
    source:    string;
    embedding: number[];
    createdAt: string;
    metadata:  Record<string, unknown>;
}

export interface SearchResult {
    chunk: KnowledgeChunk;
    score: number;
}

const STORE_PATH = path.join(process.env.WORKSPACE_ROOT ?? process.cwd(), '.knowledge.json');
const EMBED_DIMS = 128;

// ─── Pseudo-embedding (n-gram hashing, no deps) ───────────────────────────────

function ngrams(text: string, n: number): string[] {
    const tokens = text.toLowerCase().replace(/[^a-z0-9 ]/g, ' ').split(/\s+/).filter(Boolean);
    const out: string[] = [];
    for (let i = 0; i <= tokens.length - n; i++) out.push(tokens.slice(i, i + n).join(' '));
    return out;
}

function hashEmbed(text: string): number[] {
    const vec = new Array<number>(EMBED_DIMS).fill(0);
    const grams = [...ngrams(text, 1), ...ngrams(text, 2)];
    for (const g of grams) {
        const h = crypto.createHash('md5').update(g).digest();
        const idx = h.readUInt32LE(0) % EMBED_DIMS;
        const sign = (h.readUInt32LE(4) & 1) ? 1 : -1;
        vec[idx] += sign;
    }
    const norm = Math.sqrt(vec.reduce((s, v) => s + v * v, 0)) || 1;
    return vec.map(v => v / norm);
}

async function openaiEmbed(text: string): Promise<number[]> {
    const resp = await (globalThis.fetch as typeof fetch)('https://api.openai.com/v1/embeddings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${process.env.OPENAI_API_KEY}` },
        body: JSON.stringify({ input: text.slice(0, 8000), model: 'text-embedding-3-small' }),
    });
    if (!resp.ok) throw new Error(`OpenAI embedding failed: ${resp.status}`);
    const data = await resp.json() as { data: [{ embedding: number[] }] };
    return data.data[0].embedding;
}

export async function embed(text: string): Promise<number[]> {
    if (process.env.OPENAI_API_KEY) {
        try { return await openaiEmbed(text); } catch { /* fall through */ }
    }
    return hashEmbed(text);
}

// ─── Cosine similarity ────────────────────────────────────────────────────────

function cosine(a: number[], b: number[]): number {
    let dot = 0, na = 0, nb = 0;
    const len = Math.min(a.length, b.length);
    for (let i = 0; i < len; i++) { dot += a[i] * b[i]; na += a[i] * a[i]; nb += b[i] * b[i]; }
    return dot / (Math.sqrt(na) * Math.sqrt(nb) || 1);
}

// ─── Chunking ─────────────────────────────────────────────────────────────────

export function chunkText(text: string, size = 400, overlap = 50): string[] {
    const words = text.split(/\s+/);
    const chunks: string[] = [];
    let start = 0;
    while (start < words.length) {
        chunks.push(words.slice(start, start + size).join(' '));
        start += size - overlap;
    }
    return chunks.filter(c => c.trim().length > 0);
}

// ─── Knowledge store ──────────────────────────────────────────────────────────

class KnowledgeStore {
    private chunks = new Map<string, KnowledgeChunk>();
    private loaded = false;

    async load(): Promise<void> {
        try {
            const raw = await fs.readFile(STORE_PATH, 'utf-8');
            const list = JSON.parse(raw) as KnowledgeChunk[];
            for (const c of list) this.chunks.set(c.id, c);
        } catch { /* no file yet */ }
        this.loaded = true;
    }

    private async persist(): Promise<void> {
        if (!this.loaded) return;
        await fs.mkdir(path.dirname(STORE_PATH), { recursive: true });
        await fs.writeFile(STORE_PATH, JSON.stringify([...this.chunks.values()], null, 2));
    }

    async upsert(chunk: Omit<KnowledgeChunk, 'id' | 'embedding' | 'createdAt'>): Promise<KnowledgeChunk> {
        const id = `kc_${crypto.randomBytes(8).toString('hex')}`;
        const embedding = await embed(chunk.content);
        const record: KnowledgeChunk = { ...chunk, id, embedding, createdAt: new Date().toISOString() };
        this.chunks.set(id, record);
        await this.persist();
        return record;
    }

    async search(tenantId: string, query: string, topK = 5): Promise<SearchResult[]> {
        const qvec = await embed(query);
        const candidates = [...this.chunks.values()].filter(c => c.tenantId === tenantId || tenantId === '*');
        return candidates
            .map(chunk => ({ chunk, score: cosine(qvec, chunk.embedding) }))
            .sort((a, b) => b.score - a.score)
            .slice(0, topK);
    }

    list(tenantId: string): KnowledgeChunk[] {
        return [...this.chunks.values()]
            .filter(c => c.tenantId === tenantId || tenantId === '*')
            .sort((a, b) => b.createdAt.localeCompare(a.createdAt));
    }

    get(id: string): KnowledgeChunk | undefined {
        return this.chunks.get(id);
    }

    async delete(id: string): Promise<boolean> {
        if (!this.chunks.has(id)) return false;
        this.chunks.delete(id);
        await this.persist();
        return true;
    }

    count(tenantId?: string): number {
        if (!tenantId) return this.chunks.size;
        return [...this.chunks.values()].filter(c => c.tenantId === tenantId).length;
    }
}

export const knowledgeStore = new KnowledgeStore();

// ─── Ingest helpers ───────────────────────────────────────────────────────────

export async function ingestText(
    tenantId: string,
    title: string,
    text: string,
    source = 'manual',
    metadata: Record<string, unknown> = {},
): Promise<KnowledgeChunk[]> {
    const chunks = chunkText(text);
    const records: KnowledgeChunk[] = [];
    for (let i = 0; i < chunks.length; i++) {
        const record = await knowledgeStore.upsert({
            tenantId, title: chunks.length > 1 ? `${title} [${i + 1}/${chunks.length}]` : title,
            content: chunks[i], source, metadata,
        });
        records.push(record);
    }
    return records;
}

export async function ingestUrl(tenantId: string, url: string): Promise<KnowledgeChunk[]> {
    const resp = await (globalThis.fetch as typeof fetch)(url, { signal: AbortSignal.timeout(10000) });
    if (!resp.ok) throw new Error(`Fetch failed: ${resp.status}`);
    const html = await resp.text();
    const text = html
        .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, ' ')
        .replace(/<style[^>]*>[\s\S]*?<\/style>/gi, ' ')
        .replace(/<[^>]+>/g, ' ')
        .replace(/\s+/g, ' ')
        .trim();
    const title = (html.match(/<title[^>]*>([^<]+)<\/title>/i)?.[1] ?? url).trim();
    return ingestText(tenantId, title, text, url);
}
