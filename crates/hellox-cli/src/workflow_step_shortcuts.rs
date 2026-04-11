use std::path::Path;

use anyhow::Result;

use crate::workflow_authoring::{
    duplicate_workflow_step, move_workflow_step, remove_workflow_step,
    resolve_existing_workflow_path, update_workflow_step, WorkflowStepPatch,
};
use crate::workflows::WorkflowScriptDetail;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowStepShortcut {
    Duplicate { to_step_number: Option<usize> },
    Move { to_step_number: usize },
    Remove,
    SetName { value: String },
    ClearName,
    SetPrompt { value: String },
    SetWhen { value: String },
    ClearWhen,
    SetModel { value: String },
    ClearModel,
    SetBackend { value: String },
    ClearBackend,
    SetStepCwd { value: String },
    ClearStepCwd,
    SetRunMode { run_in_background: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowStepShortcutResult {
    pub(crate) detail: WorkflowScriptDetail,
    pub(crate) selected_step: Option<usize>,
    pub(crate) message: String,
}

pub(crate) fn parse_workflow_step_shortcut(
    input: &str,
) -> Option<Result<WorkflowStepShortcut, String>> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?.to_ascii_lowercase();
    let tail = parts.collect::<Vec<_>>();

    Some(match command.as_str() {
        "dup" | "duplicate" => parse_optional_step_number(&tail, "Usage: dup [to-step-number]")
            .map(|to_step_number| WorkflowStepShortcut::Duplicate { to_step_number }),
        "move" => parse_required_step_number(&tail, "Usage: move <to-step-number>")
            .map(|to_step_number| WorkflowStepShortcut::Move { to_step_number }),
        "rm" | "remove" | "delete" => {
            if tail.is_empty() {
                Ok(WorkflowStepShortcut::Remove)
            } else {
                Err("Usage: rm".to_string())
            }
        }
        "name" | "rename" => parse_required_text(&tail, "Usage: name <text>")
            .map(|value| WorkflowStepShortcut::SetName { value }),
        "clear-name" => {
            parse_no_tail(&tail, "Usage: clear-name").map(|_| WorkflowStepShortcut::ClearName)
        }
        "prompt" => parse_required_text(&tail, "Usage: prompt <text>")
            .map(|value| WorkflowStepShortcut::SetPrompt { value }),
        "when" => parse_required_text(&tail, "Usage: when <json>")
            .map(|value| WorkflowStepShortcut::SetWhen { value }),
        "clear-when" => {
            parse_no_tail(&tail, "Usage: clear-when").map(|_| WorkflowStepShortcut::ClearWhen)
        }
        "model" => parse_required_text(&tail, "Usage: model <name>")
            .map(|value| WorkflowStepShortcut::SetModel { value }),
        "clear-model" => {
            parse_no_tail(&tail, "Usage: clear-model").map(|_| WorkflowStepShortcut::ClearModel)
        }
        "backend" => parse_required_text(&tail, "Usage: backend <name>")
            .map(|value| WorkflowStepShortcut::SetBackend { value }),
        "clear-backend" => {
            parse_no_tail(&tail, "Usage: clear-backend").map(|_| WorkflowStepShortcut::ClearBackend)
        }
        "step-cwd" | "cwd" => parse_required_text(&tail, "Usage: step-cwd <path>")
            .map(|value| WorkflowStepShortcut::SetStepCwd { value }),
        "clear-step-cwd" | "clear-cwd" => parse_no_tail(&tail, "Usage: clear-step-cwd")
            .map(|_| WorkflowStepShortcut::ClearStepCwd),
        "background" | "bg" => {
            parse_no_tail(&tail, "Usage: background").map(|_| WorkflowStepShortcut::SetRunMode {
                run_in_background: true,
            })
        }
        "foreground" | "fg" => {
            parse_no_tail(&tail, "Usage: foreground").map(|_| WorkflowStepShortcut::SetRunMode {
                run_in_background: false,
            })
        }
        _ => return None,
    })
}

pub(crate) fn execute_workflow_step_shortcut(
    root: &Path,
    workflow_name: &str,
    selected_step: usize,
    shortcut: WorkflowStepShortcut,
) -> Result<WorkflowStepShortcutResult> {
    let path = resolve_existing_workflow_path(root, workflow_name)?;
    execute_workflow_step_shortcut_for_path(root, &path, selected_step, shortcut)
}

pub(crate) fn execute_workflow_step_shortcut_for_path(
    root: &Path,
    path: &Path,
    selected_step: usize,
    shortcut: WorkflowStepShortcut,
) -> Result<WorkflowStepShortcutResult> {
    Ok(match shortcut {
        WorkflowStepShortcut::Duplicate { to_step_number } => {
            let result = duplicate_workflow_step(root, &path, selected_step, to_step_number, None)?;
            let duplicated_name = result
                .duplicated_step_name
                .as_deref()
                .unwrap_or("(unnamed)");
            WorkflowStepShortcutResult {
                detail: result.detail,
                selected_step: Some(result.step_number),
                message: format!(
                    "Duplicated workflow step {selected_step} into step {} (`{duplicated_name}`).",
                    result.step_number
                ),
            }
        }
        WorkflowStepShortcut::Move { to_step_number } => {
            let result = move_workflow_step(root, &path, selected_step, to_step_number)?;
            let moved_name = result.moved_step_name.as_deref().unwrap_or("(unnamed)");
            WorkflowStepShortcutResult {
                detail: result.detail,
                selected_step: Some(result.step_number),
                message: format!(
                    "Moved workflow step {selected_step} (`{moved_name}`) to step {}.",
                    result.step_number
                ),
            }
        }
        WorkflowStepShortcut::Remove => {
            let result = remove_workflow_step(root, &path, selected_step)?;
            let removed_name = result.removed_step_name.as_deref().unwrap_or("(unnamed)");
            let next_step = (!result.detail.steps.is_empty())
                .then_some(selected_step.min(result.detail.steps.len()));
            WorkflowStepShortcutResult {
                detail: result.detail,
                selected_step: next_step,
                message: format!("Removed workflow step {selected_step} (`{removed_name}`)."),
            }
        }
        WorkflowStepShortcut::SetName { value } => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    name: Some(Some(value)),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Updated workflow step {selected_step} name."),
        },
        WorkflowStepShortcut::ClearName => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    name: Some(None),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Cleared workflow step {selected_step} name."),
        },
        WorkflowStepShortcut::SetPrompt { value } => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    prompt: Some(value),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Updated workflow step {selected_step} prompt."),
        },
        WorkflowStepShortcut::SetWhen { value } => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    when: Some(Some(value)),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Updated workflow step {selected_step} when condition."),
        },
        WorkflowStepShortcut::ClearWhen => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    when: Some(None),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Cleared workflow step {selected_step} when condition."),
        },
        WorkflowStepShortcut::SetModel { value } => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    model: Some(Some(value)),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Updated workflow step {selected_step} model."),
        },
        WorkflowStepShortcut::ClearModel => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    model: Some(None),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Cleared workflow step {selected_step} model."),
        },
        WorkflowStepShortcut::SetBackend { value } => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    backend: Some(Some(value)),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Updated workflow step {selected_step} backend."),
        },
        WorkflowStepShortcut::ClearBackend => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    backend: Some(None),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Cleared workflow step {selected_step} backend."),
        },
        WorkflowStepShortcut::SetStepCwd { value } => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    step_cwd: Some(Some(value)),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Updated workflow step {selected_step} cwd."),
        },
        WorkflowStepShortcut::ClearStepCwd => WorkflowStepShortcutResult {
            detail: update_workflow_step(
                root,
                &path,
                selected_step,
                WorkflowStepPatch {
                    step_cwd: Some(None),
                    ..WorkflowStepPatch::default()
                },
            )?,
            selected_step: Some(selected_step),
            message: format!("Cleared workflow step {selected_step} cwd."),
        },
        WorkflowStepShortcut::SetRunMode { run_in_background } => {
            let mode = if run_in_background {
                "background"
            } else {
                "foreground"
            };
            WorkflowStepShortcutResult {
                detail: update_workflow_step(
                    root,
                    &path,
                    selected_step,
                    WorkflowStepPatch {
                        run_in_background: Some(run_in_background),
                        ..WorkflowStepPatch::default()
                    },
                )?,
                selected_step: Some(selected_step),
                message: format!("Set workflow step {selected_step} to {mode} mode."),
            }
        }
    })
}

fn parse_required_text(parts: &[&str], usage: &str) -> Result<String, String> {
    let value = parts.join(" ");
    let value = value.trim();
    if value.is_empty() {
        Err(usage.to_string())
    } else {
        Ok(value.to_string())
    }
}

fn parse_no_tail(parts: &[&str], usage: &str) -> Result<(), String> {
    if parts.is_empty() {
        Ok(())
    } else {
        Err(usage.to_string())
    }
}

fn parse_required_step_number(parts: &[&str], usage: &str) -> Result<usize, String> {
    match parts {
        [value] => value
            .parse::<usize>()
            .ok()
            .filter(|step_number| *step_number > 0)
            .ok_or_else(|| usage.to_string()),
        _ => Err(usage.to_string()),
    }
}

fn parse_optional_step_number(parts: &[&str], usage: &str) -> Result<Option<usize>, String> {
    match parts {
        [] => Ok(None),
        [value] => value
            .parse::<usize>()
            .ok()
            .filter(|step_number| *step_number > 0)
            .map(Some)
            .ok_or_else(|| usage.to_string()),
        _ => Err(usage.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_workflow_step_shortcut, WorkflowStepShortcut};

    #[test]
    fn parses_duplicate_move_remove_and_field_shortcuts() {
        assert_eq!(
            parse_workflow_step_shortcut("dup 3"),
            Some(Ok(WorkflowStepShortcut::Duplicate {
                to_step_number: Some(3),
            }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("move 1"),
            Some(Ok(WorkflowStepShortcut::Move { to_step_number: 1 }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("rm"),
            Some(Ok(WorkflowStepShortcut::Remove))
        );
        assert_eq!(
            parse_workflow_step_shortcut("name ship review"),
            Some(Ok(WorkflowStepShortcut::SetName {
                value: "ship review".to_string(),
            }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("prompt summarize findings"),
            Some(Ok(WorkflowStepShortcut::SetPrompt {
                value: "summarize findings".to_string(),
            }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("when {\"previous_status\":\"completed\"}"),
            Some(Ok(WorkflowStepShortcut::SetWhen {
                value: "{\"previous_status\":\"completed\"}".to_string(),
            }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("clear-model"),
            Some(Ok(WorkflowStepShortcut::ClearModel))
        );
        assert_eq!(
            parse_workflow_step_shortcut("backend detached_process"),
            Some(Ok(WorkflowStepShortcut::SetBackend {
                value: "detached_process".to_string(),
            }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("cwd docs"),
            Some(Ok(WorkflowStepShortcut::SetStepCwd {
                value: "docs".to_string(),
            }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("background"),
            Some(Ok(WorkflowStepShortcut::SetRunMode {
                run_in_background: true,
            }))
        );
        assert_eq!(
            parse_workflow_step_shortcut("foreground"),
            Some(Ok(WorkflowStepShortcut::SetRunMode {
                run_in_background: false,
            }))
        );
    }

    #[test]
    fn invalid_shortcuts_return_usage() {
        assert_eq!(
            parse_workflow_step_shortcut("move"),
            Some(Err("Usage: move <to-step-number>".to_string()))
        );
        assert_eq!(
            parse_workflow_step_shortcut("name"),
            Some(Err("Usage: name <text>".to_string()))
        );
        assert_eq!(
            parse_workflow_step_shortcut("clear-name nope"),
            Some(Err("Usage: clear-name".to_string()))
        );
        assert_eq!(parse_workflow_step_shortcut("/workflow panel"), None);
        assert_eq!(parse_workflow_step_shortcut("mystery"), None);
    }
}
