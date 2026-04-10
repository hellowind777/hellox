use super::*;
use crate::sessions::list_sessions;
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
        items: Vec<crate::workflow_overview::WorkflowOverviewSelectionItem>,
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
        self.clear_workflow_panel_focus();
    }

    pub(super) fn set_selector_context(&self, context: SelectorContext) {
        let keep_workflow_focus = matches!(context, SelectorContext::WorkflowPanelSteps { .. });
        if let Ok(mut guard) = self.selector_context.lock() {
            *guard = Some(context);
        }
        if !keep_workflow_focus {
            self.clear_workflow_panel_focus();
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

    pub(super) async fn handle_selector_index(
        &self,
        index: usize,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<bool> {
        let Some(context) = self.selector_context() else {
            return Ok(false);
        };

        if self
            .handle_workflow_selector_index(index, &context, session)
            .await?
        {
            return Ok(true);
        }

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
            _ => Ok(false),
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
