use std::path::Path;

use hellox_tui::{
    render_panel, render_selector, render_table, status_badge, KeyValueRow, PanelSection,
    SelectorEntry, Table,
};

use super::super::{path_text, workflow_run_path, WorkflowRunRecord};
use super::{compact_summary, preview_text};
use crate::workflow_runs::selector::render_step_lens;

pub(super) fn render_workflow_run_inspect_panel(
    root: &Path,
    record: &WorkflowRunRecord,
    step_number: Option<usize>,
) -> String {
    let continue_on_error = record
        .continue_on_error
        .map(|value| value.to_string())
        .unwrap_or_else(|| "(script default)".to_string());
    let mut metadata = vec![
        KeyValueRow::new(
            "record_path",
            path_text(&workflow_run_path(root, &record.run_id)),
        ),
        KeyValueRow::new(
            "workflow",
            record.workflow_name.as_deref().unwrap_or("(custom path)"),
        ),
        KeyValueRow::new(
            "workflow_source",
            record.workflow_source.as_deref().unwrap_or("(unknown)"),
        ),
        KeyValueRow::new(
            "requested_script_path",
            record.requested_script_path.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new("status", status_badge(&record.status)),
        KeyValueRow::new("started_at", record.started_at.to_string()),
        KeyValueRow::new("finished_at", record.finished_at.to_string()),
        KeyValueRow::new(
            "shared_context",
            record.shared_context.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new("continue_on_error_override", continue_on_error),
        KeyValueRow::new("summary", compact_summary(record)),
    ];

    if let Some(error) = &record.error {
        metadata.push(KeyValueRow::new("error", preview_text(error)));
    }
    if !record.result_text.trim().is_empty() {
        metadata.push(KeyValueRow::new(
            "result_preview",
            preview_text(&record.result_text),
        ));
    }

    let mut sections = vec![
        PanelSection::new(
            "Visual execution map",
            render_table(&build_execution_table(record)),
        ),
        PanelSection::new("Step selector", render_step_selector(record, step_number)),
        PanelSection::new(
            if step_number.is_some() {
                "Focused step lens"
            } else {
                "Primary step lens"
            },
            render_step_lens(record, step_number),
        ),
        PanelSection::new("CLI palette", render_cli_palette(root, record)),
    ];
    let details = render_execution_details(record);
    if !details.is_empty() {
        sections.push(PanelSection::new("Execution details", details));
    }
    let repl_palette = render_repl_palette(record);
    if !repl_palette.is_empty() {
        sections.push(PanelSection::new("REPL palette", repl_palette));
    }

    render_panel(
        &format!("Workflow run inspect panel: {}", record.run_id),
        &metadata,
        &sections,
    )
}

fn build_execution_table(record: &WorkflowRunRecord) -> Table {
    let rows = record
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            vec![
                (index + 1).to_string(),
                status_badge(&step.status),
                step.name.clone(),
                step.result_text
                    .as_ref()
                    .map(|text| text.chars().count().to_string())
                    .unwrap_or_else(|| "-".to_string()),
                yes_no(step.error.is_some()).to_string(),
                yes_no(step.reason.is_some()).to_string(),
            ]
        })
        .collect();
    Table::new(
        vec![
            "#".to_string(),
            "status".to_string(),
            "step".to_string(),
            "result_chars".to_string(),
            "error".to_string(),
            "reason".to_string(),
        ],
        rows,
    )
}

fn render_step_selector(record: &WorkflowRunRecord, step_number: Option<usize>) -> Vec<String> {
    let selected_step = select_step_number(record, step_number);
    let entries = record
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let lines = vec![
                format!("status: {}", status_badge(&step.status)),
                format!(
                    "result_chars: {}",
                    step.result_text
                        .as_ref()
                        .map(|text| text.chars().count().to_string())
                        .unwrap_or_else(|| "-".to_string())
                ),
                format!("has_error: {}", yes_no(step.error.is_some())),
                format!("has_reason: {}", yes_no(step.reason.is_some())),
                format!(
                    "focus: `/workflow show-run {} {}`",
                    record.run_id,
                    index + 1
                ),
            ];
            SelectorEntry::new(step.name.clone(), lines)
                .with_badge(status_badge(&step.status))
                .selected(selected_step == Some(index + 1))
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

fn render_execution_details(record: &WorkflowRunRecord) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, step) in record.steps.iter().enumerate() {
        let mut detail_lines = Vec::new();
        if let Some(reason) = &step.reason {
            detail_lines.push(format!("reason: {}", preview_text(reason)));
        }
        if let Some(error) = &step.error {
            detail_lines.push(format!("error: {}", preview_text(error)));
        }
        if let Some(result_text) = &step.result_text {
            detail_lines.push(format!("result: {}", preview_text(result_text)));
        }
        if detail_lines.is_empty() {
            continue;
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("[{}] {}", index + 1, step.name));
        lines.extend(detail_lines.into_iter().map(|line| format!("  {line}")));
    }
    lines
}

fn render_cli_palette(root: &Path, record: &WorkflowRunRecord) -> Vec<String> {
    let mut lines = vec![format!(
        "- open raw record: `{}`",
        path_text(&workflow_run_path(root, &record.run_id))
    )];

    if let Some(workflow_name) = record.workflow_name.as_deref() {
        lines.push(format!(
            "- rerun: `{}`",
            cli_run_command(Some(workflow_name), None, record.shared_context.as_deref())
        ));
        lines.push(format!(
            "- inspect script: `hellox workflow panel {workflow_name}`"
        ));
        lines.push(format!(
            "- inspect history: `hellox workflow runs {workflow_name}`"
        ));
        lines.push(format!(
            "- latest run: `hellox workflow last-run {workflow_name}`"
        ));
    } else if let Some(script_path) = record.requested_script_path.as_deref() {
        lines.push(format!(
            "- rerun: `{}`",
            cli_run_command(None, Some(script_path), record.shared_context.as_deref())
        ));
        lines.push(format!(
            "- inspect script: `hellox workflow panel --script-path {script_path}`"
        ));
        lines.push(format!(
            "- validate script: `hellox workflow validate --script-path {script_path}`"
        ));
    }

    lines
}

fn render_repl_palette(record: &WorkflowRunRecord) -> Vec<String> {
    let Some(workflow_name) = record.workflow_name.as_deref() else {
        return Vec::new();
    };

    let mut lines = vec![format!(
        "- rerun: `{}`",
        repl_run_command(workflow_name, record.shared_context.as_deref())
    )];
    lines.push(format!(
        "- inspect script: `/workflow panel {workflow_name}`"
    ));
    lines.push(format!(
        "- inspect history: `/workflow runs {workflow_name}`"
    ));
    lines.push(format!(
        "- latest run: `/workflow last-run {workflow_name}`"
    ));
    lines
}

fn cli_run_command(
    workflow_name: Option<&str>,
    script_path: Option<&str>,
    shared_context: Option<&str>,
) -> String {
    let mut command = match (workflow_name, script_path) {
        (Some(workflow_name), _) => format!("hellox workflow run {workflow_name}"),
        (None, Some(script_path)) => {
            format!("hellox workflow run --script-path {script_path}")
        }
        (None, None) => "hellox workflow run".to_string(),
    };
    if let Some(shared_context) = shared_context.filter(|value| !value.trim().is_empty()) {
        command.push_str(&format!(
            " --shared-context \"{}\"",
            shared_context.replace('"', "\\\"")
        ));
    }
    command
}

fn repl_run_command(workflow_name: &str, shared_context: Option<&str>) -> String {
    let mut command = format!("/workflow run {workflow_name}");
    if let Some(shared_context) = shared_context.filter(|value| !value.trim().is_empty()) {
        command.push(' ');
        command.push_str(shared_context);
    }
    command
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn select_step_number(record: &WorkflowRunRecord, step_number: Option<usize>) -> Option<usize> {
    if let Some(step_number) = step_number {
        return (step_number > 0 && step_number <= record.steps.len()).then_some(step_number);
    }

    record
        .steps
        .iter()
        .enumerate()
        .find(|(_, step)| step.status.eq_ignore_ascii_case("failed"))
        .map(|(index, _)| index + 1)
        .or_else(|| {
            record
                .steps
                .iter()
                .enumerate()
                .find(|(_, step)| step.status.eq_ignore_ascii_case("running"))
                .map(|(index, _)| index + 1)
        })
        .or_else(|| (!record.steps.is_empty()).then_some(1))
}
