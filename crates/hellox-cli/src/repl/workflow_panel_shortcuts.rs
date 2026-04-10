use anyhow::Result;
use hellox_agent::AgentSession;

use super::*;
use crate::workflow_authoring::{
    duplicate_workflow_step, move_workflow_step, remove_workflow_step,
    resolve_existing_workflow_path,
};
use crate::workflow_panel::render_workflow_panel_detail;
use crate::workflows::WorkflowScriptDetail;

enum WorkflowPanelShortcut {
    Duplicate { to_step_number: Option<usize> },
    Move { to_step_number: usize },
    Remove,
}

impl CliReplDriver {
    pub(super) async fn handle_workflow_panel_shortcut(
        &self,
        input: &str,
        session: &mut AgentSession,
    ) -> Result<bool> {
        let Some(SelectorContext::WorkflowPanelSteps {
            workflow_name,
            step_count,
        }) = self.selector_context()
        else {
            return Ok(false);
        };

        let Some(shortcut) = parse_workflow_panel_shortcut(input) else {
            return Ok(false);
        };
        let shortcut = match shortcut {
            Ok(shortcut) => shortcut,
            Err(usage) => {
                println!("{usage}");
                return Ok(true);
            }
        };

        let selected_step = self
            .workflow_panel_focus()
            .filter(|focus| focus.workflow_name == workflow_name)
            .map(|focus| focus.selected_step)
            .filter(|selected_step| *selected_step <= step_count)
            .unwrap_or(1);
        let root = session.working_directory();
        let path = resolve_existing_workflow_path(root, &workflow_name)?;

        match shortcut {
            WorkflowPanelShortcut::Duplicate { to_step_number } => {
                let result =
                    duplicate_workflow_step(root, &path, selected_step, to_step_number, None)?;
                let duplicated_name = result
                    .duplicated_step_name
                    .as_deref()
                    .unwrap_or("(unnamed)");
                println!(
                    "Duplicated workflow step {selected_step} into step {} (`{duplicated_name}`).\n\n{}",
                    result.step_number,
                    self.render_workflow_panel_after_change(
                        root,
                        &result.detail,
                        Some(result.step_number),
                    )?
                );
            }
            WorkflowPanelShortcut::Move { to_step_number } => {
                let result = move_workflow_step(root, &path, selected_step, to_step_number)?;
                let moved_name = result.moved_step_name.as_deref().unwrap_or("(unnamed)");
                println!(
                    "Moved workflow step {selected_step} (`{moved_name}`) to step {}.\n\n{}",
                    result.step_number,
                    self.render_workflow_panel_after_change(
                        root,
                        &result.detail,
                        Some(result.step_number),
                    )?
                );
            }
            WorkflowPanelShortcut::Remove => {
                let result = remove_workflow_step(root, &path, selected_step)?;
                let removed_name = result.removed_step_name.as_deref().unwrap_or("(unnamed)");
                let next_step = (!result.detail.steps.is_empty())
                    .then_some(selected_step.min(result.detail.steps.len()));
                println!(
                    "Removed workflow step {selected_step} (`{removed_name}`).\n\n{}",
                    self.render_workflow_panel_after_change(root, &result.detail, next_step)?
                );
            }
        }

        Ok(true)
    }

    fn render_workflow_panel_after_change(
        &self,
        root: &std::path::Path,
        detail: &WorkflowScriptDetail,
        selected_step: Option<usize>,
    ) -> Result<String> {
        if let Some(selected_step) = selected_step {
            self.set_selector_context(SelectorContext::WorkflowPanelSteps {
                workflow_name: detail.summary.name.clone(),
                step_count: detail.steps.len(),
            });
            self.set_workflow_panel_focus(detail.summary.name.clone(), selected_step);
        } else {
            self.clear_selector_context();
        }
        render_workflow_panel_detail(root, detail, selected_step)
    }
}

fn parse_workflow_panel_shortcut(
    input: &str,
) -> Option<Result<WorkflowPanelShortcut, &'static str>> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?.to_ascii_lowercase();
    match command.as_str() {
        "dup" | "duplicate" => Some(match parts.next() {
            None => Ok(WorkflowPanelShortcut::Duplicate {
                to_step_number: None,
            }),
            Some(step) => match parse_shortcut_step_number(step) {
                Some(to_step_number) if parts.next().is_none() => {
                    Ok(WorkflowPanelShortcut::Duplicate {
                        to_step_number: Some(to_step_number),
                    })
                }
                _ => Err("Usage: dup [to-step-number]"),
            },
        }),
        "move" => Some(match (parts.next(), parts.next()) {
            (Some(step), None) => parse_shortcut_step_number(step)
                .map(|to_step_number| WorkflowPanelShortcut::Move { to_step_number })
                .ok_or("Usage: move <to-step-number>"),
            _ => Err("Usage: move <to-step-number>"),
        }),
        "rm" | "remove" | "delete" => Some(if parts.next().is_none() {
            Ok(WorkflowPanelShortcut::Remove)
        } else {
            Err("Usage: rm")
        }),
        _ => None,
    }
}

fn parse_shortcut_step_number(value: &str) -> Option<usize> {
    value
        .parse::<usize>()
        .ok()
        .filter(|step_number| *step_number > 0)
}
