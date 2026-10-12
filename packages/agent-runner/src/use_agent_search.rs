// USEagent CodeRetrieval — deterministic code search over the request's
// workspace (term-weighted file ranking + definition-line lookup) instead of
// asking the model to guess code locations. Ported from #13.
//
// Uses the same workspace confinement as ExecuteTests (USE_AGENT_WORKSPACE_ROOT,
// see use_agent_exec::resolve_workspace). Reads files only; never executes.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_TERMS: usize = 12;

const SKIP_DIRS: &[&str] = &[
    "target", "node_modules", "dist", "build", "vendor", "venv", "__pycache__", "out", "coverage",
];

const CODE_EXTS: &[&str] = &[
    "rs", "py", "ts", "tsx", "js", "jsx", "mjs", "cjs", "go", "java", "kt", "c", "h", "cc", "cpp",
    "hpp", "cs", "rb", "php", "swift", "scala", "sh", "toml", "yaml", "yml",
];

const DEF_PREFIXES: &[&str] = &[
    "fn ", "pub fn ", "pub(crate) fn ", "async fn ", "pub async fn ", "struct ", "pub struct ",
    "enum ", "pub enum ", "trait ", "pub trait ", "impl ", "mod ", "pub mod ", "def ", "async def ",
    "class ", "export class ", "function ", "export function ", "export async function ",
    "async function ", "export const ", "interface ", "export interface ", "type ", "export type ",
    "func ",
];

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "when", "then", "than", "should",
    "would", "could", "does", "doesn", "have", "has", "not", "are", "was", "were", "will", "make",
    "fix", "bug", "issue", "error", "code", "test", "tests", "function", "method", "file", "files",
    "new", "use", "using", "returns", "return", "wrong", "correct", "instead", "value",
    "values", "some", "there", "their", "which", "what", "where", "about", "also", "only",
];



// ── CodeRetrieval: code search ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeHit {
    pub path: String,
    pub line: usize,
    pub text: String,
    pub score: f64,
    pub is_definition: bool,
}

impl CodeHit {
    /// grep-style location: `path:line: text`
    pub fn location(&self) -> String {
        format!("{}:{}: {}", self.path, self.line, self.text)
    }
}

/// Pull identifier-like search terms out of free text.
pub fn extract_terms(text: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for raw in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if raw.is_empty() || raw.chars().all(|c| c.is_ascii_digit()) { continue; }
        let lower = raw.to_lowercase();
        let identifier_like = raw.contains('_')
            || raw.chars().skip(1).any(|c| c.is_uppercase());
        if !identifier_like && (lower.len() < 3 || STOPWORDS.contains(&lower.as_str())) { continue; }
        if !terms.contains(&lower) { terms.push(lower); }
        if terms.len() >= MAX_TERMS { break; }
    }
    terms
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= MAX_FILES { return; }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() { continue; }
        let path = entry.path();
        if ft.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) { continue; }
            walk(root, &path, out);
        } else if ft.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !CODE_EXTS.contains(&ext) { continue; }
            if entry.metadata().map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(true) { continue; }
            out.push(path);
        }
    }
}

fn is_test_path(rel: &str) -> bool {
    let l = rel.to_lowercase();
    l.split('/').any(|seg| {
        seg == "tests" || seg == "test" || seg == "__tests__" || seg == "spec"
            || seg.starts_with("test_") || seg.ends_with("_test.py") || seg.ends_with("_test.go")
            || seg.contains(".test.") || seg.contains(".spec.") || seg.ends_with("_tests.rs")
    })
}

fn is_definition(line: &str) -> bool {
    let t = line.trim_start();
    DEF_PREFIXES.iter().any(|p| t.starts_with(p))
}

/// Rank files in the workspace against the query and return the best line per file.
/// Blocking: call from spawn_blocking.
pub fn search_code(root: &Path, query: &str, limit: usize) -> Vec<CodeHit> {
    let terms = extract_terms(query);
    if terms.is_empty() { return Vec::new(); }

    let mut files = Vec::new();
    walk(root, root, &mut files);

    struct FileMatch { rel: String, counts: Vec<usize>, path_hits: Vec<bool>, lines: Vec<(usize, String, usize, bool)> }
    let mut matches: Vec<FileMatch> = Vec::new();
    let mut doc_freq = vec![0usize; terms.len()];

    for path in &files {
        let Ok(content) = std::fs::read_to_string(path) else { continue };
        let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let rel_l = rel.to_lowercase();
        let mut counts = vec![0usize; terms.len()];
        let mut lines = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let ll = line.to_lowercase();
            let mut hit_terms = 0;
            for (ti, t) in terms.iter().enumerate() {
                let n = ll.matches(t.as_str()).count();
                if n > 0 { counts[ti] += n; hit_terms += 1; }
            }
            if hit_terms > 0 {
                lines.push((i + 1, line.trim().chars().take(160).collect(), hit_terms, is_definition(line)));
            }
        }
        let path_hits: Vec<bool> = terms.iter().map(|t| rel_l.contains(t.as_str())).collect();
        // A directory-name hit alone (e.g. every file under calc/) is not a match.
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if lines.is_empty() && !terms.iter().any(|t| stem.contains(t.as_str())) { continue; }
        for (ti, &c) in counts.iter().enumerate() {
            if c > 0 || path_hits[ti] { doc_freq[ti] += 1; }
        }
        matches.push(FileMatch { rel, counts, path_hits, lines });
    }

    let n_docs = files.len().max(1) as f64;
    let idf: Vec<f64> = doc_freq.iter().map(|&df| (1.0 + n_docs / (df.max(1) as f64)).ln()).collect();

    let mut hits: Vec<CodeHit> = matches.into_iter().map(|m| {
        let mut score = 0.0;
        for ti in 0..terms.len() {
            score += (m.counts[ti].min(5) as f64 / 5.0) * idf[ti];
            if m.path_hits[ti] { score += idf[ti]; }
        }
        // Best line: definitions first, then lines matching the most terms.
        let best = m.lines.iter()
            .max_by_key(|(line, _, hits, def)| (*def as usize * 100 + hits, usize::MAX - line));
        let (line, text, def) = best
            .map(|(l, t, _, d)| (*l, t.clone(), *d))
            .unwrap_or((1, String::new(), false));
        if def { score *= 1.25; }
        // Lc is code under change; tests belong to TestRetrieval (Lt).
        if is_test_path(&m.rel) { score *= 0.5; }
        CodeHit { path: m.rel, line, text, score: (score * 1000.0).round() / 1000.0, is_definition: def }
    }).collect();

    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then(a.path.cmp(&b.path)));
    hits.truncate(limit);
    hits
}

// ── ExecuteTests: real test harness ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny Python project with one bug: add() subtracts.
    fn sample_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("useagent-search-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("calc")).unwrap();
        std::fs::write(dir.join("calc/__init__.py"), "").unwrap();
        std::fs::write(dir.join("calc/arith.py"),
            "def add(a, b):\n    return a - b\n\n\ndef multiply(a, b):\n    return a * b\n").unwrap();
        std::fs::write(dir.join("calc/strings.py"), "def shout(s):\n    return s.upper()\n").unwrap();
        std::fs::write(dir.join("test_arith.py"),
            "import unittest\nfrom calc.arith import add, multiply\n\n\nclass ArithTest(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 3), 5)\n").unwrap();
        dir
    }

    #[test]
    fn terms_keep_identifiers_and_drop_noise() {
        let t = extract_terms("Fix the add function in calc_arith: AddResult is wrong for 2 and 3");
        assert!(t.contains(&"calc_arith".to_string()));
        assert!(t.contains(&"addresult".to_string()));
        assert!(!t.contains(&"the".to_string()));
        assert!(!t.contains(&"2".to_string()));
    }

    #[test]
    fn search_finds_definition_of_named_function() {
        let root = sample_repo();
        let hits = search_code(&root, "add returns the wrong sum in arith", 5);
        assert_eq!(hits[0].path, "calc/arith.py", "{hits:?}");
        assert_eq!(hits[0].line, 1);
        assert!(hits[0].is_definition);
        assert!(hits[0].location().starts_with("calc/arith.py:1: def add"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_terms_means_no_hits() {
        let root = sample_repo();
        assert!(search_code(&root, "the and for", 5).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
