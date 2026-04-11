use anyhow::Result;
use hellox_agent::AgentSession;

use super::{commands::WorkflowCommand, *};
use crate::workflow_overview::{
    list_workflow_focus_selection_items, list_workflow_overview_selection_items,
    WorkflowOverviewFocusSelectionItem, WorkflowOverviewSelectionItem,
};
use crate::workflow_panel::{list_workflow_panel_selection_items, WorkflowPanelSelectionItem};
use crate::workflow_runs::{
    list_workflow_runs, load_latest_workflow_run, load_workflow_run,
    render_workflow_run_inspect_panel_with_step, select_workflow_run_step_number,
    WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
};
use crate::workflows::{list_workflows, load_named_workflow_detail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkflowPanelFocus {
    pub(super) workflow_name: String,
    pub(super) selected_step: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkflowRunFocus {
    pub(super) run_id: String,
    pub(super) selected_step: usize,
}

impl CliReplDriver {
    pub(super) fn clear_workflow_panel_focus(&self) {
        if let Ok(mut guard) = self.workflow_panel_focus.lock() {
            *guard = None;
        }
    }

    pub(super) fn set_workflow_panel_focus(&self, workflow_name: String, selected_step: usize) {
        if let Ok(mut guard) = self.workflow_panel_focus.lock() {
            *guard = Some(WorkflowPanelFocus {
                workflow_name,
                selected_step,
            });
        }
    }

    pub(super) fn workflow_panel_focus(&self) -> Option<WorkflowPanelFocus> {
        self.workflow_panel_focus
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub(super) fn clear_workflow_run_focus(&self) {
        if let Ok(mut guard) = self.workflow_run_focus.lock() {
            *guard = None;
        }
    }

    pub(super) fn set_workflow_run_focus(&self, run_id: String, selected_step: usize) {
        if let Ok(mut guard) = self.workflow_run_focus.lock() {
            *guard = Some(WorkflowRunFocus {
                run_id,
                selected_step,
            });
        }
    }

    pub(super) fn workflow_run_focus(&self) -> Option<WorkflowRunFocus> {
        self.workflow_run_focus
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub(super) fn prepare_workflow_selector_context(
        &self,
        session: &AgentSession,
        command: &WorkflowCommand,
    ) {
        match command {
            WorkflowCommand::Overview {
                workflow_name: None,
            } => {
                if let Ok(items) =
                    list_workflow_overview_selection_items(session.working_directory())
                {
                    if !items.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowOverviewList { items });
                    }
                }
            }
            WorkflowCommand::Overview {
                workflow_name: Some(workflow_name),
            } => {
                if let Ok(items) =
                    list_workflow_focus_selection_items(session.working_directory(), workflow_name)
                {
                    if !items.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowOverviewFocusItems {
                            workflow_name: workflow_name.clone(),
                            items,
                        });
                    }
                }
            }
            WorkflowCommand::Panel {
                workflow_name: None,
                ..
            } => {
                if let Ok(workflows) = list_workflows(session.working_directory()) {
                    let workflow_names = workflows
                        .into_iter()
                        .map(|workflow| workflow.name)
                        .collect::<Vec<_>>();
                    if !workflow_names.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowPanelList {
                            workflow_names,
                        });
                    }
                }
            }
            WorkflowCommand::Panel {
                workflow_name: Some(workflow_name),
                step_number,
            } => {
                if let Ok(detail) =
                    load_named_workflow_detail(session.working_directory(), workflow_name)
                {
                    if let Ok(items) = list_workflow_panel_selection_items(
                        session.working_directory(),
                        &detail.summary.name,
                    ) {
                        if !items.is_empty() {
                            self.set_selector_context(SelectorContext::WorkflowPanelItems {
                                workflow_name: detail.summary.name.clone(),
                                step_count: detail.steps.len(),
                                items,
                            });
                        }
                    }
                    if let Some(selected_step) =
                        normalize_selected_step(*step_number, detail.steps.len())
                    {
                        self.set_workflow_panel_focus(detail.summary.name, selected_step);
                    }
                }
            }
            WorkflowCommand::Runs { workflow_name } => {
                if let Ok(runs) = list_workflow_runs(
                    session.working_directory(),
                    workflow_name.as_deref(),
                    WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
                ) {
                    let run_ids = runs
                        .into_iter()
                        .map(|record| record.run_id)
                        .collect::<Vec<_>>();
                    if !run_ids.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowRunList { run_ids });
                    }
                }
            }
            WorkflowCommand::ShowRun {
                run_id: Some(run_id),
                step_number,
            } => {
                if let Ok(record) = load_workflow_run(session.working_directory(), run_id) {
                    if !record.steps.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowRunSteps {
                            run_id: record.run_id.clone(),
                            step_count: record.steps.len(),
                        });
                        if let Some(selected_step) =
                            select_workflow_run_step_number(&record, *step_number)
                        {
                            self.set_workflow_run_focus(record.run_id, selected_step);
                        }
                    }
                }
            }
            WorkflowCommand::LastRun {
                workflow_name,
                step_number,
            } => {
                if let Ok(record) =
                    load_latest_workflow_run(session.working_directory(), workflow_name.as_deref())
                {
                    if !record.steps.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowRunSteps {
                            run_id: record.run_id.clone(),
                            step_count: record.steps.len(),
                        });
                        if let Some(selected_step) =
                            select_workflow_run_step_number(&record, *step_number)
                        {
                            self.set_workflow_run_focus(record.run_id, selected_step);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) async fn handle_workflow_selector_index(
        &self,
        index: usize,
        context: &SelectorContext,
        session: &mut AgentSession,
    ) -> Result<bool> {
        match context {
            SelectorContext::WorkflowOverviewList { items } => {
                if index == 0 || index > items.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow overview`.",
                        items.len()
                    );
                    return Ok(true);
                }

                let item = items[index - 1].clone();
                self.clear_selector_context();
                match item {
                    WorkflowOverviewSelectionItem::Workflow(workflow_name) => {
                        let command = WorkflowCommand::Overview {
                            workflow_name: Some(workflow_name),
                        };
                        self.prepare_workflow_selector_context(session, &command);
                        println!("{}", handle_workflow_command(command, session).await?);
                    }
                    WorkflowOverviewSelectionItem::Run(run_id) => {
                        let command = WorkflowCommand::ShowRun {
                            run_id: Some(run_id),
                            step_number: None,
                        };
                        self.prepare_workflow_selector_context(session, &command);
                        println!("{}", handle_workflow_command(command, session).await?);
                    }
                }
                Ok(true)
            }
            SelectorContext::WorkflowOverviewFocusItems {
                workflow_name,
                items,
            } => {
                if index == 0 || index > items.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow overview {}`.",
                        items.len(),
                        workflow_name
                    );
                    return Ok(true);
                }

                let command = match &items[index - 1] {
                    WorkflowOverviewFocusSelectionItem::Step(step_number) => {
                        WorkflowCommand::Panel {
                            workflow_name: Some(workflow_name.clone()),
                            step_number: Some(*step_number),
                        }
                    }
                    WorkflowOverviewFocusSelectionItem::Run(run_id) => WorkflowCommand::ShowRun {
                        run_id: Some(run_id.clone()),
                        step_number: None,
                    },
                };
                self.clear_selector_context();
                self.prepare_workflow_selector_context(session, &command);
                println!("{}", handle_workflow_command(command, session).await?);
                Ok(true)
            }
            SelectorContext::WorkflowPanelList { workflow_names } => {
                if index == 0 || index > workflow_names.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow panel`.",
                        workflow_names.len()
                    );
                    return Ok(true);
                }

                let command = WorkflowCommand::Panel {
                    workflow_name: Some(workflow_names[index - 1].clone()),
                    step_number: None,
                };
                self.clear_selector_context();
                self.prepare_workflow_selector_context(session, &command);
                println!("{}", handle_workflow_command(command, session).await?);
                Ok(true)
            }
            SelectorContext::WorkflowPanelItems {
                workflow_name,
                step_count: _,
                items,
            } => {
                if index == 0 || index > items.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow panel {}`.",
                        items.len(),
                        workflow_name
                    );
                    return Ok(true);
                }

                match &items[index - 1] {
                    WorkflowPanelSelectionItem::Step(step_number) => {
                        self.set_workflow_panel_focus(workflow_name.clone(), *step_number);
                        println!(
                            "{}",
                            handle_workflow_command(
                                WorkflowCommand::Panel {
                                    workflow_name: Some(workflow_name.clone()),
                                    step_number: Some(*step_number),
                                },
                                session,
                            )
                            .await?
                        );
                    }
                    WorkflowPanelSelectionItem::Run(run_id) => {
                        let command = WorkflowCommand::ShowRun {
                            run_id: Some(run_id.clone()),
                            step_number: None,
                        };
                        self.clear_selector_context();
                        self.prepare_workflow_selector_context(session, &command);
                        println!("{}", handle_workflow_command(command, session).await?);
                    }
                }
                Ok(true)
            }
            SelectorContext::WorkflowRunList { run_ids } => {
                if index == 0 || index > run_ids.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow runs`.",
                        run_ids.len()
                    );
                    return Ok(true);
                }

                let command = WorkflowCommand::ShowRun {
                    run_id: Some(run_ids[index - 1].clone()),
                    step_number: None,
                };
                self.clear_selector_context();
                self.prepare_workflow_selector_context(session, &command);
                println!("{}", handle_workflow_command(command, session).await?);
                Ok(true)
            }
            SelectorContext::WorkflowRunSteps { run_id, step_count } => {
                if index == 0 || index > *step_count {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow show-run {}`.",
                        step_count, run_id
                    );
                    return Ok(true);
                }

                let record = load_workflow_run(session.working_directory(), run_id)?;
                self.clear_selector_context();
                self.set_selector_context(SelectorContext::WorkflowRunSteps {
                    run_id: run_id.clone(),
                    step_count: *step_count,
                });
                self.set_workflow_run_focus(run_id.clone(), index);
                println!(
                    "{}",
                    render_workflow_run_inspect_panel_with_step(
                        session.working_directory(),
                        &record,
                        Some(index),
                    )
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

fn normalize_selected_step(step_number: Option<usize>, step_count: usize) -> Option<usize> {
    if step_count == 0 {
        None
    } else {
        match step_number {
            Some(0) => None,
            Some(step_number) if step_number > step_count => None,
            Some(step_number) => Some(step_number),
            None => Some(1),
        }
    }
}
