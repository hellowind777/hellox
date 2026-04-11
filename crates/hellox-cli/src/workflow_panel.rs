use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_tui::{
    render_panel, render_selector, status_badge, KeyValueRow, PanelSection, SelectorEntry,
};

#[path = "workflow_panel_focus.rs"]
mod focus;

use crate::workflow_runs::WorkflowRunRecord;
use crate::workflow_runs::{list_workflow_runs, WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT};
use crate::workflows::{
    list_workflows, load_named_workflow_detail, render_workflow_list, WorkflowScriptDetail,
    WorkflowScriptSummary, WorkflowStepSummary,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowPanelSelectionItem {
    Step(usize),
    Run(String),
}

pub(crate) fn render_workflow_panel(
    root: &Path,
    workflow_name: Option<&str>,
    step_number: Option<usize>,
) -> Result<String> {
    let Some(workflow_name) = workflow_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return render_workflow_panel_selector(root);
    };

    let detail = load_named_workflow_detail(root, workflow_name)?;
    focus::render_workflow_panel_detail(root, &detail, step_number)
}

pub(crate) fn render_workflow_panel_detail(
    root: &Path,
    detail: &WorkflowScriptDetail,
    step_number: Option<usize>,
) -> Result<String> {
    focus::render_workflow_panel_detail(root, detail, step_number)
}

pub(crate) fn list_workflow_panel_selection_items(
    root: &Path,
    workflow_name: &str,
) -> Result<Vec<WorkflowPanelSelectionItem>> {
    let detail = load_named_workflow_detail(root, workflow_name)?;
    let mut items = (1..=detail.steps.len())
        .map(WorkflowPanelSelectionItem::Step)
        .collect::<Vec<_>>();
    let runs = list_workflow_runs(
        root,
        Some(&detail.summary.name),
        WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
    )?;
    items.extend(
        runs.into_iter()
            .map(|record| WorkflowPanelSelectionItem::Run(record.run_id)),
    );
    Ok(items)
}

fn render_workflow_panel_selector(root: &Path) -> Result<String> {
    let workflows = list_workflows(root)?;
    if workflows.is_empty() {
        return Ok(format!(
            "No workflow scripts found under `{}`.\nCreate one with `hellox workflow init <name>` or `/workflow init <name>`.",
            path_text(&root.join(".hellox").join("workflows"))
        ));
    }

    let root_path = root.join(".hellox").join("workflows");
    let metadata = vec![
        KeyValueRow::new("root", path_text(&root_path)),
        KeyValueRow::new("workflows", workflows.len().to_string()),
    ];

    let mut sections = vec![
        PanelSection::new(
            "Workflows",
            render_selector(&build_workflow_selector_entries(&workflows)),
        ),
        PanelSection::new(
            "Action palette",
            vec![
                "- open one panel: `hellox workflow panel <name>`".to_string(),
                "- open in repl: `/workflow panel <name>`".to_string(),
                "- validate all scripts: `hellox workflow validate`".to_string(),
                "- scaffold a new script: `hellox workflow init <name>`".to_string(),
            ],
        ),
    ];
    if workflows
        .iter()
        .any(|workflow| workflow.validation_error.is_some())
    {
        sections.push(PanelSection::new(
            "Notes",
            vec![
                "Invalid scripts appear above; run `hellox workflow validate` before opening a focused panel.".to_string(),
                String::new(),
                render_workflow_list(root, &workflows),
            ],
        ));
    }

    Ok(render_panel(
        "Workflow authoring panel selector",
        &metadata,
        &sections,
    ))
}

fn build_workflow_selector_entries(workflows: &[WorkflowScriptSummary]) -> Vec<SelectorEntry> {
    workflows
        .iter()
        .map(|workflow| {
            let mut lines = vec![
                format!("steps: {}", workflow.step_count),
                format!("continue_on_error: {}", workflow.continue_on_error),
                format!(
                    "shared_context: {}",
                    workflow.shared_context.as_deref().unwrap_or("(none)")
                ),
                format!("dynamic_command: {}", dynamic_command_hint(workflow)),
                format!("open: `hellox workflow panel {}`", workflow.name),
                format!("repl: `/workflow panel {}`", workflow.name),
            ];
            if let Some(error) = &workflow.validation_error {
                lines.push(format!("validation_error: {error}"));
            }

            SelectorEntry::new(workflow.name.clone(), lines).with_badge(
                if workflow.validation_error.is_some() {
                    status_badge("invalid")
                } else {
                    status_badge("valid")
                },
            )
        })
        .collect()
}

pub(super) fn latest_step_status(
    step: &WorkflowStepSummary,
    latest_run: Option<&WorkflowRunRecord>,
) -> String {
    let Some(latest_run) = latest_run else {
        return "(none)".to_string();
    };
    let Some(name) = step.name.as_deref() else {
        return "(unnamed)".to_string();
    };

    latest_run
        .steps
        .iter()
        .find(|record| record.name.eq_ignore_ascii_case(name))
        .map(|record| record.status.clone())
        .unwrap_or_else(|| "(not recorded)".to_string())
}

pub(super) fn latest_run_summary(record: Option<&WorkflowRunRecord>) -> String {
    match record {
        Some(record) => format!(
            "{} (`{}`, c{}/f{}/r{}/s{})",
            record.status,
            record.run_id,
            record.summary.completed_steps,
            record.summary.failed_steps,
            record.summary.running_steps,
            record.summary.skipped_steps
        ),
        None => "(none recorded yet)".to_string(),
    }
}

pub(super) fn dynamic_command_hint(summary: &WorkflowScriptSummary) -> String {
    if summary.dynamic_command && summary.validation_error.is_none() {
        format!("/{} [shared_context]", summary.name)
    } else {
        format!("/workflow run {} [shared_context]", summary.name)
    }
}

pub(super) fn validate_step_number(step_number: usize, step_count: usize) -> Result<()> {
    if step_number == 0 || step_number > step_count {
        return Err(anyhow!(
            "workflow panel step `{step_number}` is out of range; expected 1..={step_count}"
        ));
    }
    Ok(())
}

pub(super) fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

pub(super) fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests;
