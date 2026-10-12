// USEagent ExecuteTests — runs the project's real test suite instead of asking
// the model to predict pass/fail.
//
// Safety model:
//   - Disabled unless USE_AGENT_WORKSPACE_ROOT is set. Every requested
//     workspace must resolve to a directory under that root.
//   - The test command never comes from the request. It is USE_AGENT_TEST_COMMAND
//     if the operator set one, otherwise detected from the workspace's manifest.
//   - Tests run in a scratch copy (a detached git worktree, or a plain copy for
//     non-git dirs) so the latest EditCode patch can be applied without touching
//     the user's checkout. The scratch copy is removed afterwards.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use chrono::Utc;
use tokio::process::Command;
use uuid::Uuid;

use crate::use_agent::{DiffKind, ExecResult, TaskState};

const DEFAULT_TIMEOUT_SECS: u64 = 600;
const MAX_OUTPUT_BYTES: usize = 8 * 1024;

pub async fn run_tests(workspace: Option<&str>, task_state: &TaskState) -> ExecResult {
    let started = Instant::now();
    let mut result = match prepare_and_run(workspace, task_state).await {
        Ok(r) => r,
        Err(reason) => not_executed(reason),
    };
    result.duration_ms = started.elapsed().as_millis() as u64;
    result
}

async fn prepare_and_run(workspace: Option<&str>, task_state: &TaskState) -> Result<ExecResult, String> {
    let root = std::env::var("USE_AGENT_WORKSPACE_ROOT").ok()
        .filter(|r| !r.trim().is_empty())
        .ok_or_else(|| "Tests not executed: real test execution is disabled (set USE_AGENT_WORKSPACE_ROOT on the runner).".to_string())?;
    let workspace = workspace
        .ok_or_else(|| "Tests not executed: request has no `workspace` to run tests in.".to_string())?;
    let workspace = resolve_workspace(&root, workspace)?;

    let command = match std::env::var("USE_AGENT_TEST_COMMAND") {
        Ok(c) if !c.trim().is_empty() => c,
        _ => detect_test_command(&workspace)
            .ok_or_else(|| format!("Tests not executed: could not detect a test command in {} (set USE_AGENT_TEST_COMMAND).", workspace.display()))?
            .to_string(),
    };

    let scratch = std::env::temp_dir().join(format!("use-agent-{}", Uuid::new_v4()));
    let is_git = workspace.join(".git").exists();
    create_scratch(&workspace, &scratch, is_git).await?;

    let patch_note = match task_state.diff_store.iter().rev().find(|d| d.kind == DiffKind::Code) {
        Some(diff) => apply_patch(&scratch, &diff.content).await.err(),
        None => None,
    };

    let run = run_command(&scratch, &command).await;
    remove_scratch(&workspace, &scratch, is_git).await;
    let (exit_code, raw_output) = run?;

    let (tests_run, tests_failed) = parse_counts(&raw_output);
    let mut output = tail(&raw_output, MAX_OUTPUT_BYTES);
    if let Some(note) = &patch_note {
        output = format!("{note}\n\n{output}");
    }

    Ok(ExecResult {
        timestamp: Utc::now(),
        // A run against an unpatched tree says nothing about the patch.
        passed: exit_code == Some(0) && patch_note.is_none(),
        output,
        tests_run,
        tests_failed,
        executed: true,
        command: Some(command),
        exit_code,
        duration_ms: 0,
    })
}

fn not_executed(reason: String) -> ExecResult {
    ExecResult {
        timestamp: Utc::now(),
        passed: false,
        output: reason,
        tests_run: 0,
        tests_failed: 0,
        executed: false,
        command: None,
        exit_code: None,
        duration_ms: 0,
    }
}

pub(crate) fn resolve_workspace(root: &str, workspace: &str) -> Result<PathBuf, String> {
    let root = Path::new(root).canonicalize()
        .map_err(|e| format!("Tests not executed: USE_AGENT_WORKSPACE_ROOT is not usable ({e})."))?;
    let requested = Path::new(workspace);
    let joined = if requested.is_absolute() { requested.to_path_buf() } else { root.join(requested) };
    let resolved = joined.canonicalize()
        .map_err(|e| format!("Tests not executed: workspace {workspace} not found ({e})."))?;
    if !resolved.starts_with(&root) || !resolved.is_dir() {
        return Err(format!("Tests not executed: workspace {workspace} is outside USE_AGENT_WORKSPACE_ROOT."));
    }
    Ok(resolved)
}

pub fn detect_test_command(dir: &Path) -> Option<&'static str> {
    if dir.join("Cargo.toml").exists() { return Some("cargo test --color never 2>&1"); }
    if dir.join("go.mod").exists()     { return Some("go test -v ./... 2>&1"); }
    if dir.join("package.json").exists() { return Some("CI=true npm test --silent 2>&1"); }
    if ["pyproject.toml", "pytest.ini", "setup.py", "setup.cfg", "tox.ini"].iter().any(|f| dir.join(f).exists()) {
        return Some("python -m pytest -q 2>&1");
    }
    None
}

async fn create_scratch(workspace: &Path, scratch: &Path, is_git: bool) -> Result<(), String> {
    let out = if is_git {
        Command::new("git").arg("-C").arg(workspace)
            .args(["worktree", "add", "--detach", "--quiet"]).arg(scratch).arg("HEAD")
            .output().await
    } else {
        Command::new("cp").arg("-a").arg(workspace).arg(scratch).output().await
    };
    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!("Tests not executed: could not prepare scratch copy: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => Err(format!("Tests not executed: could not prepare scratch copy: {e}")),
    }
}

async fn remove_scratch(workspace: &Path, scratch: &Path, is_git: bool) {
    if is_git {
        let _ = Command::new("git").arg("-C").arg(workspace)
            .args(["worktree", "remove", "--force"]).arg(scratch)
            .output().await;
    }
    let _ = tokio::fs::remove_dir_all(scratch).await;
}

/// Apply a unified diff with `git apply`. Returns a note when it does not apply.
async fn apply_patch(dir: &Path, diff: &str) -> Result<(), String> {
    let patch = dir.join(".use-agent.patch");
    let mut body = diff.trim_end().to_string();
    body.push('\n');
    tokio::fs::write(&patch, body).await.map_err(|e| format!("Patch not applied: {e}"))?;
    let out = Command::new("git").arg("-C").arg(dir)
        .args(["apply", "--recount", "--whitespace=nowarn", ".use-agent.patch"])
        .output().await;
    let _ = tokio::fs::remove_file(&patch).await;
    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!("Patch not applied, tests ran on the unpatched tree: {}", String::from_utf8_lossy(&o.stderr).trim())),
        Err(e) => Err(format!("Patch not applied, tests ran on the unpatched tree: {e}")),
    }
}

async fn run_command(dir: &Path, command: &str) -> Result<(Option<i32>, String), String> {
    let timeout = std::env::var("USE_AGENT_TEST_TIMEOUT_SECS").ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);
    let child = Command::new("sh").arg("-c").arg(command)
        .current_dir(dir)
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(Duration::from_secs(timeout), child).await {
        Ok(Ok(out)) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.trim().is_empty() {
                text.push_str("\n");
                text.push_str(&stderr);
            }
            Ok((out.status.code(), text))
        }
        Ok(Err(e)) => Err(format!("Tests not executed: could not start `{command}`: {e}")),
        Err(_) => Ok((None, format!("Test command `{command}` timed out after {timeout}s."))),
    }
}

/// Best-effort totals from cargo, pytest, jest/vitest and `go test -v` output.
/// Pass/fail is decided by the exit code, not by these counts.
pub fn parse_counts(output: &str) -> (u32, u32) {
    let mut passed = 0u32;
    let mut failed = 0u32;
    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("--- PASS") { passed += 1; continue; }
        if line.starts_with("--- FAIL") { failed += 1; continue; }
        // jest prints a suite summary next to the test summary; count tests only.
        if line.starts_with("Test Suites:") { continue; }
        let is_summary = line.starts_with("test result:")       // cargo
            || line.starts_with("Tests:")                       // jest
            || line.starts_with("Tests ")                       // vitest
            || (line.starts_with('=') && line.contains(" in ")) // pytest
            || line.contains(" passed in ")                     // pytest -q
            || line.contains(" failed in ");
        if !is_summary { continue; }
        passed += count_before(line, "passed");
        failed += count_before(line, "failed");
    }
    (passed + failed, failed)
}

fn count_before(line: &str, word: &str) -> u32 {
    let words: Vec<&str> = line.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '(' || c == ')')
        .filter(|w| !w.is_empty())
        .collect();
    words.windows(2)
        .filter(|w| w[1].trim_end_matches('.') == word)
        .filter_map(|w| w[0].parse::<u32>().ok())
        .sum()
}

fn tail(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let mut start = s.len() - max;
    while !s.is_char_boundary(start) { start += 1; }
    format!("…(truncated)\n{}", &s[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_cargo() {
        let out = "running 3 tests\ntest result: ok. 3 passed; 0 failed; 0 ignored\n\
                   test result: FAILED. 1 passed; 2 failed; 0 ignored";
        assert_eq!(parse_counts(out), (6, 2));
    }

    #[test]
    fn counts_pytest() {
        assert_eq!(parse_counts("==== 2 failed, 10 passed in 0.31s ===="), (12, 2));
        assert_eq!(parse_counts("5 passed in 0.02s"), (5, 0));
    }

    #[test]
    fn counts_jest() {
        let out = "Test Suites: 1 failed, 2 passed, 3 total\nTests:       1 failed, 7 passed, 8 total";
        assert_eq!(parse_counts(out), (8, 1));
    }

    #[test]
    fn counts_go() {
        let out = "=== RUN TestA\n--- PASS: TestA (0.00s)\n--- FAIL: TestB (0.00s)\nFAIL";
        assert_eq!(parse_counts(out), (2, 1));
    }

    #[test]
    fn rejects_workspace_outside_root() {
        let root = std::env::temp_dir().join(format!("ua-root-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let err = resolve_workspace(root.to_str().unwrap(), "/").unwrap_err();
        assert!(err.contains("outside"));
        let err = resolve_workspace(root.to_str().unwrap(), "../").unwrap_err();
        assert!(err.contains("outside"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn runs_real_tests_and_applies_patch() {
        let root = std::env::temp_dir().join(format!("ua-root-{}", Uuid::new_v4()));
        let ws = root.join("proj");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(ws.join("value.txt"), "bad\n").unwrap();
        let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(&ws).args(args).output().unwrap();
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "init"]);

        std::env::set_var("USE_AGENT_WORKSPACE_ROOT", &root);
        std::env::set_var("USE_AGENT_TEST_COMMAND", "grep -q good value.txt && echo '1 passed in 0.01s' || { echo '1 failed in 0.01s'; exit 1; }");

        let mut state = TaskState::default();
        let r = run_tests(Some("proj"), &state).await;
        assert!(r.executed && !r.passed, "{}", r.output);
        assert_eq!((r.tests_run, r.tests_failed, r.exit_code), (1, 1, Some(1)));

        state.diff_store.push(crate::use_agent::Diff {
            id: Uuid::new_v4(), kind: DiffKind::Code,
            content: "--- a/value.txt\n+++ b/value.txt\n@@ -1 +1 @@\n-bad\n+good\n".into(),
            created_at: Utc::now(), source_action: "EditCode".into(),
        });
        let r = run_tests(Some("proj"), &state).await;
        assert!(r.executed && r.passed, "{}", r.output);
        assert_eq!((r.tests_run, r.tests_failed), (1, 0));

        // The user's checkout is untouched and the scratch worktree is gone.
        assert_eq!(std::fs::read_to_string(ws.join("value.txt")).unwrap(), "bad\n");
        let wt = git(&["worktree", "list"]);
        assert_eq!(String::from_utf8_lossy(&wt.stdout).lines().count(), 1);

        std::env::remove_var("USE_AGENT_TEST_COMMAND");
        std::env::remove_var("USE_AGENT_WORKSPACE_ROOT");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
