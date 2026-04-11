mod assistant_panel;
mod auth_commands;
mod auto_compact;
mod auto_memory;
mod bridge_commands;
mod cli_auth_types;
mod cli_bridge_types;
mod cli_commands;
mod cli_extension_types;
mod cli_install_types;
mod cli_plan_types;
mod cli_remote_types;
mod cli_server_types;
mod cli_style_types;
mod cli_sync_types;
mod cli_task_types;
mod cli_types;
mod cli_ui_types;
mod cli_workflow_types;
mod config_commands;
mod config_panel;
mod diagnostics;
mod extension_commands;
mod hooks;
mod install_commands;
mod mcp_panel;
mod memory;
mod memory_panel;
mod model_panel;
mod output_style_commands;
mod output_styles;
mod persona_commands;
mod personas;
mod plan_commands;
mod plan_panel;
mod plugin_commands;
mod plugin_panel;
mod prompt_fragment_commands;
mod prompt_fragments;
mod remote_commands;
mod repl;
mod search;
mod server_commands;
mod session_panel;
mod sessions;
mod settings_commands;
mod skills;
mod style_command_support;
mod style_panels;
mod sync_commands;
mod task_commands;
mod task_panel;
mod tasks;
mod team_memory_panel;
mod transcript;
mod ui_commands;
mod usage;
mod worker_runner;
mod workflow_authoring;
mod workflow_command_authoring;
mod workflow_command_support;
mod workflow_commands;
mod workflow_dashboard;
mod workflow_overview;
mod workflow_panel;
mod workflow_runs;
mod workflow_step_navigation;
mod workflow_step_shortcuts;
mod workflows;

#[cfg(test)]
mod main_extension_tests;
#[cfg(test)]
mod main_state_tests;
#[cfg(test)]
mod main_style_tests;
#[cfg(test)]
mod main_task_tests;
#[cfg(test)]
mod main_tests;
#[cfg(test)]
mod main_ui_tests;
#[cfg(test)]
mod server_admin_tests;
#[cfg(test)]
mod workflow_authoring_tests;
#[cfg(test)]
mod workflow_commands_tests;
#[cfg(test)]
mod workflow_repl_authoring_tests;
#[cfg(test)]
mod workflows_tests;

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use hellox_agent::{
    default_tool_registry, AgentOptions, AgentSession, ConsoleApprovalHandler, GatewayClient,
    StoredSession,
};
use hellox_config::{
    default_config_path, default_config_toml, load_or_default, memory_root, plugins_root,
    session_file_path, sessions_root, shares_root,
};

use crate::auth_commands::handle_auth_command;
use crate::auto_compact::{format_auto_compact_notice, maybe_auto_compact_session};
use crate::auto_memory::{format_auto_memory_refresh_notice, maybe_auto_refresh_session_memory};
use crate::bridge_commands::{handle_bridge_command, handle_ide_command};
use crate::cli_commands::{
    handle_cost_command, handle_doctor_command, handle_mcp_command, handle_memory_command,
    handle_search, handle_session_command, handle_stats_command, handle_status_command,
    handle_usage_command,
};
use crate::cli_types::{Cli, Commands, GatewayCommands};
use crate::config_commands::handle_config_command;
use crate::extension_commands::{handle_hooks_command, handle_skills_command};
use crate::install_commands::{handle_install_command, handle_upgrade_command};
use crate::output_style_commands::handle_output_style_command;
use crate::persona_commands::handle_persona_command;
use crate::plan_commands::handle_plan_command;
use crate::plugin_commands::handle_plugin_command;
use crate::prompt_fragment_commands::handle_prompt_fragment_command;
use crate::remote_commands::{
    handle_assistant_command, handle_remote_env_command, handle_teleport_command,
};
use crate::repl::{run_repl, ReplExit, ReplMetadata};
use crate::server_commands::handle_server_command;
use crate::settings_commands::{handle_model_command, handle_permissions_command};
use crate::sync_commands::handle_sync_command;
use crate::task_commands::handle_tasks_command;
use crate::ui_commands::{handle_brief_command, handle_tools_command};
use crate::usage::print_usage;
use crate::worker_runner::run_worker_job;
use crate::workflow_commands::handle_workflow_command;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Gateway { command }) => match command {
            GatewayCommands::Serve { config } => hellox_gateway::serve(config).await?,
            GatewayCommands::PrintDefaultConfig => {
                println!("{}", default_config_toml());
            }
        },
        Some(Commands::Brief { command }) => handle_brief_command(command)?,
        Some(Commands::Tools { command }) => handle_tools_command(command)?,
        Some(Commands::Config { command }) => handle_config_command(command)?,
        Some(Commands::Plan { command }) => handle_plan_command(command)?,
        Some(Commands::OutputStyle { command }) => handle_output_style_command(command)?,
        Some(Commands::Persona { command }) => handle_persona_command(command)?,
        Some(Commands::PromptFragment { command }) => handle_prompt_fragment_command(command)?,
        Some(Commands::Model { command }) => handle_model_command(command)?,
        Some(Commands::Permissions { command }) => handle_permissions_command(command)?,
        Some(Commands::Memory { command }) => handle_memory_command(command)?,
        Some(Commands::Auth { command }) => handle_auth_command(command)?,
        Some(Commands::Sync { command }) => handle_sync_command(command)?,
        Some(Commands::Doctor) => handle_doctor_command()?,
        Some(Commands::Status) => handle_status_command()?,
        Some(Commands::Usage) => handle_usage_command()?,
        Some(Commands::Stats) => handle_stats_command()?,
        Some(Commands::Cost) => handle_cost_command()?,
        Some(Commands::Install { command }) => handle_install_command(command)?,
        Some(Commands::Upgrade { command }) => handle_upgrade_command(command)?,
        Some(Commands::Search { query, limit }) => handle_search(query, limit)?,
        Some(Commands::Skills { name }) => handle_skills_command(name)?,
        Some(Commands::Hooks { name }) => handle_hooks_command(name)?,
        Some(Commands::Server { command }) => handle_server_command(command).await?,
        Some(Commands::RemoteEnv { command }) => handle_remote_env_command(command)?,
        Some(Commands::Teleport { command }) => handle_teleport_command(command)?,
        Some(Commands::Assistant { command }) => handle_assistant_command(command)?,
        Some(Commands::Bridge { command }) => handle_bridge_command(command)?,
        Some(Commands::Ide { command }) => handle_ide_command(command)?,
        Some(Commands::Tasks { command }) => handle_tasks_command(command)?,
        Some(Commands::Workflow { command }) => handle_workflow_command(command).await?,
        Some(Commands::Mcp { command }) => handle_mcp_command(command)?,
        Some(Commands::Plugin { command }) => handle_plugin_command(command)?,
        Some(Commands::Session { command }) => handle_session_command(command)?,
        Some(Commands::Chat {
            prompt,
            model,
            gateway_url,
            config,
            cwd,
            session_id,
            max_turns,
        }) => {
            let mut bootstrap =
                build_session(config, gateway_url, model, cwd, session_id, max_turns)?;
            let result = bootstrap.session.run_user_prompt(prompt).await?;
            println!("{}", result.final_text);
            match maybe_auto_compact_session(
                &mut bootstrap.session,
                &bootstrap.repl_metadata.memory_root,
            )? {
                Some(outcome) => println!("{}", format_auto_compact_notice(&outcome)),
                None => match maybe_auto_refresh_session_memory(
                    &bootstrap.session,
                    &bootstrap.repl_metadata.memory_root,
                )? {
                    Some(outcome) => println!("{}", format_auto_memory_refresh_notice(&outcome)),
                    None => {}
                },
            }
        }
        Some(Commands::Repl {
            model,
            gateway_url,
            config,
            cwd,
            session_id,
            max_turns,
        }) => {
            let config_arg = config.clone();
            let gateway_url_arg = gateway_url.clone();
            let cwd_arg = cwd.clone();
            let mut active_session_id = session_id.clone();
            let mut active_model_override = model.clone();

            loop {
                let mut bootstrap = build_session(
                    config_arg.clone(),
                    gateway_url_arg.clone(),
                    active_model_override.clone(),
                    cwd_arg.clone(),
                    active_session_id.clone(),
                    max_turns,
                )?;
                match run_repl(&mut bootstrap.session, &bootstrap.repl_metadata).await? {
                    ReplExit::Exit => break,
                    ReplExit::Resume(session_id) => {
                        active_session_id = Some(session_id);
                        active_model_override = None;
                    }
                }
            }
        }
        Some(Commands::WorkerRunAgent { job }) => run_worker_job(job).await?,
        None => print_usage(),
    }

    Ok(())
}

pub(crate) fn build_session(
    config: Option<PathBuf>,
    gateway_url: Option<String>,
    model: Option<String>,
    cwd: Option<PathBuf>,
    session_id: Option<String>,
    max_turns: usize,
) -> Result<SessionBootstrap> {
    let config_path = config.clone().unwrap_or_else(default_config_path);
    let current = load_or_default(Some(config_path.clone()))?;
    let permission_mode = current.permissions.mode.clone();
    let persist = current.session.persist;
    let telemetry_sink = Some(hellox_telemetry::default_jsonl_telemetry_sink());
    let gateway =
        GatewayClient::from_config(&current, gateway_url).with_telemetry(telemetry_sink.clone());
    let shell_name = current_shell_name();
    let console_handler = Arc::new(ConsoleApprovalHandler);
    let approval_handler = Some(console_handler.clone() as _);
    let question_handler = Some(console_handler as _);

    if let Some(ref session_id) = session_id {
        let session_path = session_file_path(session_id);
        if session_path.exists() {
            let stored = StoredSession::load(session_id)?;
            let options = AgentOptions {
                model: model.unwrap_or_else(|| stored.snapshot.model.clone()),
                max_turns,
                ..AgentOptions::default()
            };
            return Ok(SessionBootstrap {
                session: AgentSession::restore_with_telemetry(
                    gateway,
                    default_tool_registry(),
                    options,
                    permission_mode,
                    approval_handler,
                    question_handler,
                    stored,
                    telemetry_sink,
                ),
                repl_metadata: ReplMetadata {
                    config: current,
                    config_path,
                    memory_root: memory_root(),
                    plugins_root: plugins_root(),
                    sessions_root: sessions_root(),
                    shares_root: shares_root(),
                },
            });
        }
    }

    let working_directory = match cwd {
        Some(path) => path,
        None => env::current_dir()?,
    };
    let options = AgentOptions {
        output_style: hellox_style::resolve_configured_output_style(&current, &working_directory)?,
        persona: hellox_style::resolve_configured_persona(&current, &working_directory)?,
        prompt_fragments: hellox_style::resolve_configured_fragments(&current, &working_directory)?,
        model: model.unwrap_or_else(|| current.session.model.clone()),
        max_turns,
        ..AgentOptions::default()
    };

    Ok(SessionBootstrap {
        session: AgentSession::create_with_telemetry(
            gateway,
            default_tool_registry(),
            config_path.clone(),
            working_directory,
            &shell_name,
            options,
            permission_mode,
            approval_handler,
            question_handler,
            persist,
            session_id,
            telemetry_sink,
        ),
        repl_metadata: ReplMetadata {
            config: current,
            config_path,
            memory_root: memory_root(),
            plugins_root: plugins_root(),
            sessions_root: sessions_root(),
            shares_root: shares_root(),
        },
    })
}

fn current_shell_name() -> String {
    env::var("SHELL")
        .ok()
        .or_else(|| env::var("COMSPEC").ok())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "powershell".to_string()
            } else {
                "sh".to_string()
            }
        })
}

pub(crate) struct SessionBootstrap {
    pub(crate) session: AgentSession,
    pub(crate) repl_metadata: ReplMetadata,
}
