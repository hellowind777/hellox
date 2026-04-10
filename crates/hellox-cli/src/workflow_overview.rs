use std::path::Path;

use anyhow::Result;
use hellox_tui::status_badge;

use crate::workflow_runs::{
    derive_workflow_name_from_source, list_workflow_runs, WorkflowRunRecord, WorkflowRunSummary,
};
use crate::workflows::{list_workflows, WorkflowScriptSummary};

const CUSTOM_RUN_PREVIEW_LIMIT: usize = 5;

#[path = "workflow_overview_focus.rs"]
mod focus;

#[path = "workflow_overview_selector.rs"]
mod selector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowOverviewSelectionItem {
    Workflow(String),
    Run(String),
}

pub(crate) fn render_workflow_overview(root: &Path, workflow_name: Option<&str>) -> Result<String> {
    let workflows = list_workflows(root)?;
    let runs = list_workflow_runs(root, None, usize::MAX)?;
    match normalize_filter(workflow_name) {
        Some(name) => focus::render_workflow_focus(root, &workflows, &runs, &name),
        None => Ok(selector::render_workflow_selector(root, &workflows, &runs)),
    }
}

pub(crate) fn list_workflow_overview_selection_items(
    root: &Path,
) -> Result<Vec<WorkflowOverviewSelectionItem>> {
    let workflows = list_workflows(root)?;
    let runs = list_workflow_runs(root, None, usize::MAX)?;
    Ok(build_workflow_overview_selection_items(&workflows, &runs))
}

fn build_workflow_overview_selection_items(
    workflows: &[WorkflowScriptSummary],
    runs: &[WorkflowRunRecord],
) -> Vec<WorkflowOverviewSelectionItem> {
    let mut items = workflows
        .iter()
        .map(|workflow| WorkflowOverviewSelectionItem::Workflow(workflow.name.clone()))
        .collect::<Vec<_>>();
    items.extend(
        collect_custom_runs(workflows, runs)
            .into_iter()
            .take(CUSTOM_RUN_PREVIEW_LIMIT)
            .map(|record| WorkflowOverviewSelectionItem::Run(record.run_id.clone())),
    );
    items
}

pub(super) fn find_latest_run<'a>(
    runs: &'a [WorkflowRunRecord],
    workflow_name: &str,
) -> Option<&'a WorkflowRunRecord> {
    runs.iter()
        .find(|record| matches_workflow_name(record, workflow_name))
}

fn matches_workflow_name(record: &WorkflowRunRecord, workflow_name: &str) -> bool {
    record
        .workflow_name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case(workflow_name))
        || record
            .workflow_source
            .as_deref()
            .and_then(derive_workflow_name_from_source)
            .is_some_and(|name| name.eq_ignore_ascii_case(workflow_name))
}

fn collect_custom_runs<'a>(
    workflows: &[WorkflowScriptSummary],
    runs: &'a [WorkflowRunRecord],
) -> Vec<&'a WorkflowRunRecord> {
    let known_names = workflows
        .iter()
        .map(|workflow| workflow.name.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>();

    runs.iter()
        .filter(|record| {
            let workflow_name = record
                .workflow_name
                .as_deref()
                .map(str::to_ascii_lowercase)
                .or_else(|| {
                    record
                        .workflow_source
                        .as_deref()
                        .and_then(derive_workflow_name_from_source)
                        .map(|name| name.to_ascii_lowercase())
                });
            match workflow_name.as_ref() {
                None => true,
                Some(name) => !known_names.contains(name),
            }
        })
        .collect()
}

pub(super) fn latest_run_summary(record: Option<&WorkflowRunRecord>) -> String {
    match record {
        Some(record) => format!(
            "{} (`{}`, finished_at: {}, {})",
            status_badge(&record.status),
            record.run_id,
            record.finished_at,
            compact_summary(&record.summary)
        ),
        None => "(none recorded yet)".to_string(),
    }
}

pub(super) fn compact_summary(summary: &WorkflowRunSummary) -> String {
    format!(
        "c{}/f{}/r{}/s{}",
        summary.completed_steps, summary.failed_steps, summary.running_steps, summary.skipped_steps
    )
}

pub(super) fn script_state_label(workflow: &WorkflowScriptSummary) -> &'static str {
    if workflow.validation_error.is_some() {
        "invalid"
    } else {
        "valid"
    }
}

pub(super) fn dynamic_command_hint(workflow: &WorkflowScriptSummary) -> String {
    if workflow.dynamic_command && workflow.validation_error.is_none() {
        format!("/{} [shared_context]", workflow.name)
    } else {
        format!("/workflow run {} [shared_context]", workflow.name)
    }
}

pub(super) fn preview_text(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 160 {
        compact
    } else {
        format!("{}...", compact.chars().take(157).collect::<String>())
    }
}

pub(super) fn latest_step_status(
    step_name: Option<&str>,
    latest_run: Option<&WorkflowRunRecord>,
) -> String {
    let Some(step_name) = step_name else {
        return "(unnamed)".to_string();
    };
    let Some(latest_run) = latest_run else {
        return "(none)".to_string();
    };

    latest_run
        .steps
        .iter()
        .find(|record| record.name.eq_ignore_ascii_case(step_name))
        .map(|record| status_badge(&record.status))
        .unwrap_or_else(|| "(not recorded)".to_string())
}

fn normalize_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests;

pub(super) fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}
