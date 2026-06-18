use std::sync::Arc;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::AppState;
use crate::fabric::FabricEvent;
use crate::lifecycle::Stage;

const MAX_STEPS: usize = 20;

// ── Task Types (6 variants from USEbench) ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SeTaskType {
    ProgramRepair,
    RegressionTesting,
    CodeGeneration,
    TestGeneration,
    PartialFix,
    FeatureDevelopment,
}

impl SeTaskType {
    pub fn description(&self) -> &'static str {
        match self {
            SeTaskType::ProgramRepair => "Fix bugs in existing code to make tests pass",
            SeTaskType::RegressionTesting => "Write regression tests for existing code changes",
            SeTaskType::CodeGeneration => "Generate new code based on a specification",
            SeTaskType::TestGeneration => "Generate test cases for existing code",
            SeTaskType::PartialFix => "Compound task: partial fix with incomplete patch provided",
            SeTaskType::FeatureDevelopment => "Compound task: develop a new feature end-to-end",
        }
    }

    pub fn from_description(desc: &str) -> Self {
        let lower = desc.to_lowercase();
        if lower.contains("feature") || lower.contains("develop") || lower.contains("implement new") {
            SeTaskType::FeatureDevelopment
        } else if lower.contains("partial") || lower.contains("incomplete patch") || lower.contains("partial fix") {
            SeTaskType::PartialFix
        } else if lower.contains("regression") {
            SeTaskType::RegressionTesting
        } else if lower.contains("generate test") || lower.contains("write test") || lower.contains("test generation") {
            SeTaskType::TestGeneration
        } else if lower.contains("generate code") || lower.contains("code generation") || lower.contains("create new") {
            SeTaskType::CodeGeneration
        } else {
            SeTaskType::ProgramRepair
        }
    }

    pub fn default_action_sequence(&self) -> Vec<SeAction> {
        match self {
            SeTaskType::ProgramRepair => vec![
                SeAction::CodeRetrieval,
                SeAction::Reproduction,
                SeAction::ExecuteTests,
                SeAction::EditCode,
                SeAction::ExecuteTests,
                SeAction::ReviewPatch,
                SeAction::Terminate,
            ],
            SeTaskType::RegressionTesting => vec![
                SeAction::CodeRetrieval,
                SeAction::TestRetrieval,
                SeAction::ExecuteTests,
                SeAction::EditCode,
                SeAction::ExecuteTests,
                SeAction::ReviewPatch,
                SeAction::Terminate,
            ],
            SeTaskType::CodeGeneration => vec![
                SeAction::CodeRetrieval,
                SeAction::EditCode,
                SeAction::ReviewPatch,
                SeAction::Terminate,
            ],
            SeTaskType::TestGeneration => vec![
                SeAction::CodeRetrieval,
                SeAction::TestRetrieval,
                SeAction::EditCode,
                SeAction::ExecuteTests,
                SeAction::ReviewPatch,
                SeAction::Terminate,
            ],
            SeTaskType::PartialFix => vec![
                SeAction::CodeRetrieval,
                SeAction::Reproduction,
                SeAction::ExecuteTests,
                SeAction::EditCode,
                SeAction::ExecuteTests,
                SeAction::ReviewPatch,
                SeAction::EditCode,
                SeAction::ExecuteTests,
                SeAction::ReviewPatch,
                SeAction::Terminate,
            ],
            SeTaskType::FeatureDevelopment => vec![
                SeAction::CodeRetrieval,
                SeAction::TestRetrieval,
                SeAction::EditCode,
                SeAction::TestRetrieval,
                SeAction::ExecuteTests,
                SeAction::ReviewPatch,
                SeAction::EditCode,
                SeAction::ExecuteTests,
                SeAction::ReviewPatch,
                SeAction::Terminate,
            ],
        }
    }
}

// ── SE Actions (7 variants from Table 2) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SeAction {
    CodeRetrieval,
    TestRetrieval,
    Reproduction,
    ExecuteTests,
    EditCode,
    ReviewPatch,
    Terminate,
}

impl SeAction {
    pub fn name(&self) -> &'static str {
        match self {
            SeAction::CodeRetrieval => "CodeRetrieval",
            SeAction::TestRetrieval => "TestRetrieval",
            SeAction::Reproduction => "Reproduction",
            SeAction::ExecuteTests => "ExecuteTests",
            SeAction::EditCode => "EditCode",
            SeAction::ReviewPatch => "ReviewPatch",
            SeAction::Terminate => "Terminate",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SeAction::CodeRetrieval => "Retrieve relevant code locations (files, functions, classes) related to the task",
            SeAction::TestRetrieval => "Retrieve relevant test locations and existing test cases",
            SeAction::Reproduction => "Reproduce the bug or issue to understand it clearly",
            SeAction::ExecuteTests => "Execute tests and collect results/coverage information",
            SeAction::EditCode => "Edit or generate code to address the task",
            SeAction::ReviewPatch => "Review the generated patch for correctness and quality",
            SeAction::Terminate => "Terminate the agent loop and return the final result",
        }
    }

    pub fn all_variants() -> Vec<SeAction> {
        vec![
            SeAction::CodeRetrieval,
            SeAction::TestRetrieval,
            SeAction::Reproduction,
            SeAction::ExecuteTests,
            SeAction::EditCode,
            SeAction::ReviewPatch,
            SeAction::Terminate,
        ]
    }
}

// ── Task State S = (Lc, Lt, Rexec, DS) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskState {
    pub code_locations: Vec<String>,
    pub test_locations: Vec<String>,
    pub exec_results: Vec<ExecResult>,
    pub diff_store: Vec<Diff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub timestamp: DateTime<Utc>,
    pub passed: bool,
    pub output: String,
    pub tests_run: u32,
    pub tests_failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diff {
    pub id: Uuid,
    pub kind: DiffKind,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub source_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Code,
    Test,
    Reproduction,
    Review,
}

// ── Action Record ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    pub step: usize,
    pub action: SeAction,
    pub output: String,
    pub timestamp: DateTime<Utc>,
}

// ── Request / Response ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseAgentRequest {
    pub task_description: String,
    pub project_context: Option<String>,
    #[serde(default)]
    pub dynamic: bool,
    pub task_type: Option<SeTaskType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseAgentResponse {
    pub status: UseAgentStatus,
    pub task_type: SeTaskType,
    pub steps_taken: usize,
    pub final_diff: Option<String>,
    pub task_state: TaskState,
    pub action_history: Vec<ActionRecord>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UseAgentStatus {
    Success,
    Failed,
    MaxStepsReached,
}

// ── Meta-Agent: select action via LLM ────────────────────────────────────────

async fn meta_agent_select_action(
    state: &Arc<AppState>,
    task_type: &SeTaskType,
    task_state: &TaskState,
    last_output: &str,
    task_description: &str,
    step: usize,
) -> SeAction {
    let (model, _) = {
        let llm_config = state.llm_config.read().unwrap();
        (llm_config.default_model.clone(), llm_config.default_max_tokens)
    };
    let max_tokens: u32 = 32;

    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();

    let system = "You are Meta-Agent in USEagent. Output ONLY the action name.".to_string();

    let state_summary = serde_json::to_string_pretty(task_state).unwrap_or_default();
    let action_list = SeAction::all_variants()
        .iter()
        .map(|a| a.name())
        .collect::<Vec<_>>()
        .join(", ");

    let user_msg = format!(
        "Task Type: {task_type:?}\nTask Description: {task_description}\nStep: {step}\n\
         Current Task State:\n{state_summary}\nLast Action Output:\n{last_output}\n\n\
         Available actions: {action_list}\nWhich action should be taken next? Output ONLY the action name."
    );

    let messages: Vec<Value> = vec![json!({"role": "user", "content": user_msg})];

    match crate::providers::complete(state.egress.llm(), &model, &api_key, &system, &messages, max_tokens).await {
        Ok(resp) => parse_action_choice(&resp.text),
        Err(_) => SeAction::Terminate,
    }
}

fn parse_action_choice(s: &str) -> SeAction {
    let normalized = s.trim().to_lowercase().replace([' ', '-', '_'], "");
    match normalized.as_str() {
        "coderetrieval" => SeAction::CodeRetrieval,
        "testretrieval" => SeAction::TestRetrieval,
        "reproduction" => SeAction::Reproduction,
        "executetests" => SeAction::ExecuteTests,
        "editcode" => SeAction::EditCode,
        "reviewpatch" => SeAction::ReviewPatch,
        "terminate" => SeAction::Terminate,
        other => {
            if other.contains("code") && other.contains("retrieval") {
                SeAction::CodeRetrieval
            } else if other.contains("test") && other.contains("retrieval") {
                SeAction::TestRetrieval
            } else if other.contains("repro") {
                SeAction::Reproduction
            } else if other.contains("execute") || other.contains("run") {
                SeAction::ExecuteTests
            } else if other.contains("edit") {
                SeAction::EditCode
            } else if other.contains("review") {
                SeAction::ReviewPatch
            } else {
                SeAction::Terminate
            }
        }
    }
}

// ── Action Execution ──────────────────────────────────────────────────────────

async fn execute_action(
    state: &Arc<AppState>,
    action: &SeAction,
    task_state: &TaskState,
    task_description: &str,
    project_context: &str,
) -> String {
    if *action == SeAction::Terminate {
        return "Agent terminating. Final patch selection complete.".to_string();
    }

    let (model, max_tokens) = {
        let llm_config = state.llm_config.read().unwrap();
        (llm_config.default_model.clone(), llm_config.default_max_tokens)
    };

    let api_key = std::env::var("LLM_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();

    let (system_prompt, user_message) = build_action_prompt(action, task_state, task_description, project_context);
    let messages: Vec<Value> = vec![json!({"role": "user", "content": user_message})];

    match crate::providers::complete(state.egress.llm(), &model, &api_key, &system_prompt, &messages, max_tokens).await {
        Ok(resp) => resp.text,
        Err(e) => format!("Action execution error: {e}"),
    }
}

fn build_action_prompt(
    action: &SeAction,
    task_state: &TaskState,
    task_description: &str,
    project_context: &str,
) -> (String, String) {
    let state_json = serde_json::to_string_pretty(task_state).unwrap_or_default();
    let system = format!(
        "You are a software engineering AI executing the {} action as part of the USEagent framework. \
         Be precise, technical, and output structured results.",
        action.name()
    );
    let user = match action {
        SeAction::CodeRetrieval => format!(
            "Task: {task_description}\nProject Context:\n{project_context}\nCurrent State:\n{state_json}\n\n\
             Perform CodeRetrieval: Identify and list the relevant code locations (files, classes, functions) \
             that need to be examined or modified to address this task.\n\
             Format your response as:\nLOCATIONS:\n[\"src/foo.rs\", \"src/bar.rs:MyStruct::my_method\"]"
        ),
        SeAction::TestRetrieval => format!(
            "Task: {task_description}\nProject Context:\n{project_context}\nCurrent State:\n{state_json}\n\
             Code Locations: {code_locs:?}\n\n\
             Perform TestRetrieval: Identify and list the relevant test files and test cases.\n\
             Format your response as:\nTEST_LOCATIONS:\n[\"tests/foo_test.rs\"]",
            code_locs = task_state.code_locations
        ),
        SeAction::Reproduction => format!(
            "Task: {task_description}\nProject Context:\n{project_context}\nCurrent State:\n{state_json}\n\n\
             Perform Reproduction: Write a minimal reproduction script or test case.\n\
             Format your response as:\nREPRODUCTION_DIFF:\n```\n<diff content here>\n```"
        ),
        SeAction::ExecuteTests => format!(
            "Task: {task_description}\nProject Context:\n{project_context}\nCurrent State:\n{state_json}\n\
             Test Locations: {test_locs:?}\n\n\
             Perform ExecuteTests: Analyze what tests would pass/fail.\n\
             Format your response as:\nEXEC_RESULT:\nPASSED: <true|false>\nTESTS_RUN: <number>\nTESTS_FAILED: <number>\nOUTPUT: <summary>",
            test_locs = task_state.test_locations
        ),
        SeAction::EditCode => format!(
            "Task: {task_description}\nProject Context:\n{project_context}\nCurrent State:\n{state_json}\n\
             Code Locations: {code_locs:?}\nExec Results: {exec_summary}\n\n\
             Perform EditCode: Generate the code changes needed. Output a unified diff patch.\n\
             Format your response as:\nCODE_DIFF:\n```diff\n<unified diff here>\n```",
            code_locs = task_state.code_locations,
            exec_summary = task_state.exec_results.last()
                .map(|r| format!("passed={}, output={}", r.passed, r.output))
                .unwrap_or_else(|| "no results yet".to_string())
        ),
        SeAction::ReviewPatch => format!(
            "Task: {task_description}\nProject Context:\n{project_context}\nCurrent State:\n{state_json}\n\
             Latest Diff:\n{latest_diff}\n\n\
             Perform ReviewPatch: Review the latest generated patch/diff.\n\
             Format your response as:\nREVIEW_DIFF:\nAPPROVED: <true|false>\nCOMMENTS: <review>\nSUGGESTED_CHANGES: <diff if needed>",
            latest_diff = task_state.diff_store.last()
                .map(|d| d.content.clone())
                .unwrap_or_else(|| "No diff available yet".to_string())
        ),
        SeAction::Terminate => "Agent is terminating.".to_string(),
    };
    (system, user)
}

fn apply_action_output(action: &SeAction, output: &str, state: &mut TaskState) {
    match action {
        SeAction::CodeRetrieval => {
            if let Some(start) = output.find("LOCATIONS:") {
                let after = &output[start + "LOCATIONS:".len()..];
                let trimmed = after.trim();
                if let Some(arr_start) = trimmed.find('[') {
                    if let Some(arr_end) = trimmed.find(']') {
                        let arr_str = &trimmed[arr_start..=arr_end];
                        if let Ok(locs) = serde_json::from_str::<Vec<String>>(arr_str) {
                            state.code_locations = locs;
                            return;
                        }
                    }
                }
            }
            let locs: Vec<String> = output.lines()
                .filter(|l| l.contains('/') || l.contains(".rs") || l.contains(".py") || l.contains(".ts"))
                .map(|l| l.trim().trim_matches('"').to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if !locs.is_empty() { state.code_locations = locs; }
        }
        SeAction::TestRetrieval => {
            if let Some(start) = output.find("TEST_LOCATIONS:") {
                let after = &output[start + "TEST_LOCATIONS:".len()..];
                let trimmed = after.trim();
                if let Some(arr_start) = trimmed.find('[') {
                    if let Some(arr_end) = trimmed.find(']') {
                        let arr_str = &trimmed[arr_start..=arr_end];
                        if let Ok(locs) = serde_json::from_str::<Vec<String>>(arr_str) {
                            state.test_locations = locs;
                            return;
                        }
                    }
                }
            }
            let locs: Vec<String> = output.lines()
                .filter(|l| l.contains("test") || l.contains("spec") || l.contains("_test") || l.contains("Test"))
                .map(|l| l.trim().trim_matches('"').to_string())
                .filter(|l| !l.is_empty() && (l.contains('/') || l.contains('.')))
                .collect();
            if !locs.is_empty() { state.test_locations = locs; }
        }
        SeAction::Reproduction => {
            let content = extract_code_block(output, "REPRODUCTION_DIFF:").unwrap_or_else(|| output.to_string());
            state.diff_store.push(Diff {
                id: Uuid::new_v4(), kind: DiffKind::Reproduction, content,
                created_at: Utc::now(), source_action: "Reproduction".to_string(),
            });
        }
        SeAction::ExecuteTests => {
            let passed = output.contains("PASSED: true");
            let tests_run = parse_labeled_u32(output, "TESTS_RUN:");
            let tests_failed = parse_labeled_u32(output, "TESTS_FAILED:");
            let out_summary = extract_after_label(output, "OUTPUT:").unwrap_or_else(|| output.to_string());
            state.exec_results.push(ExecResult {
                timestamp: Utc::now(), passed, output: out_summary, tests_run, tests_failed,
            });
        }
        SeAction::EditCode => {
            let content = extract_code_block(output, "CODE_DIFF:").unwrap_or_else(|| output.to_string());
            state.diff_store.push(Diff {
                id: Uuid::new_v4(), kind: DiffKind::Code, content,
                created_at: Utc::now(), source_action: "EditCode".to_string(),
            });
        }
        SeAction::ReviewPatch => {
            let content = extract_code_block(output, "REVIEW_DIFF:").unwrap_or_else(|| output.to_string());
            state.diff_store.push(Diff {
                id: Uuid::new_v4(), kind: DiffKind::Review, content,
                created_at: Utc::now(), source_action: "ReviewPatch".to_string(),
            });
        }
        SeAction::Terminate => {}
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_code_block(text: &str, after_label: &str) -> Option<String> {
    let start = text.find(after_label)?;
    let remainder = &text[start + after_label.len()..];
    if let Some(fence_start) = remainder.find("```") {
        let inner = &remainder[fence_start + 3..];
        let content_start = inner.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &inner[content_start..];
        let end = content.find("```").unwrap_or(content.len());
        return Some(content[..end].trim().to_string());
    }
    Some(remainder.trim().to_string())
}

fn extract_after_label(text: &str, label: &str) -> Option<String> {
    let start = text.find(label)?;
    let remainder = &text[start + label.len()..];
    let end = remainder.find('\n').unwrap_or(remainder.len());
    Some(remainder[..end].trim().to_string())
}

fn parse_labeled_u32(text: &str, label: &str) -> u32 {
    extract_after_label(text, label)
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0)
}

// ── Main Entry Point ──────────────────────────────────────────────────────────

pub async fn execute_use_agent(state: Arc<AppState>, req: UseAgentRequest) -> UseAgentResponse {
    let task_type = req.task_type.unwrap_or_else(|| SeTaskType::from_description(&req.task_description));
    let project_context = req.project_context.unwrap_or_default();
    let dynamic = req.dynamic;

    let mut task_state = TaskState::default();
    let mut action_history: Vec<ActionRecord> = Vec::new();
    let mut last_output = String::new();

    let static_sequence = if !dynamic {
        Some(task_type.default_action_sequence())
    } else {
        None
    };

    let mut step = 0;
    let status;

    loop {
        if step >= MAX_STEPS {
            status = UseAgentStatus::MaxStepsReached;
            break;
        }

        let action = if dynamic {
            meta_agent_select_action(
                &state, &task_type, &task_state, &last_output,
                &req.task_description, step,
            ).await
        } else {
            let seq = static_sequence.as_ref().unwrap();
            if step < seq.len() { seq[step].clone() } else { SeAction::Terminate }
        };

        let event_payload = json!({
            "step": step,
            "action": action.name(),
            "task_type": task_type,
            "task_state_summary": {
                "code_locations_count": task_state.code_locations.len(),
                "test_locations_count": task_state.test_locations.len(),
                "exec_results_count": task_state.exec_results.len(),
                "diff_store_count": task_state.diff_store.len(),
            }
        });
        state.fabric.emit(
            FabricEvent::open("use-agent", Stage::Observe, event_payload)
                .with_entities(["task", req.task_description.as_str()])
        );

        if action == SeAction::Terminate {
            status = UseAgentStatus::Success;
            action_history.push(ActionRecord {
                step, action, output: "Terminated".to_string(), timestamp: Utc::now(),
            });
            break;
        }

        let output = execute_action(&state, &action, &task_state, &req.task_description, &project_context).await;
        apply_action_output(&action, &output, &mut task_state);
        action_history.push(ActionRecord {
            step, action: action.clone(), output: output.clone(), timestamp: Utc::now(),
        });
        last_output = output;
        step += 1;
    }

    let final_diff = task_state.diff_store.iter()
        .rev()
        .find(|d| d.kind == DiffKind::Code)
        .or_else(|| task_state.diff_store.last())
        .map(|d| d.content.clone());

    let message = match &status {
        UseAgentStatus::Success => format!("USEagent completed successfully in {} steps.", step + 1),
        UseAgentStatus::Failed => "USEagent encountered a failure.".to_string(),
        UseAgentStatus::MaxStepsReached => format!("USEagent reached maximum steps ({MAX_STEPS})."),
    };

    UseAgentResponse {
        status, task_type, steps_taken: step, final_diff,
        task_state, action_history, message,
    }
}
