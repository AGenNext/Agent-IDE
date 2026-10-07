// USEagent workspace — the real-world side of CodeRetrieval and ExecuteTests.
//
// CodeRetrieval: deterministic code search over a local checkout (term-weighted
//                file ranking + definition-line lookup). No LLM involved.
// ExecuteTests:  runs the project's actual test harness (auto-detected or given
//                as argv), captures exit code + output, and applies the latest
//                EditCode diff in a throwaway git worktree first so post-edit runs
//                test the edited code without touching the caller's checkout.
//
// Gates (deny by default):
//   USEAGENT_WORKSPACE_ROOT    — repo_path must resolve inside this directory
//   USEAGENT_ALLOW_TEST_EXEC   — "true" to allow running test commands
//   USEAGENT_TEST_TIMEOUT_SECS — per-run timeout (default 600)

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_TERMS: usize = 12;
const OUTPUT_TAIL_CHARS: usize = 8_000;
const DEFAULT_TEST_TIMEOUT_SECS: u64 = 600;

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

// ── Workspace resolution ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
}

/// Resolve a caller-supplied repo path, confined to USEAGENT_WORKSPACE_ROOT.
pub fn resolve(repo_path: &str) -> Result<Workspace, String> {
    let allowed = std::env::var("USEAGENT_WORKSPACE_ROOT")
        .map_err(|_| "repo_path given but USEAGENT_WORKSPACE_ROOT is not set".to_string())?;
    let allowed = Path::new(&allowed).canonicalize()
        .map_err(|e| format!("USEAGENT_WORKSPACE_ROOT unusable: {e}"))?;
    let root = Path::new(repo_path).canonicalize()
        .map_err(|e| format!("repo_path {repo_path:?} unusable: {e}"))?;
    if !root.starts_with(&allowed) {
        return Err(format!("repo_path {} is outside USEAGENT_WORKSPACE_ROOT {}", root.display(), allowed.display()));
    }
    if !root.is_dir() {
        return Err(format!("repo_path {} is not a directory", root.display()));
    }
    Ok(Workspace { root })
}

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
pub fn search_code(ws: &Workspace, query: &str, limit: usize) -> Vec<CodeHit> {
    let terms = extract_terms(query);
    if terms.is_empty() { return Vec::new(); }

    let mut files = Vec::new();
    walk(&ws.root, &ws.root, &mut files);

    struct FileMatch { rel: String, counts: Vec<usize>, path_hits: Vec<bool>, lines: Vec<(usize, String, usize, bool)> }
    let mut matches: Vec<FileMatch> = Vec::new();
    let mut doc_freq = vec![0usize; terms.len()];

    for path in &files {
        let Ok(content) = std::fs::read_to_string(path) else { continue };
        let rel = path.strip_prefix(&ws.root).unwrap_or(path).to_string_lossy().replace('\\', "/");
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TestRun {
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub output_tail: String,
    pub tests_run: u32,
    pub tests_failed: u32,
    /// None: no code diff to apply. Some(true/false): whether the latest EditCode diff applied.
    pub patch_applied: Option<bool>,
    pub error: Option<String>,
}

impl TestRun {
    pub fn passed(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out && self.patch_applied != Some(false)
    }
}

pub fn test_exec_allowed() -> bool {
    std::env::var("USEAGENT_ALLOW_TEST_EXEC").as_deref() == Ok("true")
}

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// Pick the project's test command from its manifest files.
pub fn detect_test_command(root: &Path) -> Option<Vec<String>> {
    let argv = |v: &[&str]| Some(v.iter().map(|s| s.to_string()).collect());
    if root.join("Cargo.toml").is_file() {
        return argv(&["cargo", "test", "--color", "never"]);
    }
    if let Ok(pkg) = std::fs::read_to_string(root.join("package.json")) {
        let has_test = serde_json::from_str::<serde_json::Value>(&pkg).ok()
            .and_then(|v| v["scripts"]["test"].as_str().map(|s| !s.contains("no test specified")))
            .unwrap_or(false);
        if has_test { return argv(&["npm", "test", "--silent"]); }
    }
    if root.join("go.mod").is_file() {
        return argv(&["go", "test", "./..."]);
    }
    let pytest_markers = ["pytest.ini", "pyproject.toml", "setup.cfg", "tox.ini", "setup.py", "conftest.py"];
    let has_py_tests = pytest_markers.iter().any(|m| root.join(m).is_file())
        || has_python_test_files(root);
    if has_py_tests {
        return if on_path("pytest") { argv(&["pytest", "-q"]) } else { argv(&["python3", "-m", "pytest", "-q"]) };
    }
    if std::fs::read_to_string(root.join("Makefile")).map(|m| m.lines().any(|l| l.starts_with("test:"))).unwrap_or(false) {
        return argv(&["make", "test"]);
    }
    None
}

fn has_python_test_files(root: &Path) -> bool {
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.iter().any(|p| {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        name.ends_with(".py") && (name.starts_with("test_") || name.ends_with("_test.py"))
    })
}

/// Best-effort pass/fail counts across cargo, pytest, jest and unittest summaries.
/// The exit code stays the source of truth for pass/fail.
pub fn parse_counts(output: &str) -> (u32, u32) {
    let (mut passed, mut failed) = (0u32, 0u32);
    let (mut ut_ran, mut ut_failed, mut saw_unittest) = (0u32, 0u32, false);
    for line in output.lines() {
        let t = line.trim();
        // unittest: "Ran 3 tests in 0.001s" / "FAILED (failures=1, errors=1)"
        if let Some(rest) = t.strip_prefix("Ran ") {
            if let Some(n) = rest.split_whitespace().next().and_then(|n| n.parse::<u32>().ok()) {
                ut_ran += n; saw_unittest = true; continue;
            }
        }
        if saw_unittest && t.starts_with("FAILED (") {
            for part in t.trim_start_matches("FAILED (").trim_end_matches(')').split(',') {
                if let Some((_, n)) = part.split_once('=') { ut_failed += n.trim().parse::<u32>().unwrap_or(0); }
            }
            continue;
        }
        let words: Vec<&str> = t.split_whitespace().collect();
        for w in words.windows(2) {
            let Ok(n) = w[0].parse::<u32>() else { continue };
            let word = w[1].trim_end_matches([',', ';', '.']);
            match word {
                "passed" => passed += n,
                "failed" => failed += n,
                _ => {}
            }
        }
    }
    if saw_unittest { return (ut_ran, ut_failed); }
    (passed + failed, failed)
}

fn tail(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars { return s.to_string(); }
    let skipped: String = s.chars().skip(n - max_chars).collect();
    format!("…[{} chars truncated]\n{skipped}", n - max_chars)
}

async fn run_cmd(argv: &[String], dir: &Path, envs: &[(String, String)], timeout: Duration) -> TestRun {
    let started = Instant::now();
    let mut cmd = tokio::process::Command::new(&argv[0]);
    cmd.args(&argv[1..]).current_dir(dir)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .env("CI", "true")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .kill_on_drop(true);
    for (k, v) in envs { cmd.env(k, v); }
    let mut run = TestRun { command: argv.to_vec(), ..Default::default() };
    match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => {
            run.timed_out = true;
            run.error = Some(format!("timed out after {}s", timeout.as_secs()));
        }
        Ok(Err(e)) => run.error = Some(format!("failed to start {:?}: {e}", argv[0])),
        Ok(Ok(out)) => {
            let combined = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            let (run_n, failed_n) = parse_counts(&combined);
            run.exit_code = out.status.code();
            run.tests_run = run_n;
            run.tests_failed = failed_n;
            run.output_tail = tail(&combined, OUTPUT_TAIL_CHARS);
        }
    }
    run.duration_ms = started.elapsed().as_millis() as u64;
    run
}

async fn git(dir: &Path, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    exec("git", dir, args, stdin).await
}

async fn exec(program: &str, dir: &Path, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new(program)
        .args(args).current_dir(dir)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn().map_err(|e| format!("{program}: {e}"))?;
    if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(input.as_bytes()).await.map_err(|e| format!("{program} stdin: {e}"))?;
        // Ensure the patch ends with a newline; a truncated last hunk is rejected.
        if !input.ends_with('\n') { let _ = pipe.write_all(b"\n").await; }
    }
    let out = child.wait_with_output().await.map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if msg.is_empty() { msg = String::from_utf8_lossy(&out.stdout).trim().to_string(); }
        Err(msg)
    }
}

/// Apply an LLM-written diff. `git apply` is strict about context and hunk counts,
/// so fall back to GNU patch with fuzz when it is installed.
async fn apply_patch(dir: &Path, patch: &str) -> Result<(), String> {
    let strict = match git(dir, &["apply", "--recount", "--whitespace=nowarn", "-"], Some(patch)).await {
        Ok(_) => return Ok(()),
        Err(e) => e,
    };
    if !on_path("patch") { return Err(strict); }
    exec("patch", dir, &["-p1", "--forward", "--batch", "--fuzz=2", "--no-backup-if-mismatch"], Some(patch)).await
        .map(|_| ())
        .map_err(|fuzzy| format!("git apply: {strict}; patch: {fuzzy}"))
}

/// Run the workspace's tests. When `patch` is given and the workspace is a git repo,
/// the patch is applied in a detached worktree of HEAD and the tests run there.
pub async fn run_tests(ws: &Workspace, command: Option<Vec<String>>, patch: Option<&str>) -> TestRun {
    let argv = match command.filter(|c| !c.is_empty()).or_else(|| detect_test_command(&ws.root)) {
        Some(a) => a,
        None => return TestRun {
            error: Some("no test command given and none detected (looked for Cargo.toml, package.json, go.mod, pytest files, Makefile test target)".into()),
            ..Default::default()
        },
    };
    let timeout = Duration::from_secs(
        std::env::var("USEAGENT_TEST_TIMEOUT_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_TEST_TIMEOUT_SECS),
    );

    let mut envs: Vec<(String, String)> = Vec::new();
    if ws.root.join("Cargo.toml").is_file() {
        // Reuse the checkout's build cache so a worktree run doesn't rebuild from scratch.
        let target = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| ws.root.join("target").display().to_string());
        envs.push(("CARGO_TARGET_DIR".into(), target));
    }

    let Some(patch) = patch.filter(|p| !p.trim().is_empty()) else {
        return run_cmd(&argv, &ws.root, &envs, timeout).await;
    };

    if git(&ws.root, &["rev-parse", "--is-inside-work-tree"], None).await.is_err() {
        let mut run = run_cmd(&argv, &ws.root, &envs, timeout).await;
        run.patch_applied = Some(false);
        run.error = Some("not a git repository: latest EditCode diff was not applied; tests ran on the checkout as-is".into());
        return run;
    }

    let wt = std::env::temp_dir().join(format!("useagent-wt-{}", uuid::Uuid::new_v4()));
    let wt_str = wt.display().to_string();
    if let Err(e) = git(&ws.root, &["worktree", "add", "--detach", "--quiet", &wt_str, "HEAD"], None).await {
        return TestRun { command: argv, patch_applied: Some(false), error: Some(format!("git worktree add failed: {e}")), ..Default::default() };
    }
    if ws.root.join("node_modules").is_dir() {
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(ws.root.join("node_modules"), wt.join("node_modules"));
    }

    let run = match apply_patch(&wt, patch).await {
        Err(e) => TestRun {
            command: argv,
            patch_applied: Some(false),
            error: Some(format!("latest EditCode diff did not apply: {e}")),
            ..Default::default()
        },
        Ok(_) => {
            let mut r = run_cmd(&argv, &wt, &envs, timeout).await;
            r.patch_applied = Some(true);
            r
        }
    };

    let _ = git(&ws.root, &["worktree", "remove", "--force", &wt_str], None).await;
    let _ = std::fs::remove_dir_all(&wt);
    let _ = git(&ws.root, &["worktree", "prune"], None).await;
    run
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny Python project with one bug: add() subtracts.
    fn sample_repo(git_init: bool) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("useagent-sample-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("calc")).unwrap();
        std::fs::write(dir.join("calc/__init__.py"), "").unwrap();
        std::fs::write(dir.join("calc/arith.py"),
            "def add(a, b):\n    return a - b\n\n\ndef multiply(a, b):\n    return a * b\n").unwrap();
        std::fs::write(dir.join("calc/strings.py"), "def shout(s):\n    return s.upper()\n").unwrap();
        std::fs::write(dir.join("test_arith.py"),
            "import unittest\nfrom calc.arith import add, multiply\n\n\nclass ArithTest(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 3), 5)\n\n    def test_multiply(self):\n        self.assertEqual(multiply(2, 3), 6)\n").unwrap();
        if git_init {
            let g = |args: &[&str]| { std::process::Command::new("git").args(args).current_dir(&dir).output().unwrap(); };
            g(&["init", "-q"]);
            g(&["add", "."]);
            g(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "init"]);
        }
        dir
    }

    fn unittest_cmd() -> Option<Vec<String>> {
        Some(vec!["python3".into(), "-m".into(), "unittest".into(), "-q".into()])
    }

    const FIX: &str = "--- a/calc/arith.py\n+++ b/calc/arith.py\n@@ -1,2 +1,2 @@\n def add(a, b):\n-    return a - b\n+    return a + b\n";

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
        let ws = Workspace { root: sample_repo(false) };
        let hits = search_code(&ws, "add returns the wrong sum in arith", 5);
        assert_eq!(hits[0].path, "calc/arith.py", "{hits:?}");
        assert_eq!(hits[0].line, 1);
        assert!(hits[0].is_definition);
        assert!(hits[0].location().starts_with("calc/arith.py:1: def add"));
        let _ = std::fs::remove_dir_all(&ws.root);
    }

    #[test]
    fn parses_common_summaries() {
        assert_eq!(parse_counts("test result: FAILED. 2 passed; 1 failed; 0 ignored"), (3, 1));
        assert_eq!(parse_counts("==== 1 failed, 4 passed in 0.12s ===="), (5, 1));
        assert_eq!(parse_counts("Tests:       2 failed, 3 passed, 5 total"), (5, 2));
        assert_eq!(parse_counts("Ran 2 tests in 0.001s\n\nFAILED (failures=1)"), (2, 1));
        assert_eq!(parse_counts("Ran 2 tests in 0.001s\n\nOK"), (2, 0));
    }

    #[test]
    fn detects_python_project() {
        let root = sample_repo(false);
        let cmd = detect_test_command(&root).unwrap();
        assert!(cmd.join(" ").contains("pytest"), "{cmd:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn real_run_reports_failure_then_patch_fixes_it() {
        let ws = Workspace { root: sample_repo(true) };

        let before = run_tests(&ws, unittest_cmd(), None).await;
        assert_eq!(before.exit_code, Some(1), "{before:?}");
        assert_eq!((before.tests_run, before.tests_failed), (2, 1));
        assert!(!before.passed());
        assert!(before.output_tail.contains("test_add"));

        let after = run_tests(&ws, unittest_cmd(), Some(FIX)).await;
        assert_eq!(after.patch_applied, Some(true), "{after:?}");
        assert_eq!(after.exit_code, Some(0), "{after:?}");
        assert!(after.passed());

        // The caller's checkout is untouched and the worktree is gone.
        let src = std::fs::read_to_string(ws.root.join("calc/arith.py")).unwrap();
        assert!(src.contains("a - b"));
        let wts = std::process::Command::new("git").args(["worktree", "list"]).current_dir(&ws.root).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&wts.stdout).lines().count(), 1);
        let _ = std::fs::remove_dir_all(&ws.root);
    }

    #[tokio::test]
    async fn bad_patch_is_a_failed_run_not_a_pass() {
        let ws = Workspace { root: sample_repo(true) };
        let run = run_tests(&ws, unittest_cmd(), Some("this is not a diff")).await;
        assert_eq!(run.patch_applied, Some(false));
        assert!(!run.passed());
        assert!(run.error.unwrap().contains("did not apply"));
        let _ = std::fs::remove_dir_all(&ws.root);
    }

    #[test]
    fn resolve_rejects_paths_outside_root() {
        let root = sample_repo(false);
        // Single test touches the env var to avoid races between tests.
        std::env::set_var("USEAGENT_WORKSPACE_ROOT", root.join("calc"));
        assert!(resolve(root.to_str().unwrap()).unwrap_err().contains("outside"));
        std::env::set_var("USEAGENT_WORKSPACE_ROOT", &root);
        assert!(resolve(root.join("calc").to_str().unwrap()).is_ok());
        assert!(resolve(root.join("calc/../..").to_str().unwrap()).is_err());
        std::env::remove_var("USEAGENT_WORKSPACE_ROOT");
        let _ = std::fs::remove_dir_all(&root);
    }
}
