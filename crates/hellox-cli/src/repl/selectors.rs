use super::commands::{
    BridgeCommand, ConfigCommand, McpCommand, ModelCommand, OutputStyleCommand, PersonaCommand,
    PlanCommand, PluginCommand, PromptFragmentCommand, RemoteEnvCommand,
};
use super::*;
use crate::bridge_panel::bridge_panel_session_ids;
use crate::config_panel::config_selector_keys;
use crate::mcp_panel::mcp_panel_server_names;
use crate::model_panel::model_panel_profile_names;
use crate::plugin_panel::plugin_panel_ids;
use crate::remote_panel::remote_env_panel_names;
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
    BridgePanelList {
        session_ids: Vec<String>,
    },
    RemoteEnvPanelList {
        environment_names: Vec<String>,
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

    pub(super) fn prepare_bridge_selector_context(
        &self,
        command: &BridgeCommand,
        metadata: &ReplMetadata,
    ) {
        if let BridgeCommand::Panel { session_id: None } = command {
            let paths = hellox_bridge::BridgeRuntimePaths::new(
                metadata.config_path.clone(),
                metadata.sessions_root.clone(),
                metadata.plugins_root.clone(),
            );
            if let Ok(session_ids) = bridge_panel_session_ids(&paths) {
                if !session_ids.is_empty() {
                    self.set_selector_context(SelectorContext::BridgePanelList { session_ids });
                }
            }
        }
    }

    pub(super) fn prepare_remote_env_selector_context(
        &self,
        command: &RemoteEnvCommand,
        metadata: &ReplMetadata,
    ) {
        if let RemoteEnvCommand::Panel {
            environment_name: None,
        } = command
        {
            let config = hellox_config::load_or_default(Some(metadata.config_path.clone()))
                .unwrap_or_else(|_| metadata.config.clone());
            let environment_names = remote_env_panel_names(&config);
            if !environment_names.is_empty() {
                self.set_selector_context(SelectorContext::RemoteEnvPanelList {
                    environment_names,
                });
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
}
