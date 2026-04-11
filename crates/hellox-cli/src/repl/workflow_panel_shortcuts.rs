use anyhow::Result;
use hellox_agent::AgentSession;

use super::*;
use crate::workflow_panel::list_workflow_panel_selection_items;
use crate::workflow_panel::render_workflow_panel_detail;
use crate::workflow_runs::{
    load_workflow_run, render_workflow_run_inspect_panel_with_step,
    select_workflow_run_step_number, WorkflowRunRecord,
};
use crate::workflow_step_navigation::{
    execute_workflow_step_navigation, parse_workflow_step_navigation, WorkflowStepNavigationResult,
    WorkflowStepNavigationShortcut,
};
use crate::workflow_step_shortcuts::{
    execute_workflow_step_shortcut, parse_workflow_step_shortcut,
};
use crate::workflows::{load_named_workflow_detail, WorkflowScriptDetail};

impl CliReplDriver {
    pub(super) async fn handle_workflow_panel_shortcut(
        &self,
        input: &str,
        session: &mut AgentSession,
    ) -> Result<bool> {
        if self.handle_workflow_step_navigation(input, session)? {
            return Ok(true);
        }

        let Some(SelectorContext::WorkflowPanelItems {
            workflow_name,
            step_count,
            ..
        }) = self.selector_context()
        else {
            return Ok(false);
        };

        let Some(shortcut) = parse_workflow_step_shortcut(input) else {
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
        let result = execute_workflow_step_shortcut(root, &workflow_name, selected_step, shortcut)?;
        println!(
            "{}\n\n{}",
            result.message,
            self.render_workflow_panel_after_change(root, &result.detail, result.selected_step)?
        );

        Ok(true)
    }

    fn handle_workflow_step_navigation(
        &self,
        input: &str,
        session: &mut AgentSession,
    ) -> Result<bool> {
        let Some(shortcut) = parse_workflow_step_navigation(input) else {
            return Ok(false);
        };
        let shortcut = match shortcut {
            Ok(shortcut) => shortcut,
            Err(usage) => {
                println!("{usage}");
                return Ok(true);
            }
        };

        match self.selector_context() {
            Some(SelectorContext::WorkflowPanelItems {
                workflow_name,
                step_count,
                ..
            }) => {
                let detail =
                    load_named_workflow_detail(session.working_directory(), &workflow_name)?;
                if step_count == 0 || detail.steps.is_empty() {
                    println!(
                        "workflow `{}` has no steps to focus yet.",
                        detail.summary.name
                    );
                    return Ok(true);
                }

                let step_count = detail.steps.len();
                let selected_step = self
                    .workflow_panel_focus()
                    .filter(|focus| focus.workflow_name == detail.summary.name)
                    .map(|focus| focus.selected_step)
                    .filter(|selected_step| *selected_step <= step_count)
                    .unwrap_or(1);
                let result =
                    match execute_workflow_step_navigation(selected_step, step_count, shortcut) {
                        Ok(result) => result,
                        Err(message) => {
                            println!("{message}");
                            return Ok(true);
                        }
                    };
                println!(
                    "{}\n\n{}",
                    navigation_message(shortcut, "workflow step", step_count, result),
                    self.render_workflow_panel_after_change(
                        session.working_directory(),
                        &detail,
                        Some(result.step_number),
                    )?
                );
                Ok(true)
            }
            Some(SelectorContext::WorkflowRunSteps { run_id, .. }) => {
                let record = load_workflow_run(session.working_directory(), &run_id)?;
                let step_count = record.steps.len();
                if step_count == 0 {
                    println!("workflow run `{run_id}` has no recorded steps.");
                    return Ok(true);
                }

                let selected_step = self
                    .workflow_run_focus()
                    .filter(|focus| focus.run_id == run_id)
                    .map(|focus| focus.selected_step)
                    .filter(|selected_step| *selected_step <= step_count)
                    .or_else(|| select_workflow_run_step_number(&record, None))
                    .unwrap_or(1);
                let result =
                    match execute_workflow_step_navigation(selected_step, step_count, shortcut) {
                        Ok(result) => result,
                        Err(message) => {
                            println!("{message}");
                            return Ok(true);
                        }
                    };
                println!(
                    "{}\n\n{}",
                    navigation_message(shortcut, "recorded step", step_count, result),
                    self.render_workflow_run_after_navigation(
                        session.working_directory(),
                        &record,
                        result.step_number,
                    )
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn render_workflow_panel_after_change(
        &self,
        root: &std::path::Path,
        detail: &WorkflowScriptDetail,
        selected_step: Option<usize>,
    ) -> Result<String> {
        if let Ok(items) = list_workflow_panel_selection_items(root, &detail.summary.name) {
            if !items.is_empty() {
                self.set_selector_context(SelectorContext::WorkflowPanelItems {
                    workflow_name: detail.summary.name.clone(),
                    step_count: detail.steps.len(),
                    items,
                });
            } else {
                self.clear_selector_context();
            }
        } else if selected_step.is_none() {
            self.clear_selector_context();
        }
        if let Some(selected_step) = selected_step {
            self.set_workflow_panel_focus(detail.summary.name.clone(), selected_step);
        } else if detail.steps.is_empty() {
            self.clear_workflow_panel_focus();
        }
        render_workflow_panel_detail(root, detail, selected_step)
    }

    fn render_workflow_run_after_navigation(
        &self,
        root: &std::path::Path,
        record: &WorkflowRunRecord,
        selected_step: usize,
    ) -> String {
        self.set_selector_context(SelectorContext::WorkflowRunSteps {
            run_id: record.run_id.clone(),
            step_count: record.steps.len(),
        });
        self.set_workflow_run_focus(record.run_id.clone(), selected_step);
        render_workflow_run_inspect_panel_with_step(root, record, Some(selected_step))
    }
}

fn navigation_message(
    shortcut: WorkflowStepNavigationShortcut,
    label: &str,
    step_count: usize,
    result: WorkflowStepNavigationResult,
) -> String {
    if result.changed {
        format!("Focused {label} {} of {step_count}.", result.step_number)
    } else {
        format!(
            "Already on the {} {label} ({} of {step_count}).",
            shortcut.boundary_name(),
            result.step_number
        )
    }
}
