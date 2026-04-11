use std::path::Path;

use hellox_tui::{
    render_panel, render_selector_with_start, status_badge, KeyValueRow, PanelSection,
    SelectorEntry,
};

use crate::workflow_runs::WorkflowRunRecord;
use crate::workflows::WorkflowScriptSummary;

use super::{
    collect_custom_runs, compact_summary, dynamic_command_hint, find_latest_run,
    latest_run_summary, path_text, preview_text, script_state_label, CUSTOM_RUN_PREVIEW_LIMIT,
};

pub(super) fn render_workflow_selector(
    root: &Path,
    workflows: &[WorkflowScriptSummary],
    runs: &[WorkflowRunRecord],
) -> String {
    let workflow_root = root.join(".hellox").join("workflows");
    if workflows.is_empty() && runs.is_empty() {
        return format!(
            "No workflow scripts or recorded runs found under `{}`.",
            path_text(&workflow_root)
        );
    }

    let metadata = vec![
        KeyValueRow::new("root", path_text(&workflow_root)),
        KeyValueRow::new("workflows", workflows.len().to_string()),
        KeyValueRow::new("recorded_runs", runs.len().to_string()),
    ];
    let mut sections = Vec::new();

    if workflows.is_empty() {
        sections.push(PanelSection::new(
            "Workflows",
            vec!["No project workflow scripts found.".to_string()],
        ));
    } else {
        let entries = workflows
            .iter()
            .map(|workflow| build_workflow_entry(workflow, find_latest_run(runs, &workflow.name)))
            .collect::<Vec<_>>();
        sections.push(PanelSection::new(
            "Workflows",
            render_selector_with_start(&entries, 1),
        ));
    }

    let custom_runs = collect_custom_runs(workflows, runs);
    if !custom_runs.is_empty() {
        let entries = custom_runs
            .into_iter()
            .take(CUSTOM_RUN_PREVIEW_LIMIT)
            .map(build_custom_run_entry)
            .collect::<Vec<_>>();
        sections.push(PanelSection::new(
            "Custom-path runs",
            render_selector_with_start(&entries, workflows.len() + 1),
        ));
    }

    sections.push(PanelSection::new(
        "Action palette",
        vec![
            "- focus one workflow: `hellox workflow overview <name>`".to_string(),
            "- open authoring: `hellox workflow panel <name>`".to_string(),
            "- inspect a run: `hellox workflow show-run <run-id>`".to_string(),
            "- repl focus: `/workflow overview <name>`".to_string(),
        ],
    ));

    render_panel("Workflow overview selector", &metadata, &sections)
}

fn build_workflow_entry(
    workflow: &WorkflowScriptSummary,
    latest_run: Option<&WorkflowRunRecord>,
) -> SelectorEntry {
    let mut lines = vec![
        format!("steps: {}", workflow.step_count),
        format!("continue_on_error: {}", workflow.continue_on_error),
        format!(
            "shared_context: {}",
            workflow.shared_context.as_deref().unwrap_or("(none)")
        ),
        format!("dynamic_command: {}", dynamic_command_hint(workflow)),
        format!("path: {}", path_text(&workflow.path)),
        format!("latest_run: {}", latest_run_summary(latest_run)),
        format!("overview: `hellox workflow overview {}`", workflow.name),
        format!("panel: `hellox workflow panel {}`", workflow.name),
        format!("runs: `hellox workflow runs {}`", workflow.name),
        format!("overview (repl): `/workflow overview {}`", workflow.name),
    ];
    if let Some(error) = &workflow.validation_error {
        lines.push(format!("validation_error: {}", preview_text(error)));
    }

    SelectorEntry::new(workflow.name.clone(), lines)
        .with_badge(status_badge(script_state_label(workflow)))
}

fn build_custom_run_entry(record: &WorkflowRunRecord) -> SelectorEntry {
    let source = record
        .requested_script_path
        .as_deref()
        .or(record.workflow_source.as_deref())
        .unwrap_or("(unknown)");
    let mut lines = vec![
        format!("finished_at: {}", record.finished_at),
        format!("summary: {}", compact_summary(&record.summary)),
        format!("source: {source}"),
        format!("inspect: `hellox workflow show-run {}`", record.run_id),
    ];
    if let Some(script_path) = record.requested_script_path.as_deref() {
        lines.push(format!(
            "rerun: `hellox workflow run --script-path {script_path}`"
        ));
        lines.push(format!(
            "validate: `hellox workflow validate --script-path {script_path}`"
        ));
        lines.push(format!(
            "rerun (repl): `/workflow run --script-path {script_path}`"
        ));
        lines.push(format!(
            "show (repl): `/workflow show --script-path {script_path}`"
        ));
    }

    SelectorEntry::new(record.run_id.clone(), lines).with_badge(status_badge(&record.status))
}
