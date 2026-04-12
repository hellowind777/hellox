use super::commands::{
    BridgeCommand, ConfigCommand, McpCommand, MemoryCommand, ModelCommand, OutputStyleCommand,
    PersonaCommand, PlanCommand, PluginCommand, PromptFragmentCommand, RemoteEnvCommand,
    SessionCommand, TaskCommand,
};
use super::selectors::SelectorContext;
use super::*;

impl CliReplDriver {
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
                        "{}",
                        invalid_selection_text(self.language, focus_keys.len(), "/config panel")
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
            SelectorContext::BridgePanelList { session_ids } => {
                if index == 0 || index > session_ids.len() {
                    println!(
                        "{}",
                        invalid_selection_text(self.language, session_ids.len(), "/bridge panel")
                    );
                    return Ok(true);
                }

                let session_id = session_ids[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    crate::repl::bridge_actions::handle_bridge_command(
                        BridgeCommand::Panel {
                            session_id: Some(session_id),
                        },
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::RemoteEnvPanelList { environment_names } => {
                if index == 0 || index > environment_names.len() {
                    println!(
                        "{}",
                        invalid_selection_text(
                            self.language,
                            environment_names.len(),
                            "/remote-env panel",
                        )
                    );
                    return Ok(true);
                }

                let environment_name = environment_names[index - 1].clone();
                self.clear_selector_context();
                println!(
                    "{}",
                    crate::repl::remote_actions::handle_remote_env_command(
                        RemoteEnvCommand::Panel {
                            environment_name: Some(environment_name),
                        },
                        metadata,
                    )?
                );
                Ok(true)
            }
            SelectorContext::PlanPanelSteps { step_count } => {
                if index == 0 || index > step_count {
                    println!(
                        "{}",
                        invalid_selection_text(self.language, step_count, "/plan panel")
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
                        "{}",
                        invalid_selection_text(self.language, session_ids.len(), "/session panel")
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
                        self.language,
                    )?
                );
                Ok(true)
            }
            SelectorContext::TaskPanelList { task_ids } => {
                if index == 0 || index > task_ids.len() {
                    println!(
                        "{}",
                        invalid_selection_text(self.language, task_ids.len(), "/tasks panel")
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
                    let rerun =
                        format!("/memory panel{}", if archived { " --archived" } else { "" });
                    println!(
                        "{}",
                        invalid_selection_text(self.language, memory_ids.len(), &rerun)
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
                        self.language,
                    )?
                );
                Ok(true)
            }
            SelectorContext::ModelPanelList { profile_names } => {
                if index == 0 || index > profile_names.len() {
                    println!(
                        "{}",
                        invalid_selection_text(self.language, profile_names.len(), "/model panel")
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
                        self.language,
                    )?
                );
                Ok(true)
            }
            SelectorContext::OutputStylePanelList { style_names } => {
                if index == 0 || index > style_names.len() {
                    println!(
                        "{}",
                        invalid_selection_text(
                            self.language,
                            style_names.len(),
                            "/output-style panel",
                        )
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
                        "{}",
                        invalid_selection_text(
                            self.language,
                            persona_names.len(),
                            "/persona panel",
                        )
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
                        "{}",
                        invalid_selection_text(
                            self.language,
                            fragment_names.len(),
                            "/fragment panel",
                        )
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
                        "{}",
                        invalid_selection_text(self.language, server_names.len(), "/mcp panel")
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
                        "{}",
                        invalid_selection_text(self.language, plugin_ids.len(), "/plugin panel")
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

fn invalid_selection_text(language: AppLanguage, upper_bound: usize, rerun: &str) -> String {
    match language {
        AppLanguage::English => {
            format!("Invalid selection. Choose 1..{upper_bound} or re-run `{rerun}`.")
        }
        AppLanguage::SimplifiedChinese => {
            format!("选择无效。请选择 1..{upper_bound}，或重新运行 `{rerun}`。")
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
