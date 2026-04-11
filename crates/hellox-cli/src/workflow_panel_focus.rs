use std::path::Path;

use anyhow::Result;
use hellox_tui::{
    render_panel, render_selector, render_table, status_badge, KeyValueRow, PanelSection,
    SelectorEntry, Table,
};

use crate::workflow_runs::{
    list_workflow_runs, load_latest_workflow_run, render_run_selector_with_start,
    WorkflowRunRecord, WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
};
use crate::workflows::{WorkflowRunTarget, WorkflowScriptDetail, WorkflowStepSummary};

use super::{
    dynamic_command_hint, latest_run_summary, latest_step_status, path_text, validate_step_number,
    yes_no,
};

pub(super) fn render_workflow_panel_detail(
    root: &Path,
    detail: &WorkflowScriptDetail,
    target: &WorkflowRunTarget,
    step_number: Option<usize>,
) -> Result<String> {
    if let Some(step_number) = step_number {
        validate_step_number(step_number, detail.steps.len())?;
    }

    let recent_runs = list_workflow_runs(root, Some(target), WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT)?;
    let latest_run = load_latest_workflow_run(root, Some(target)).ok();
    let metadata = vec![
        KeyValueRow::new("path", path_text(&detail.summary.path)),
        KeyValueRow::new("steps", detail.summary.step_count.to_string()),
        KeyValueRow::new(
            "continue_on_error",
            detail.summary.continue_on_error.to_string(),
        ),
        KeyValueRow::new(
            "shared_context",
            detail.summary.shared_context.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new("dynamic_command", dynamic_command_hint(&detail.summary)),
        KeyValueRow::new("latest_run", latest_run_summary(latest_run.as_ref())),
    ];
    let sections = vec![
        PanelSection::new(
            "Visual step map",
            render_table(&build_step_table(detail, latest_run.as_ref(), step_number)),
        ),
        PanelSection::new(
            "Step selector",
            render_step_selector(detail, target, latest_run.as_ref(), step_number),
        ),
        PanelSection::new("Recent runs", render_recent_runs(detail, &recent_runs)),
        PanelSection::new(
            "Focused step lens",
            render_focused_step_lens(detail, target, latest_run.as_ref(), step_number),
        ),
        PanelSection::new(
            "Action palette",
            render_action_palette(target, detail.steps.len(), step_number),
        ),
        PanelSection::new(
            "REPL palette",
            render_repl_palette(target, detail.steps.len(), step_number),
        ),
    ];

    Ok(render_panel(
        &format!("Workflow authoring panel: {}", detail.summary.name),
        &metadata,
        &sections,
    ))
}

fn build_step_table(
    detail: &WorkflowScriptDetail,
    latest_run: Option<&WorkflowRunRecord>,
    step_number: Option<usize>,
) -> Table {
    let rows = detail
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let selected = if step_number == Some(index + 1) {
                ">"
            } else {
                ""
            };
            vec![
                selected.to_string(),
                (index + 1).to_string(),
                step.name.as_deref().unwrap_or("(unnamed)").to_string(),
                step.prompt_chars.to_string(),
                yes_no(step.when).to_string(),
                step.model.as_deref().unwrap_or("(default)").to_string(),
                step.backend.as_deref().unwrap_or("(default)").to_string(),
                step.cwd.as_deref().unwrap_or("(workspace)").to_string(),
                if step.run_in_background {
                    "background".to_string()
                } else {
                    "foreground".to_string()
                },
                latest_step_status(step, latest_run),
            ]
        })
        .collect();
    Table::new(
        vec![
            "".to_string(),
            "#".to_string(),
            "step".to_string(),
            "prompt".to_string(),
            "when".to_string(),
            "model".to_string(),
            "backend".to_string(),
            "cwd".to_string(),
            "mode".to_string(),
            "latest".to_string(),
        ],
        rows,
    )
}

fn render_step_selector(
    detail: &WorkflowScriptDetail,
    target: &WorkflowRunTarget,
    latest_run: Option<&WorkflowRunRecord>,
    step_number: Option<usize>,
) -> Vec<String> {
    if detail.steps.is_empty() {
        return vec!["(no steps yet)".to_string()];
    }

    let selected_index = selected_step_index(detail, step_number);
    let entries = detail
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let lines = vec![
                format!("prompt_chars: {}", step.prompt_chars),
                format!("when: {}", yes_no(step.when)),
                format!("model: {}", step.model.as_deref().unwrap_or("(default)")),
                format!(
                    "backend: {}",
                    step.backend.as_deref().unwrap_or("(default)")
                ),
                format!("cwd: {}", step.cwd.as_deref().unwrap_or("(workspace)")),
                format!(
                    "mode: {}",
                    if step.run_in_background {
                        "background"
                    } else {
                        "foreground"
                    }
                ),
                format!("latest_status: {}", latest_step_status(step, latest_run)),
                format!("focus: `{}`", repl_panel_focus_command(target, index + 1)),
            ];

            SelectorEntry::new(step.name.as_deref().unwrap_or("(unnamed)"), lines)
                .with_badge(selector_status_badge(step, latest_run))
                .selected(selected_index == Some(index))
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

fn render_focused_step_lens(
    detail: &WorkflowScriptDetail,
    target: &WorkflowRunTarget,
    latest_run: Option<&WorkflowRunRecord>,
    step_number: Option<usize>,
) -> Vec<String> {
    let Some(selected_index) = selected_step_index(detail, step_number) else {
        return vec!["(no steps yet)".to_string()];
    };
    let Some(step) = detail.steps.get(selected_index) else {
        return vec!["(selected step unavailable)".to_string()];
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
        format!(
            "mode: {}",
            if step.run_in_background {
                "background"
            } else {
                "foreground"
            }
        ),
        format!("latest_status: {}", latest_step_status(step, latest_run)),
        format!(
            "edit: `{}`",
            cli_update_step_command(target, selected_index + 1)
        ),
        format!(
            "duplicate: `{}`",
            cli_duplicate_step_command(target, selected_index + 1, selected_index + 2)
        ),
        format!(
            "move: `{}`",
            cli_move_step_command(
                target,
                selected_index + 1,
                suggested_move_target(selected_index + 1, detail.steps.len())
            )
        ),
        format!(
            "remove: `{}`",
            cli_remove_step_command(target, selected_index + 1)
        ),
    ];

    render_selector(&[SelectorEntry::new(
        step.name.as_deref().unwrap_or("(unnamed)").to_string(),
        lines,
    )
    .with_badge(selector_status_badge(step, latest_run))
    .selected(true)])
}

fn render_recent_runs(
    detail: &WorkflowScriptDetail,
    recent_runs: &[WorkflowRunRecord],
) -> Vec<String> {
    if recent_runs.is_empty() {
        return vec!["(none recorded yet)".to_string()];
    }

    render_run_selector_with_start(recent_runs, detail.steps.len() + 1)
}

fn render_action_palette(
    target: &WorkflowRunTarget,
    step_count: usize,
    step_number: Option<usize>,
) -> Vec<String> {
    let mut lines = vec![
        format!("- add step: `{}`", cli_add_step_command(target)),
        format!(
            "- set shared context: `{}`",
            cli_set_shared_context_command(target)
        ),
        format!("- run: `{}`", cli_run_command(target)),
        format!("- inspect history: `{}`", cli_runs_command(target)),
    ];

    if let Some(step_number) = step_number.or_else(|| (step_count > 0).then_some(1)) {
        lines.push(format!(
            "- edit step {step_number}: `{}`",
            cli_update_step_command(target, step_number)
        ));
        lines.push(format!(
            "- duplicate step {step_number}: `{}`",
            cli_duplicate_step_command(target, step_number, step_number + 1)
        ));
        lines.push(format!(
            "- move step {step_number}: `{}`",
            cli_move_step_command(
                target,
                step_number,
                suggested_move_target(step_number, step_count)
            )
        ));
        lines.push(format!(
            "- remove step {step_number}: `{}`",
            cli_remove_step_command(target, step_number)
        ));
        lines.push(format!(
            "- focus step {step_number}: `{}`",
            cli_panel_focus_command(target, step_number)
        ));
    }

    lines
}

fn render_repl_palette(
    target: &WorkflowRunTarget,
    step_count: usize,
    step_number: Option<usize>,
) -> Vec<String> {
    let mut lines = vec![
        format!("- add step: `{}`", repl_add_step_command(target)),
        format!(
            "- set shared context: `{}`",
            repl_set_shared_context_command(target)
        ),
        format!("- run: `{}`", repl_run_command(target)),
        format!("- inspect history: `{}`", repl_runs_command(target)),
    ];

    if let Some(step_number) = step_number.or_else(|| (step_count > 0).then_some(1)) {
        lines.push(format!(
            "- edit step {step_number}: `{}`",
            repl_update_step_command(target, step_number)
        ));
        lines.push(
            "- quick field edit in focused panel/dashboard: `name <text>` / `prompt <text>` / `when <json>` / `model <name>` / `backend <name>` / `step-cwd <path>`".to_string(),
        );
        lines.push(
            "- clear focused fields: `clear-name` / `clear-when` / `clear-model` / `clear-backend` / `clear-step-cwd`".to_string(),
        );
        lines.push(
            "- switch focused step mode: `background` / `foreground`; reorder with `dup [to]` / `move <to>` / `rm`".to_string(),
        );
        lines.push(
            "- move focused lens in REPL/dashboard: `first` / `prev` / `next` / `last`".to_string(),
        );
        lines.push(format!(
            "- duplicate step {step_number}: `{}`",
            repl_duplicate_step_command(target, step_number, step_number + 1)
        ));
        lines.push(format!(
            "- move step {step_number}: `{}`",
            repl_move_step_command(
                target,
                step_number,
                suggested_move_target(step_number, step_count)
            )
        ));
        lines.push(format!(
            "- remove step {step_number}: `{}`",
            repl_remove_step_command(target, step_number)
        ));
        lines.push(format!(
            "- focus step {step_number}: `{}`",
            repl_panel_focus_command(target, step_number)
        ));
    }

    lines
}

fn selected_step_index(detail: &WorkflowScriptDetail, step_number: Option<usize>) -> Option<usize> {
    step_number
        .or_else(|| (!detail.steps.is_empty()).then_some(1))
        .and_then(|value| value.checked_sub(1))
}

fn selector_status_badge(
    step: &WorkflowStepSummary,
    latest_run: Option<&WorkflowRunRecord>,
) -> String {
    let status = latest_step_status(step, latest_run);
    if status.starts_with('(') {
        status
    } else {
        status_badge(&status)
    }
}

fn suggested_move_target(step_number: usize, step_count: usize) -> usize {
    if step_count <= 1 {
        1
    } else if step_number == 1 {
        2
    } else {
        1
    }
}

fn cli_target_arg(target: &WorkflowRunTarget) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => format!("--workflow {workflow_name}"),
        WorkflowRunTarget::Path(path) => format!("--script-path {}", path_text(path)),
    }
}

fn cli_add_step_command(target: &WorkflowRunTarget) -> String {
    format!(
        "hellox workflow add-step {} --prompt \"<text>\" --name \"<step-name>\"",
        cli_target_arg(target)
    )
}

fn cli_set_shared_context_command(target: &WorkflowRunTarget) -> String {
    format!(
        "hellox workflow set-shared-context {} \"<text>\"",
        cli_target_arg(target)
    )
}

fn cli_run_command(target: &WorkflowRunTarget) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("hellox workflow run {workflow_name} --shared-context \"<text>\"")
        }
        WorkflowRunTarget::Path(path) => format!(
            "hellox workflow run --script-path {} --shared-context \"<text>\"",
            path_text(path)
        ),
    }
}

fn cli_runs_command(target: &WorkflowRunTarget) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => format!("hellox workflow runs {workflow_name}"),
        WorkflowRunTarget::Path(path) => {
            format!("hellox workflow runs --script-path {}", path_text(path))
        }
    }
}

fn cli_update_step_command(target: &WorkflowRunTarget, step_number: usize) -> String {
    format!(
        "hellox workflow update-step {} {step_number} --prompt \"<text>\"",
        cli_target_arg(target)
    )
}

fn cli_duplicate_step_command(
    target: &WorkflowRunTarget,
    step_number: usize,
    to_step_number: usize,
) -> String {
    format!(
        "hellox workflow duplicate-step {} {step_number} --to {to_step_number} --name \"<copy-name>\"",
        cli_target_arg(target)
    )
}

fn cli_move_step_command(
    target: &WorkflowRunTarget,
    step_number: usize,
    to_step_number: usize,
) -> String {
    format!(
        "hellox workflow move-step {} {step_number} --to {to_step_number}",
        cli_target_arg(target)
    )
}

fn cli_remove_step_command(target: &WorkflowRunTarget, step_number: usize) -> String {
    format!(
        "hellox workflow remove-step {} {step_number}",
        cli_target_arg(target)
    )
}

fn cli_panel_focus_command(target: &WorkflowRunTarget, step_number: usize) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("hellox workflow panel {workflow_name} --step {step_number}")
        }
        WorkflowRunTarget::Path(path) => format!(
            "hellox workflow panel --script-path {} --step {step_number}",
            path_text(path)
        ),
    }
}

fn repl_add_step_command(target: &WorkflowRunTarget) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow add-step {workflow_name} --prompt <text> --name <step-name>")
        }
        WorkflowRunTarget::Path(path) => format!(
            "/workflow add-step --script-path {} --prompt <text> --name <step-name>",
            path_text(path)
        ),
    }
}

fn repl_set_shared_context_command(target: &WorkflowRunTarget) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow set-shared-context {workflow_name} <text>")
        }
        WorkflowRunTarget::Path(path) => {
            format!(
                "/workflow set-shared-context --script-path {} <text>",
                path_text(path)
            )
        }
    }
}

fn repl_run_command(target: &WorkflowRunTarget) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow run {workflow_name} <shared_context>")
        }
        WorkflowRunTarget::Path(path) => {
            format!(
                "/workflow run --script-path {} <shared_context>",
                path_text(path)
            )
        }
    }
}

fn repl_runs_command(target: &WorkflowRunTarget) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => format!("/workflow runs {workflow_name}"),
        WorkflowRunTarget::Path(path) => {
            format!("/workflow runs --script-path {}", path_text(path))
        }
    }
}

fn repl_update_step_command(target: &WorkflowRunTarget, step_number: usize) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow update-step {workflow_name} {step_number} --prompt <text>")
        }
        WorkflowRunTarget::Path(path) => format!(
            "/workflow update-step --script-path {} {step_number} --prompt <text>",
            path_text(path)
        ),
    }
}

fn repl_duplicate_step_command(
    target: &WorkflowRunTarget,
    step_number: usize,
    to_step_number: usize,
) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => format!(
            "/workflow duplicate-step {workflow_name} {step_number} --to {to_step_number} --name <copy-name>"
        ),
        WorkflowRunTarget::Path(path) => format!(
            "/workflow duplicate-step --script-path {} {step_number} --to {to_step_number} --name <copy-name>",
            path_text(path)
        ),
    }
}

fn repl_move_step_command(
    target: &WorkflowRunTarget,
    step_number: usize,
    to_step_number: usize,
) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow move-step {workflow_name} {step_number} --to {to_step_number}")
        }
        WorkflowRunTarget::Path(path) => format!(
            "/workflow move-step --script-path {} {step_number} --to {to_step_number}",
            path_text(path)
        ),
    }
}

fn repl_remove_step_command(target: &WorkflowRunTarget, step_number: usize) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow remove-step {workflow_name} {step_number}")
        }
        WorkflowRunTarget::Path(path) => {
            format!(
                "/workflow remove-step --script-path {} {step_number}",
                path_text(path)
            )
        }
    }
}

fn repl_panel_focus_command(target: &WorkflowRunTarget, step_number: usize) -> String {
    match target {
        WorkflowRunTarget::Named(workflow_name) => {
            format!("/workflow panel {workflow_name} {step_number}")
        }
        WorkflowRunTarget::Path(path) => {
            format!(
                "/workflow panel --script-path {} {step_number}",
                path_text(path)
            )
        }
    }
}
