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
            println!("{}", help_text_for_workdir(session.working_directory()));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Status => {
            println!("{}", status_text(session, metadata));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Doctor => {
            println!("{}", doctor_text(session, metadata));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Usage => {
            println!("{}", usage_text(session));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Stats => {
            println!("{}", stats_text(session));
            Ok(ReplAction::Continue)
        }
        ReplCommand::Cost => {
            println!("{}", cost_text(session));
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
            println!(
                "{}",
                handle_output_style_command(command, session, metadata)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Persona(command) => {
            println!("{}", handle_persona_command(command, session, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::PromptFragment(command) => {
            println!(
                "{}",
                handle_prompt_fragment_command(command, session, metadata)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Search { query: None } => {
            println!("Usage: /search <query>");
            Ok(ReplAction::Continue)
        }
        ReplCommand::Search { query: Some(query) } => {
            println!(
                "{}",
                search_text(session, metadata, &query, DEFAULT_SEARCH_LIMIT)
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
            println!("{}", handle_bridge_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Ide(command) => {
            println!("{}", handle_ide_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Mcp(command) => {
            println!("{}", handle_mcp_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Plugin(command) => {
            println!("{}", handle_plugin_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Memory(command) => {
            driver.prepare_memory_selector_context(&command, metadata);
            println!("{}", handle_memory_command(command, session, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Session(command) => {
            driver.prepare_session_selector_context(&command, metadata);
            println!("{}", handle_session_command(command, session, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Tasks(command) => {
            driver.prepare_task_selector_context(&command, session);
            println!("{}", handle_task_command(command, session)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Workflow(command) => {
            driver.prepare_workflow_selector_context(session, &command);
            println!("{}", handle_workflow_command(command, session).await?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Config(command) => {
            println!("{}", handle_config_command(command, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Plan(command) => {
            println!("{}", handle_plan_command(command, session).await?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Permissions { value } => {
            println!("{}", handle_permissions_command(value, session)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Resume { session_id } => match handle_resume_command(session_id, metadata)? {
            ResumeAction::Continue(message) => {
                println!("{message}");
                Ok(ReplAction::Continue)
            }
            ResumeAction::Resume(session_id) => {
                println!("Resuming session `{session_id}`...");
                Ok(ReplAction::Resume(session_id))
            }
        },
        ReplCommand::Share { path } => {
            println!("{}", handle_share_command(path, session, metadata)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Compact { instructions } => {
            println!(
                "{}",
                handle_compact_command(instructions, session, metadata)?
            );
            Ok(ReplAction::Continue)
        }
        ReplCommand::Rewind => {
            println!("{}", handle_rewind_command(session)?);
            Ok(ReplAction::Continue)
        }
        ReplCommand::Clear => {
            let cleared = session.clear_messages()?;
            println!("Cleared {cleared} message(s) from the current session.");
            Ok(ReplAction::Continue)
        }
        ReplCommand::Model(command) => {
            println!("{}", handle_model_command(command, session, metadata)?);
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
                            shared_context,
                        },
                        session,
                    )
                    .await?,
                );
                return Ok(ReplAction::Continue);
            }
            println!("Unknown command `/{name}`. Use `/help` to list available commands.");
            Ok(ReplAction::Continue)
        }
    }
}
