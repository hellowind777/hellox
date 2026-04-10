use super::*;
use crate::sessions::list_sessions;
use crate::workflow_overview::{
    list_workflow_overview_selection_items, WorkflowOverviewSelectionItem,
};
use crate::workflow_runs::{
    list_workflow_runs, load_latest_workflow_run, load_workflow_run,
    render_workflow_run_inspect_panel_with_step, WORKFLOW_RUN_SELECTOR_PREVIEW_LIMIT,
};
use crate::workflows::{list_workflows, load_named_workflow_detail};
use hellox_memory::{list_archived_memories, list_memories};

#[derive(Debug, Clone)]
pub(super) enum SelectorContext {
    SessionPanelList {
        session_ids: Vec<String>,
    },
    TaskPanelList {
        task_ids: Vec<String>,
    },
    MemoryPanelList {
        archived: bool,
        memory_ids: Vec<String>,
    },
    WorkflowOverviewList {
        items: Vec<WorkflowOverviewSelectionItem>,
    },
    WorkflowOverviewSteps {
        workflow_name: String,
        step_count: usize,
    },
    WorkflowPanelList {
        workflow_names: Vec<String>,
    },
    WorkflowPanelSteps {
        workflow_name: String,
        step_count: usize,
    },
    WorkflowRunList {
        run_ids: Vec<String>,
    },
    WorkflowRunSteps {
        run_id: String,
        step_count: usize,
    },
}

impl CliReplDriver {
    pub(super) fn clear_selector_context(&self) {
        if let Ok(mut guard) = self.selector_context.lock() {
            *guard = None;
        }
    }

    pub(super) fn set_selector_context(&self, context: SelectorContext) {
        if let Ok(mut guard) = self.selector_context.lock() {
            *guard = Some(context);
        }
    }

    pub(super) fn selector_context(&self) -> Option<SelectorContext> {
        self.selector_context
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub(super) fn prepare_memory_selector_context(
        &self,
        command: &MemoryCommand,
        metadata: &ReplMetadata,
    ) {
        if let MemoryCommand::Panel {
            archived,
            memory_id: None,
        } = command
        {
            let entries = if *archived {
                list_archived_memories(&metadata.memory_root)
            } else {
                list_memories(&metadata.memory_root)
            };
            if let Ok(entries) = entries {
                let memory_ids = entries
                    .into_iter()
                    .take(20)
                    .map(|entry| entry.memory_id)
                    .collect::<Vec<_>>();
                if !memory_ids.is_empty() {
                    self.set_selector_context(SelectorContext::MemoryPanelList {
                        archived: *archived,
                        memory_ids,
                    });
                }
            }
        }
    }

    pub(super) fn prepare_session_selector_context(
        &self,
        command: &SessionCommand,
        metadata: &ReplMetadata,
    ) {
        if matches!(command, SessionCommand::Panel { session_id: None }) {
            if let Ok(summaries) = list_sessions(&metadata.sessions_root) {
                self.set_selector_context(SelectorContext::SessionPanelList {
                    session_ids: summaries
                        .into_iter()
                        .map(|summary| summary.session_id)
                        .collect(),
                });
            }
        }
    }

    pub(super) fn prepare_task_selector_context(
        &self,
        command: &TaskCommand,
        session: &AgentSession,
    ) {
        if matches!(command, TaskCommand::Panel { task_id: None }) {
            if let Ok(tasks) = crate::tasks::load_tasks(session.working_directory()) {
                let task_ids = tasks.into_iter().map(|task| task.id).collect::<Vec<_>>();
                if !task_ids.is_empty() {
                    self.set_selector_context(SelectorContext::TaskPanelList { task_ids });
                }
            }
        }
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
                if let Ok(detail) =
                    load_named_workflow_detail(session.working_directory(), workflow_name)
                {
                    if !detail.steps.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowOverviewSteps {
                            workflow_name: detail.summary.name,
                            step_count: detail.steps.len(),
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
                ..
            } => {
                if let Ok(detail) =
                    load_named_workflow_detail(session.working_directory(), workflow_name)
                {
                    if !detail.steps.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowPanelSteps {
                            workflow_name: detail.summary.name,
                            step_count: detail.steps.len(),
                        });
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
            } => {
                if let Ok(record) = load_workflow_run(session.working_directory(), run_id) {
                    if !record.steps.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowRunSteps {
                            run_id: record.run_id,
                            step_count: record.steps.len(),
                        });
                    }
                }
            }
            WorkflowCommand::LastRun { workflow_name } => {
                if let Ok(record) =
                    load_latest_workflow_run(session.working_directory(), workflow_name.as_deref())
                {
                    if !record.steps.is_empty() {
                        self.set_selector_context(SelectorContext::WorkflowRunSteps {
                            run_id: record.run_id,
                            step_count: record.steps.len(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) async fn handle_selector_index(
        &self,
        index: usize,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<bool> {
        let Some(context) = self.selector_context() else {
            return Ok(false);
        };

        match context {
            SelectorContext::SessionPanelList { session_ids } => {
                if index == 0 || index > session_ids.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/session panel`.",
                        session_ids.len()
                    );
                    return Ok(true);
                }

                let session_id = session_ids[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_session_command(
                        SessionCommand::Panel {
                            session_id: Some(session_id),
                        },
                        session,
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::TaskPanelList { task_ids } => {
                if index == 0 || index > task_ids.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/tasks panel`.",
                        task_ids.len()
                    );
                    return Ok(true);
                }

                let task_id = task_ids[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_task_command(
                        TaskCommand::Panel {
                            task_id: Some(task_id),
                        },
                        session,
                    )?
                );
                Ok(true)
            }
            SelectorContext::MemoryPanelList {
                archived,
                memory_ids,
            } => {
                if index == 0 || index > memory_ids.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/memory panel{}`.",
                        memory_ids.len(),
                        if archived { " --archived" } else { "" }
                    );
                    return Ok(true);
                }

                let memory_id = memory_ids[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_memory_command(
                        MemoryCommand::Panel {
                            archived,
                            memory_id: Some(memory_id),
                        },
                        session,
                        metadata,
                    )?
                );
                Ok(true)
            }
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
                        };
                        self.prepare_workflow_selector_context(session, &command);
                        println!("{}", handle_workflow_command(command, session).await?);
                    }
                }
                Ok(true)
            }
            SelectorContext::WorkflowOverviewSteps {
                workflow_name,
                step_count,
            } => {
                if index == 0 || index > step_count {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow overview {}`.",
                        step_count, workflow_name
                    );
                    return Ok(true);
                }

                let command = WorkflowCommand::Panel {
                    workflow_name: Some(workflow_name),
                    step_number: Some(index),
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

                let workflow_name = workflow_names[index - 1].clone();
                let command = WorkflowCommand::Panel {
                    workflow_name: Some(workflow_name),
                    step_number: None,
                };
                self.clear_selector_context();
                self.prepare_workflow_selector_context(session, &command);
                println!("{}", handle_workflow_command(command, session).await?);
                Ok(true)
            }
            SelectorContext::WorkflowPanelSteps {
                workflow_name,
                step_count,
            } => {
                if index == 0 || index > step_count {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow panel {}`.",
                        step_count, workflow_name
                    );
                    return Ok(true);
                }

                let command = WorkflowCommand::Panel {
                    workflow_name: Some(workflow_name.clone()),
                    step_number: Some(index),
                };
                self.clear_selector_context();
                self.set_selector_context(SelectorContext::WorkflowPanelSteps {
                    workflow_name,
                    step_count,
                });
                println!("{}", handle_workflow_command(command, session).await?);
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

                let run_id = run_ids[index - 1].clone();
                let command = WorkflowCommand::ShowRun {
                    run_id: Some(run_id),
                };
                self.clear_selector_context();
                self.prepare_workflow_selector_context(session, &command);
                println!("{}", handle_workflow_command(command, session).await?);
                Ok(true)
            }
            SelectorContext::WorkflowRunSteps { run_id, step_count } => {
                if index == 0 || index > step_count {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/workflow show-run {}`.",
                        step_count, run_id
                    );
                    return Ok(true);
                }

                let record = load_workflow_run(session.working_directory(), &run_id)?;
                self.clear_selector_context();
                self.set_selector_context(SelectorContext::WorkflowRunSteps { run_id, step_count });
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
        }
    }
}

pub(super) fn parse_selector_index(input: &str) -> Option<usize> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > 6 {
        return None;
    }
    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        trimmed.parse::<usize>().ok()
    } else {
        None
    }
}
