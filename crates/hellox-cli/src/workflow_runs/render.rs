use std::path::Path;

use anyhow::{Context, Result};
use hellox_tui::{render_panel, render_table, status_badge, KeyValueRow, PanelSection, Table};
use serde_json::Value;

#[path = "render_inspect.rs"]
mod inspect;

use super::selector::render_run_selector;
use super::{
    normalize_filter, path_text, workflow_run_path, workflow_runs_root, WorkflowRunRecord,
};

pub(crate) fn render_workflow_run_list(
    root: &Path,
    records: &[WorkflowRunRecord],
    workflow_name: Option<&str>,
) -> String {
    if records.is_empty() {
        return match normalize_filter(workflow_name) {
            Some(name) => format!(
                "No workflow runs found for `{name}` under `{}`.",
                path_text(&workflow_runs_root(root))
            ),
            None => format!(
                "No workflow runs found under `{}`.",
                path_text(&workflow_runs_root(root))
            ),
        };
    }

    let filter = normalize_filter(workflow_name);
    let mut metadata = vec![
        KeyValueRow::new("root", path_text(&workflow_runs_root(root))),
        KeyValueRow::new("runs", records.len().to_string()),
    ];
    if let Some(name) = filter.as_deref() {
        metadata.push(KeyValueRow::new("workflow", name));
    }

    let mut sections = vec![PanelSection::new(
        "Recorded runs",
        render_table(&build_run_history_table(records)),
    )];
    sections.push(PanelSection::new(
        "Recent run selector",
        render_run_selector(records),
    ));

    let mut palette = vec!["- inspect one run: `hellox workflow show-run <run-id>`".to_string()];
    if let Some(name) = filter.as_deref() {
        palette.push(format!("- latest run: `hellox workflow last-run {name}`"));
        palette.push(format!("- inspect script: `hellox workflow panel {name}`"));
        palette.push(format!("- repl history: `/workflow runs {name}`"));
    } else {
        palette.push("- focus one workflow: `hellox workflow runs <name>`".to_string());
        palette.push("- repl history: `/workflow runs <name>`".to_string());
    }
    sections.push(PanelSection::new("Action palette", palette));

    render_panel("Workflow run history panel", &metadata, &sections)
}

pub(crate) fn render_workflow_run_inspect_panel(root: &Path, record: &WorkflowRunRecord) -> String {
    inspect::render_workflow_run_inspect_panel(root, record, None)
}

pub(crate) fn render_workflow_run_inspect_panel_with_step(
    root: &Path,
    record: &WorkflowRunRecord,
    step_number: Option<usize>,
) -> String {
    inspect::render_workflow_run_inspect_panel(root, record, step_number)
}

fn build_run_history_table(records: &[WorkflowRunRecord]) -> Table {
    let rows = records
        .iter()
        .map(|record| {
            let workflow = record.workflow_name.as_deref().unwrap_or("(custom path)");
            let source = record
                .workflow_source
                .as_deref()
                .or(record.requested_script_path.as_deref())
                .unwrap_or("(inline)");
            vec![
                record.run_id.clone(),
                status_badge(&record.status),
                workflow.to_string(),
                record.finished_at.to_string(),
                compact_summary(record),
                preview_text(record.shared_context.as_deref().unwrap_or("(none)")),
                preview_text(source),
                format!("hellox workflow show-run {}", record.run_id),
                next_follow_up_hint(record),
            ]
        })
        .collect();
    Table::new(
        vec![
            "run_id".to_string(),
            "status".to_string(),
            "workflow".to_string(),
            "finished_at".to_string(),
            "summary".to_string(),
            "shared_context".to_string(),
            "source".to_string(),
            "open".to_string(),
            "next".to_string(),
        ],
        rows,
    )
}

pub(super) fn render_recorded_workflow_output(
    root: &Path,
    record: &WorkflowRunRecord,
    result_text: &str,
) -> Result<String> {
    match serde_json::from_str::<Value>(result_text) {
        Ok(Value::Object(mut document)) => {
            document.insert("run_id".to_string(), Value::String(record.run_id.clone()));
            document.insert(
                "run_record".to_string(),
                Value::String(path_text(&workflow_run_path(root, &record.run_id))),
            );
            serde_json::to_string_pretty(&Value::Object(document))
                .context("failed to render recorded workflow result")
        }
        _ => Ok(format!(
            "Workflow run `{}` recorded at `{}`.\n\n{}",
            record.run_id,
            path_text(&workflow_run_path(root, &record.run_id)),
            result_text
        )),
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

pub(super) fn compact_summary(record: &WorkflowRunRecord) -> String {
    format!(
        "c{}/f{}/r{}/s{}",
        record.summary.completed_steps,
        record.summary.failed_steps,
        record.summary.running_steps,
        record.summary.skipped_steps
    )
}

fn next_follow_up_hint(record: &WorkflowRunRecord) -> String {
    if let Some(workflow_name) = record.workflow_name.as_deref() {
        format!("hellox workflow last-run {workflow_name}")
    } else if let Some(script_path) = record.requested_script_path.as_deref() {
        format!("hellox workflow run --script-path {script_path}")
    } else {
        "(inspect run)".to_string()
    }
}

#[cfg(test)]
mod tests;
