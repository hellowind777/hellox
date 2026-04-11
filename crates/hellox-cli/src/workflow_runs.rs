mod render;
mod selector;
mod store;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use hellox_agent::AgentSession;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::workflows::{execute_workflow, WorkflowRunTarget};

const WORKFLOW_SCRIPT_PREFIX: &str = ".hellox/workflows/";
pub(crate) const WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT: usize = 8;

pub(crate) use render::{
    render_workflow_run_inspect_panel_with_step, render_workflow_run_list,
    select_workflow_run_step_number,
};
pub(crate) use selector::render_run_selector_with_start;
pub(crate) use store::{list_workflow_runs, load_latest_workflow_run, load_workflow_run};

use self::render::render_recorded_workflow_output;
use self::store::save_workflow_run;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WorkflowRunSummary {
    pub(crate) total_steps: usize,
    pub(crate) completed_steps: usize,
    pub(crate) failed_steps: usize,
    pub(crate) running_steps: usize,
    pub(crate) skipped_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WorkflowRunStepRecord {
    pub(crate) name: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) result_text: Option<String>,
    #[serde(default)]
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WorkflowRunRecord {
    pub(crate) run_id: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) workflow_name: Option<String>,
    #[serde(default)]
    pub(crate) workflow_source: Option<String>,
    #[serde(default)]
    pub(crate) requested_script_path: Option<String>,
    pub(crate) started_at: u64,
    pub(crate) finished_at: u64,
    #[serde(default)]
    pub(crate) shared_context: Option<String>,
    #[serde(default)]
    pub(crate) continue_on_error: Option<bool>,
    #[serde(default)]
    pub(crate) summary: WorkflowRunSummary,
    #[serde(default)]
    pub(crate) steps: Vec<WorkflowRunStepRecord>,
    #[serde(default)]
    pub(crate) error: Option<String>,
    pub(crate) result_text: String,
}

#[derive(Debug, Clone)]
struct WorkflowRunInvocation {
    workflow_name: Option<String>,
    requested_script_path: Option<String>,
    shared_context: Option<String>,
    continue_on_error: Option<bool>,
}

impl WorkflowRunInvocation {
    fn from_target(
        target: &WorkflowRunTarget,
        shared_context: Option<String>,
        continue_on_error: Option<bool>,
    ) -> Self {
        match target {
            WorkflowRunTarget::Named(name) => Self {
                workflow_name: normalize_optional_text(Some(name.clone())),
                requested_script_path: None,
                shared_context: normalize_optional_text(shared_context),
                continue_on_error,
            },
            WorkflowRunTarget::Path(path) => Self {
                workflow_name: None,
                requested_script_path: Some(path_text(path)),
                shared_context: normalize_optional_text(shared_context),
                continue_on_error,
            },
        }
    }
}

pub(crate) async fn execute_and_record_workflow(
    session: &AgentSession,
    target: WorkflowRunTarget,
    shared_context: Option<String>,
    continue_on_error: Option<bool>,
) -> Result<String> {
    let invocation =
        WorkflowRunInvocation::from_target(&target, shared_context.clone(), continue_on_error);
    let started_at = unix_timestamp();
    match execute_workflow(session, target, shared_context, continue_on_error).await {
        Ok(result_text) => {
            let record =
                build_success_record(invocation, started_at, unix_timestamp(), &result_text);
            save_workflow_run(session.working_directory(), &record)?;
            render_recorded_workflow_output(session.working_directory(), &record, &result_text)
        }
        Err(error) => {
            let error_text = error.to_string();
            let record =
                build_failure_record(invocation, started_at, unix_timestamp(), &error_text);
            save_workflow_run(session.working_directory(), &record)?;
            Err(anyhow!(
                "{}\nworkflow run recorded at `{}`",
                error_text,
                path_text(&workflow_run_path(
                    session.working_directory(),
                    &record.run_id
                ))
            ))
        }
    }
}

fn build_success_record(
    invocation: WorkflowRunInvocation,
    started_at: u64,
    finished_at: u64,
    result_text: &str,
) -> WorkflowRunRecord {
    let parsed = serde_json::from_str::<Value>(result_text).ok();
    let workflow_source = parsed
        .as_ref()
        .and_then(|value| optional_string(value.get("workflow_source")));
    let workflow_name = invocation.workflow_name.or_else(|| {
        workflow_source
            .as_deref()
            .and_then(derive_workflow_name_from_source)
    });
    let steps = parsed
        .as_ref()
        .and_then(|value| value.get("steps"))
        .and_then(Value::as_array)
        .map(|steps| steps.iter().map(step_record_from_value).collect::<Vec<_>>())
        .unwrap_or_default();
    let summary = parsed
        .as_ref()
        .and_then(|value| value.get("summary"))
        .map(summary_from_value)
        .unwrap_or_else(|| summary_from_steps(&steps));

    WorkflowRunRecord {
        run_id: next_run_id(),
        status: parsed
            .as_ref()
            .and_then(|value| optional_string(value.get("status")))
            .unwrap_or_else(|| "completed".to_string()),
        workflow_name,
        workflow_source,
        requested_script_path: invocation.requested_script_path,
        started_at,
        finished_at,
        shared_context: invocation.shared_context,
        continue_on_error: invocation.continue_on_error,
        summary,
        steps,
        error: None,
        result_text: result_text.to_string(),
    }
}

fn build_failure_record(
    invocation: WorkflowRunInvocation,
    started_at: u64,
    finished_at: u64,
    error_text: &str,
) -> WorkflowRunRecord {
    let requested_script_path = invocation.requested_script_path;
    WorkflowRunRecord {
        run_id: next_run_id(),
        status: "failed".to_string(),
        workflow_name: invocation.workflow_name,
        workflow_source: requested_script_path.clone(),
        requested_script_path,
        started_at,
        finished_at,
        shared_context: invocation.shared_context,
        continue_on_error: invocation.continue_on_error,
        summary: WorkflowRunSummary::default(),
        steps: Vec::new(),
        error: Some(error_text.to_string()),
        result_text: error_text.to_string(),
    }
}

fn step_record_from_value(value: &Value) -> WorkflowRunStepRecord {
    WorkflowRunStepRecord {
        name: optional_string(value.get("name")).unwrap_or_else(|| "(unnamed)".to_string()),
        status: optional_string(value.get("status")).unwrap_or_else(|| "unknown".to_string()),
        result_text: value
            .get("result")
            .and_then(|result| result.get("result"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        error: optional_string(value.get("error")),
        reason: optional_string(value.get("reason")),
    }
}

fn summary_from_value(value: &Value) -> WorkflowRunSummary {
    WorkflowRunSummary {
        total_steps: optional_usize(value.get("total_steps")).unwrap_or_default(),
        completed_steps: optional_usize(value.get("completed_steps")).unwrap_or_default(),
        failed_steps: optional_usize(value.get("failed_steps")).unwrap_or_default(),
        running_steps: optional_usize(value.get("running_steps")).unwrap_or_default(),
        skipped_steps: optional_usize(value.get("skipped_steps")).unwrap_or_default(),
    }
}

fn summary_from_steps(steps: &[WorkflowRunStepRecord]) -> WorkflowRunSummary {
    let mut summary = WorkflowRunSummary {
        total_steps: steps.len(),
        ..WorkflowRunSummary::default()
    };
    for step in steps {
        match step.status.as_str() {
            "completed" | "coordinated" => summary.completed_steps += 1,
            "failed" => summary.failed_steps += 1,
            "running" => summary.running_steps += 1,
            "skipped" => summary.skipped_steps += 1,
            _ => {}
        }
    }
    summary
}

pub(super) fn derive_workflow_name_from_source(source: &str) -> Option<String> {
    let normalized = source.replace('\\', "/");
    let marker = normalized.find(WORKFLOW_SCRIPT_PREFIX)?;
    let relative = &normalized[marker + WORKFLOW_SCRIPT_PREFIX.len()..];
    let workflow_name = relative.trim_end_matches(".json").trim();
    (!workflow_name.is_empty()).then_some(workflow_name.to_string())
}

pub(super) fn workflow_runs_root(root: &Path) -> PathBuf {
    root.join(".hellox").join("workflow-runs")
}

pub(super) fn workflow_run_path(root: &Path, run_id: &str) -> PathBuf {
    workflow_runs_root(root).join(format!("{run_id}.json"))
}

fn next_run_id() -> String {
    format!("run-{}", unix_timestamp_nanos())
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn optional_usize(value: Option<&Value>) -> Option<usize> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

pub(super) fn normalize_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn normalize_required_text(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        Err(anyhow!("{label} cannot be empty"))
    } else {
        Ok(value.to_string())
    }
}

pub(super) fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn unix_timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
