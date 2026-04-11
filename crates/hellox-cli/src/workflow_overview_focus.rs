use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_tui::{
    render_panel, render_selector, status_badge, KeyValueRow, PanelSection, SelectorEntry,
};

use crate::workflow_runs::{
    list_workflow_runs, render_run_selector_with_start, WorkflowRunRecord, WorkflowRunStepRecord,
    WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
};
use crate::workflows::{
    load_workflow_detail_from_path, WorkflowRunTarget, WorkflowScriptDetail, WorkflowScriptSummary,
};

use super::{
    compact_summary, dynamic_command_hint, find_latest_run, latest_run_summary, latest_step_status,
    path_text, preview_text, script_state_label, yes_no,
};

pub(super) fn render_workflow_focus(
    root: &Path,
    workflows: &[WorkflowScriptSummary],
    runs: &[WorkflowRunRecord],
    workflow_name: &str,
) -> Result<String> {
    let workflow = workflows
        .iter()
        .find(|workflow| workflow.name.eq_ignore_ascii_case(workflow_name))
        .ok_or_else(|| {
            anyhow!(
                "workflow `{workflow_name}` was not found under `{}`",
                path_text(&root.join(".hellox").join("workflows"))
            )
        })?;

    let latest_run = find_latest_run(runs, &workflow.name);
    let run_target = WorkflowRunTarget::Named(workflow.name.clone());
    let recent_runs =
        list_workflow_runs(root, Some(&run_target), WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT)?;
    let metadata = vec![
        KeyValueRow::new("path", path_text(&workflow.path)),
        KeyValueRow::new("status", status_badge(script_state_label(workflow))),
        KeyValueRow::new("steps", workflow.step_count.to_string()),
        KeyValueRow::new("continue_on_error", workflow.continue_on_error.to_string()),
        KeyValueRow::new(
            "shared_context",
            workflow.shared_context.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new("dynamic_command", dynamic_command_hint(workflow)),
        KeyValueRow::new("latest_run", latest_run_summary(latest_run)),
    ];
    let mut sections = Vec::new();

    if let Some(error) = &workflow.validation_error {
        sections.push(PanelSection::new("Validation", vec![preview_text(error)]));
        if !recent_runs.is_empty() {
            sections.push(PanelSection::new(
                "Recent runs",
                render_run_selector_with_start(&recent_runs, 1),
            ));
        }
    } else {
        let detail =
            load_workflow_detail_from_path(root, &workflow.path, Some(workflow.name.clone()))?;
        sections.push(PanelSection::new(
            "Visual script map",
            render_focus_script_map(&detail, latest_run),
        ));
        sections.push(PanelSection::new(
            "Step selector",
            render_focus_step_selector(&detail, &run_target, latest_run),
        ));
        if !recent_runs.is_empty() {
            sections.push(PanelSection::new(
                "Recent runs",
                render_run_selector_with_start(&recent_runs, detail.steps.len() + 1),
            ));
        }
        sections.push(PanelSection::new(
            "Latest run snapshot",
            render_latest_run_snapshot(latest_run),
        ));
    }

    sections.push(PanelSection::new(
        "CLI palette",
        render_focus_cli_palette(workflow),
    ));
    sections.push(PanelSection::new(
        "REPL palette",
        render_focus_repl_palette(workflow),
    ));

    Ok(render_panel(
        &format!("Workflow overview: {}", workflow.name),
        &metadata,
        &sections,
    ))
}

pub(super) fn render_workflow_focus_for_path(
    root: &Path,
    detail: &WorkflowScriptDetail,
    script_path: &Path,
) -> Result<String> {
    let run_target = WorkflowRunTarget::Path(script_path.to_path_buf());
    let recent_runs =
        list_workflow_runs(root, Some(&run_target), WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT)?;
    let latest_run = recent_runs.first();
    let metadata = vec![
        KeyValueRow::new("path", path_text(&detail.summary.path)),
        KeyValueRow::new("status", status_badge(script_state_label(&detail.summary))),
        KeyValueRow::new("steps", detail.summary.step_count.to_string()),
        KeyValueRow::new(
            "continue_on_error",
            detail.summary.continue_on_error.to_string(),
        ),
        KeyValueRow::new(
            "shared_context",
            detail.summary.shared_context.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new(
            "dynamic_command",
            repl_run_command_hint(&detail.summary, &run_target),
        ),
        KeyValueRow::new("latest_run", latest_run_summary(latest_run)),
    ];
    let mut sections = vec![
        PanelSection::new(
            "Visual script map",
            render_focus_script_map(detail, latest_run),
        ),
        PanelSection::new(
            "Step selector",
            render_focus_step_selector(detail, &run_target, latest_run),
        ),
    ];
    if !recent_runs.is_empty() {
        sections.push(PanelSection::new(
            "Recent runs",
            render_run_selector_with_start(&recent_runs, detail.steps.len() + 1),
        ));
    }
    sections.push(PanelSection::new(
        "Latest run snapshot",
        render_latest_run_snapshot(latest_run),
    ));
    sections.push(PanelSection::new(
        "CLI palette",
        render_focus_cli_palette_for_target(&detail.summary, &run_target),
    ));
    sections.push(PanelSection::new(
        "REPL palette",
        render_focus_repl_palette_for_target(&detail.summary, &run_target),
    ));

    Ok(render_panel(
        &format!("Workflow overview: {}", detail.summary.name),
        &metadata,
        &sections,
    ))
}

fn render_focus_script_map(
    detail: &WorkflowScriptDetail,
    latest_run: Option<&WorkflowRunRecord>,
) -> Vec<String> {
    if detail.steps.is_empty() {
        return vec!["(no steps yet)".to_string()];
    }

    detail
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let name = step.name.as_deref().unwrap_or("(unnamed)");
            let mode = if step.run_in_background {
                "background"
            } else {
                "foreground"
            };
            format!(
                "  [{}] {} — prompt_chars={}, when={}, model={}, backend={}, cwd={}, mode={}, latest_status={}",
                index + 1,
                name,
                step.prompt_chars,
                yes_no(step.when),
                step.model.as_deref().unwrap_or("(default)"),
                step.backend.as_deref().unwrap_or("(default)"),
                step.cwd.as_deref().unwrap_or("(workspace)"),
                mode,
                latest_step_status(step.name.as_deref(), latest_run)
            )
        })
        .collect()
}

fn render_focus_step_selector(
    detail: &WorkflowScriptDetail,
    run_target: &WorkflowRunTarget,
    latest_run: Option<&WorkflowRunRecord>,
) -> Vec<String> {
    let entries = detail
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let mode = if step.run_in_background {
                "background"
            } else {
                "foreground"
            };
            let lines = vec![
                format!("prompt_chars: {}", step.prompt_chars),
                format!("when: {}", yes_no(step.when)),
                format!("model: {}", step.model.as_deref().unwrap_or("(default)")),
                format!(
                    "backend: {}",
                    step.backend.as_deref().unwrap_or("(default)")
                ),
                format!("cwd: {}", step.cwd.as_deref().unwrap_or("(workspace)")),
                format!("mode: {mode}"),
                format!(
                    "latest_status: {}",
                    latest_step_status(step.name.as_deref(), latest_run)
                ),
                format!(
                    "focus: `{}`",
                    repl_panel_focus_command(run_target, index + 1)
                ),
            ];
            SelectorEntry::new(step.name.as_deref().unwrap_or("(unnamed)"), lines)
                .with_badge(latest_step_status(step.name.as_deref(), latest_run))
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

fn render_latest_run_snapshot(record: Option<&WorkflowRunRecord>) -> Vec<String> {
    let Some(record) = record else {
        return vec!["(none recorded yet)".to_string()];
    };

    let mut lines = vec![
        format!("run_id: {}", record.run_id),
        format!("status: {}", status_badge(&record.status)),
        format!("finished_at: {}", record.finished_at),
        format!("summary: {}", compact_summary(&record.summary)),
        format!(
            "shared_context: {}",
            record.shared_context.as_deref().unwrap_or("(none)")
        ),
    ];
    if let Some(error) = &record.error {
        lines.push(format!("error: {}", preview_text(error)));
    }
    if !record.steps.is_empty() {
        lines.push(String::new());
        lines.push("step_results:".to_string());
        for (index, step) in record.steps.iter().enumerate() {
            lines.push(render_recorded_step_row(index + 1, step));
        }
    }
    lines
}

fn render_recorded_step_row(step_number: usize, step: &WorkflowRunStepRecord) -> String {
    let mut attributes = Vec::new();
    if let Some(result_text) = &step.result_text {
        attributes.push(format!("result_chars={}", result_text.chars().count()));
    }
    if step.error.is_some() {
        attributes.push("error=yes".to_string());
    }
    if step.reason.is_some() {
        attributes.push("reason=yes".to_string());
    }
    let suffix = if attributes.is_empty() {
        String::new()
    } else {
        format!(" | {}", attributes.join(", "))
    };
    format!(
        "  [{}] {:<10} {}{}",
        step_number,
        status_badge(&step.status),
        step.name,
        suffix
    )
}

fn render_focus_cli_palette(workflow: &WorkflowScriptSummary) -> Vec<String> {
    vec![
        format!(
            "- run: `hellox workflow run {} --shared-context \"<text>\"`",
            workflow.name
        ),
        format!(
            "- authoring panel: `hellox workflow panel {}`",
            workflow.name
        ),
        format!("- history: `hellox workflow runs {}`", workflow.name),
        format!("- latest run: `hellox workflow last-run {}`", workflow.name),
        format!("- validate: `hellox workflow validate {}`", workflow.name),
    ]
}

fn render_focus_repl_palette(workflow: &WorkflowScriptSummary) -> Vec<String> {
    let mut lines = vec![format!("- run: `{}`", dynamic_command_hint(workflow))];
    lines.push(format!(
        "- authoring panel: `/workflow panel {}`",
        workflow.name
    ));
    lines.push(format!("- history: `/workflow runs {}`", workflow.name));
    lines.push(format!(
        "- latest run: `/workflow last-run {}`",
        workflow.name
    ));
    lines.push(format!(
        "- validate: `/workflow validate {}`",
        workflow.name
    ));
    lines
}

fn render_focus_cli_palette_for_target(
    workflow: &WorkflowScriptSummary,
    run_target: &WorkflowRunTarget,
) -> Vec<String> {
    match run_target {
        WorkflowRunTarget::Named(_) => render_focus_cli_palette(workflow),
        WorkflowRunTarget::Path(path) => {
            let script_path = path_text(path);
            vec![
                format!(
                    "- run: `hellox workflow run --script-path {script_path} --shared-context \"<text>\"`"
                ),
                format!(
                    "- authoring panel: `hellox workflow panel --script-path {script_path}`"
                ),
                format!("- history: `hellox workflow runs --script-path {script_path}`"),
                format!("- latest run: `hellox workflow last-run --script-path {script_path}`"),
                format!("- validate: `hellox workflow validate --script-path {script_path}`"),
            ]
        }
    }
}

fn render_focus_repl_palette_for_target(
    workflow: &WorkflowScriptSummary,
    run_target: &WorkflowRunTarget,
) -> Vec<String> {
    match run_target {
        WorkflowRunTarget::Named(_) => render_focus_repl_palette(workflow),
        WorkflowRunTarget::Path(path) => {
            let script_path = path_text(path);
            vec![
                format!("- run: `/workflow run --script-path {script_path} [shared_context]`"),
                format!("- authoring panel: `/workflow panel --script-path {script_path}`"),
                format!("- history: `/workflow runs --script-path {script_path}`"),
                format!("- latest run: `/workflow last-run --script-path {script_path}`"),
                format!("- validate: `/workflow validate --script-path {script_path}`"),
            ]
        }
    }
}

fn repl_run_command_hint(
    workflow: &WorkflowScriptSummary,
    run_target: &WorkflowRunTarget,
) -> String {
    match run_target {
        WorkflowRunTarget::Named(_) => dynamic_command_hint(workflow),
        WorkflowRunTarget::Path(path) => {
            format!(
                "/workflow run --script-path {} [shared_context]",
                path_text(path)
            )
        }
    }
}

fn repl_panel_focus_command(run_target: &WorkflowRunTarget, step_number: usize) -> String {
    match run_target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow panel {workflow_name} {step_number}")
        }
        WorkflowRunTarget::Path(path) => format!(
            "/workflow panel --script-path {} {step_number}",
            path_text(path)
        ),
    }
}
