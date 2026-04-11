use super::commands::{
    ConfigCommand, McpCommand, ModelCommand, OutputStyleCommand, PersonaCommand, PlanCommand,
    PluginCommand, PromptFragmentCommand,
};
use super::*;
use crate::config_panel::config_selector_keys;
use crate::mcp_panel::mcp_panel_server_names;
use crate::model_panel::model_panel_profile_names;
use crate::plugin_panel::plugin_panel_ids;
use crate::sessions::list_sessions;
use crate::style_panels::{
    output_style_panel_names, persona_panel_names, prompt_fragment_panel_names,
};
use hellox_memory::{list_archived_memories, list_memories};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WorkflowOverviewFocusTarget {
    Named(String),
    Path(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WorkflowRunListTarget {
    Named(String),
    Path(String),
}

#[derive(Debug, Clone)]
pub(super) enum SelectorContext {
    ConfigPanelList {
        focus_keys: Vec<String>,
    },
    PlanPanelSteps {
        step_count: usize,
    },
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
    ModelPanelList {
        profile_names: Vec<String>,
    },
    OutputStylePanelList {
        style_names: Vec<String>,
    },
    PersonaPanelList {
        persona_names: Vec<String>,
    },
    PromptFragmentPanelList {
        fragment_names: Vec<String>,
    },
    McpPanelList {
        server_names: Vec<String>,
    },
    PluginPanelList {
        plugin_ids: Vec<String>,
    },
    WorkflowOverviewList {
        items: Vec<crate::workflow_overview::WorkflowOverviewSelectionItem>,
    },
    WorkflowOverviewFocusItems {
        target: WorkflowOverviewFocusTarget,
        items: Vec<crate::workflow_overview::WorkflowOverviewFocusSelectionItem>,
    },
    WorkflowPanelList {
        workflow_names: Vec<String>,
    },
    WorkflowPanelItems {
        workflow_name: String,
        step_count: usize,
        items: Vec<crate::workflow_panel::WorkflowPanelSelectionItem>,
    },
    WorkflowPanelPathItems {
        script_path: String,
        workflow_name: String,
        step_count: usize,
        items: Vec<crate::workflow_panel::WorkflowPanelSelectionItem>,
    },
    WorkflowRunList {
        target: Option<WorkflowRunListTarget>,
        run_ids: Vec<String>,
    },
    WorkflowRunSteps {
        run_id: String,
        step_count: usize,
    },
}

impl CliReplDriver {
    pub(super) fn prepare_config_selector_context(
        &self,
        command: &ConfigCommand,
        metadata: &ReplMetadata,
    ) {
        if let ConfigCommand::Panel { focus_key: None } = command {
            let keys = config_selector_keys(&metadata.config);
            if !keys.is_empty() {
                self.set_selector_context(SelectorContext::ConfigPanelList { focus_keys: keys });
            }
        }
    }

    pub(super) fn prepare_plan_selector_context(
        &self,
        command: &PlanCommand,
        session: &AgentSession,
    ) {
        if let PlanCommand::Panel { step_number: None } = command {
            let step_count = session.planning_state().plan.len();
            if step_count > 0 {
                self.set_selector_context(SelectorContext::PlanPanelSteps { step_count });
            }
        }
    }

    pub(super) fn clear_selector_context(&self) {
        if let Ok(mut guard) = self.selector_context.lock() {
            *guard = None;
        }
        self.clear_workflow_panel_focus();
        self.clear_workflow_run_focus();
    }

    pub(super) fn set_selector_context(&self, context: SelectorContext) {
        let keep_workflow_panel_focus = matches!(
            context,
            SelectorContext::WorkflowPanelItems { .. }
                | SelectorContext::WorkflowPanelPathItems { .. }
        );
        let keep_workflow_run_focus = matches!(context, SelectorContext::WorkflowRunSteps { .. });
        if let Ok(mut guard) = self.selector_context.lock() {
            *guard = Some(context);
        }
        if !keep_workflow_panel_focus {
            self.clear_workflow_panel_focus();
        }
        if !keep_workflow_run_focus {
            self.clear_workflow_run_focus();
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

    pub(super) fn prepare_model_selector_context(
        &self,
        command: &ModelCommand,
        _session: &AgentSession,
        metadata: &ReplMetadata,
    ) {
        if let ModelCommand::Panel { profile_name: None } = command {
            let config = hellox_config::load_or_default(Some(metadata.config_path.clone()))
                .unwrap_or_else(|_| metadata.config.clone());
            let profile_names = model_panel_profile_names(&config);
            if !profile_names.is_empty() {
                self.set_selector_context(SelectorContext::ModelPanelList { profile_names });
            }
        }
    }

    pub(super) fn prepare_output_style_selector_context(
        &self,
        command: &OutputStyleCommand,
        session: &AgentSession,
        _metadata: &ReplMetadata,
    ) {
        if let OutputStyleCommand::Panel { style_name: None } = command {
            if let Ok(style_names) = output_style_panel_names(session.working_directory()) {
                if !style_names.is_empty() {
                    self.set_selector_context(SelectorContext::OutputStylePanelList {
                        style_names,
                    });
                }
            }
        }
    }

    pub(super) fn prepare_persona_selector_context(
        &self,
        command: &PersonaCommand,
        session: &AgentSession,
        _metadata: &ReplMetadata,
    ) {
        if let PersonaCommand::Panel { persona_name: None } = command {
            if let Ok(persona_names) = persona_panel_names(session.working_directory()) {
                if !persona_names.is_empty() {
                    self.set_selector_context(SelectorContext::PersonaPanelList { persona_names });
                }
            }
        }
    }

    pub(super) fn prepare_prompt_fragment_selector_context(
        &self,
        command: &PromptFragmentCommand,
        session: &AgentSession,
        _metadata: &ReplMetadata,
    ) {
        if let PromptFragmentCommand::Panel {
            fragment_name: None,
        } = command
        {
            if let Ok(fragment_names) = prompt_fragment_panel_names(session.working_directory()) {
                if !fragment_names.is_empty() {
                    self.set_selector_context(SelectorContext::PromptFragmentPanelList {
                        fragment_names,
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

    pub(super) fn prepare_mcp_selector_context(
        &self,
        command: &McpCommand,
        metadata: &ReplMetadata,
    ) {
        if let McpCommand::Panel { server_name: None } = command {
            let config = hellox_config::load_or_default(Some(metadata.config_path.clone()))
                .unwrap_or_else(|_| metadata.config.clone());
            let server_names = mcp_panel_server_names(&config);
            if !server_names.is_empty() {
                self.set_selector_context(SelectorContext::McpPanelList { server_names });
            }
        }
    }

    pub(super) fn prepare_plugin_selector_context(
        &self,
        command: &PluginCommand,
        metadata: &ReplMetadata,
    ) {
        if let PluginCommand::Panel { plugin_id: None } = command {
            let config = hellox_config::load_or_default(Some(metadata.config_path.clone()))
                .unwrap_or_else(|_| metadata.config.clone());
            let plugin_ids = plugin_panel_ids(&config);
            if !plugin_ids.is_empty() {
                self.set_selector_context(SelectorContext::PluginPanelList { plugin_ids });
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
            SelectorContext::ConfigPanelList { focus_keys } => {
                if index == 0 || index > focus_keys.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/config panel`.",
                        focus_keys.len()
                    );
                    return Ok(true);
                }

                let focus_key = focus_keys[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_config_command(
                        ConfigCommand::Panel {
                            focus_key: Some(focus_key),
                        },
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::PlanPanelSteps { step_count } => {
                if index == 0 || index > step_count {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/plan panel`.",
                        step_count
                    );
                    return Ok(true);
                }

                self.clear_selector_context();
                println!(
                    "{}",
                    handle_plan_command(
                        PlanCommand::Panel {
                            step_number: Some(index),
                        },
                        session,
                    )
                    .await?
                );
                Ok(true)
            }
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
            SelectorContext::ModelPanelList { profile_names } => {
                if index == 0 || index > profile_names.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/model panel`.",
                        profile_names.len()
                    );
                    return Ok(true);
                }

                let profile_name = profile_names[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_model_command(
                        ModelCommand::Panel {
                            profile_name: Some(profile_name),
                        },
                        session,
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::OutputStylePanelList { style_names } => {
                if index == 0 || index > style_names.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/output-style panel`.",
                        style_names.len()
                    );
                    return Ok(true);
                }

                let style_name = style_names[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_output_style_command(
                        OutputStyleCommand::Panel {
                            style_name: Some(style_name),
                        },
                        session,
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::PersonaPanelList { persona_names } => {
                if index == 0 || index > persona_names.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/persona panel`.",
                        persona_names.len()
                    );
                    return Ok(true);
                }

                let persona_name = persona_names[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_persona_command(
                        PersonaCommand::Panel {
                            persona_name: Some(persona_name),
                        },
                        session,
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::PromptFragmentPanelList { fragment_names } => {
                if index == 0 || index > fragment_names.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/fragment panel`.",
                        fragment_names.len()
                    );
                    return Ok(true);
                }

                let fragment_name = fragment_names[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_prompt_fragment_command(
                        PromptFragmentCommand::Panel {
                            fragment_name: Some(fragment_name),
                        },
                        session,
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::McpPanelList { server_names } => {
                if index == 0 || index > server_names.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/mcp panel`.",
                        server_names.len()
                    );
                    return Ok(true);
                }

                let server_name = server_names[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_mcp_command(
                        McpCommand::Panel {
                            server_name: Some(server_name),
                        },
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::PluginPanelList { plugin_ids } => {
                if index == 0 || index > plugin_ids.len() {
                    println!(
                        "Invalid selection. Choose 1..{} or re-run `/plugin panel`.",
                        plugin_ids.len()
                    );
                    return Ok(true);
                }

                let plugin_id = plugin_ids[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    handle_plugin_command(
                        PluginCommand::Panel {
                            plugin_id: Some(plugin_id),
                        },
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
