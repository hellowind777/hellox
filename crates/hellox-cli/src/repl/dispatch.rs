use super::*;

pub(super) async fn handle_repl_input_async_impl(
    driver: &CliReplDriver,
    input: &str,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
) -> Result<ReplAction> {
    if matches!(input, "exit" | "quit") {
        return Ok(ReplAction::Exit);
    }

    let Some(command) = parse_command(input) else {
        return Ok(ReplAction::Submit(input.to_string()));
    };

    match command {
        ReplCommand::Exit => Ok(ReplAction::Exit),
        ReplCommand::Help => {
            println!(
                "{}",
                help_text_for_workdir(session.working_directory(), driver.language)
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Status => {
            println!("{}", status_text(session, metadata, driver.language));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Doctor => {
            println!("{}", doctor_text(session, metadata, driver.language));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Usage => {
            println!("{}", usage_text(session, driver.language));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Stats => {
            println!("{}", stats_text(session, driver.language));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Cost => {
            println!("{}", cost_text(session, driver.language));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Brief(command) => {
            println!("{}", handle_brief_command(command, session)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Tools(command) => {
            println!("{}", handle_tools_command(command)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Install(command) => {
            println!("{}", handle_install_command(command)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Upgrade(command) => {
            println!("{}", handle_upgrade_command(command)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::OutputStyle(command) => {
            driver.prepare_output_style_selector_context(&command, session, metadata);
            println!(
                "{}",
                handle_output_style_command(command, session, metadata)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Persona(command) => {
            driver.prepare_persona_selector_context(&command, session, metadata);
            println!("{}", handle_persona_command(command, session, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::PromptFragment(command) => {
            driver.prepare_prompt_fragment_selector_context(&command, session, metadata);
            println!(
                "{}",
                handle_prompt_fragment_command(command, session, metadata)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Search { query: None } => {
            println!("{}", search_usage_text(driver.language));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Search { query: Some(query) } => {
            println!(
                "{}",
                search_text(
                    session,
                    metadata,
                    &query,
                    DEFAULT_SEARCH_LIMIT,
                    driver.language
                )
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Skills { name } => {
            println!("{}", handle_skills_command(name, session)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Hooks { name } => {
            println!("{}", handle_hooks_command(name, session)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::RemoteEnv(command) => {
            driver.prepare_remote_env_selector_context(&command, metadata);
            println!("{}", handle_remote_env_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Teleport(command) => {
            println!("{}", handle_teleport_command(command, session, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Assistant(command) => {
            println!("{}", handle_assistant_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Bridge(command) => {
            driver.prepare_bridge_selector_context(&command, metadata);
            println!("{}", handle_bridge_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Ide(command) => {
            println!("{}", handle_ide_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Mcp(command) => {
            driver.prepare_mcp_selector_context(&command, metadata);
            println!("{}", handle_mcp_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Plugin(command) => {
            driver.prepare_plugin_selector_context(&command, metadata);
            println!("{}", handle_plugin_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Memory(command) => {
            driver.prepare_memory_selector_context(&command, metadata);
            println!(
                "{}",
                handle_memory_command(command, session, metadata, driver.language)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Session(command) => {
            driver.prepare_session_selector_context(&command, metadata);
            println!(
                "{}",
                handle_session_command(command, session, metadata, driver.language)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Tasks(command) => {
            driver.prepare_task_selector_context(&command, session);
            println!("{}", handle_task_command(command, session)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Workflow(command) => {
            if let WorkflowCommand::Dashboard {
                workflow_name,
                script_path,
            } = &command
            {
                println!(
                    "{}",
                    driver.open_workflow_dashboard(
                        session,
                        workflow_name.clone(),
                        script_path.clone(),
                    )?
                );
                return Ok(ReplAction::Continue);
            }
            driver.prepare_workflow_selector_context(session, &command);
            println!("{}", handle_workflow_command(command, session).await?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Config(command) => {
            driver.prepare_config_selector_context(&command, metadata);
            println!("{}", handle_config_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Plan(command) => {
            driver.prepare_plan_selector_context(&command, session);
            println!("{}", handle_plan_command(command, session).await?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Permissions { value } => {
            println!(
                "{}",
                handle_permissions_command(value, session, driver.language)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Resume { session_id } => {
            match handle_resume_command(session_id, metadata, driver.language)? {
                ResumeAction::Continue(message) => {
                    println!("{message}");
                    Ok(ReplAction::Continue)
                }
                ResumeAction::Resume(session_id) => {
                    println!("{}", resuming_session_text(driver.language, &session_id));
                    Ok(ReplAction::Resume(session_id))
                }
            }
        }
        ReplCommand::Share { path } => {
            println!(
                "{}",
                handle_share_command(path, session, metadata, driver.language)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Compact { instructions } => {
            println!(
                "{}",
                handle_compact_command(instructions, session, metadata, driver.language)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Rewind => {
            println!("{}", handle_rewind_command(session, driver.language)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Clear => {
            let cleared = session.clear_messages()?;
            println!("{}", cleared_messages_text(driver.language, cleared));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Model(command) => {
            driver.prepare_model_selector_context(&command, session, metadata);
            println!(
                "{}",
                handle_model_command(command, session, metadata, driver.language)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Unknown(name) => {
            if let Some((workflow_name, shared_context)) =
                resolve_dynamic_workflow_invocation(input, session.working_directory())?
            {
                println!(
                    "{}",
                    handle_workflow_command(
                        WorkflowCommand::Run {
                            workflow_name: Some(workflow_name),
                            script_path: None,
                            shared_context,
                        },
                        session,
                    )
                    .await?,
                );
                return Ok(ReplAction::Continue);
            }
            println!("{}", unknown_command_text(driver.language, &name));
            Ok(ReplAction::Continue)
        }
    }
}

fn search_usage_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Usage: /search <query>",
        AppLanguage::SimplifiedChinese => "用法：/search <query>",
    }
}

fn resuming_session_text(language: AppLanguage, session_id: &str) -> String {
    match language {
        AppLanguage::English => format!("Resuming session `{session_id}`..."),
        AppLanguage::SimplifiedChinese => format!("正在恢复会话 `{session_id}`……"),
    }
}

fn cleared_messages_text(language: AppLanguage, cleared: usize) -> String {
    match language {
        AppLanguage::English => {
            format!("Cleared {cleared} message(s) from the current session.")
        }
        AppLanguage::SimplifiedChinese => {
            format!("已从当前会话中清除 {cleared} 条消息。")
        }
    }
}

fn unknown_command_text(language: AppLanguage, name: &str) -> String {
    match language {
        AppLanguage::English => {
            format!("Unknown command `/{name}`. Use `/help` to list available commands.")
        }
        AppLanguage::SimplifiedChinese => {
            format!("未知命令 `/{name}`。请使用 `/help` 查看可用命令。")
        }
    }
}
