mod assistant_panel;
mod auth_commands;
mod auto_compact;
mod auto_memory;
mod bridge_commands;
mod bridge_panel;
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
mod remote_panel;
mod repl;
mod search;
mod server_commands;
mod session_panel;
mod sessions;
mod settings_commands;
mod skills;
mod startup;
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
mod welcome_v2;
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
mod workflow_test_support;
#[cfg(test)]
mod workflows_tests;

use std::env;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
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
use crate::sessions::{format_session_list, list_sessions, load_session};
use crate::settings_commands::{handle_model_command, handle_permissions_command};
use crate::startup::{
    ensure_workspace_trusted, format_prompt_submission_error, prepare_interactive_session_launch,
    prepare_noninteractive_session_launch, resolve_app_language, AppLanguage, LaunchPreparation,
};
use crate::sync_commands::handle_sync_command;
use crate::task_commands::handle_tasks_command;
use crate::ui_commands::{handle_brief_command, handle_tools_command};
use crate::usage::print_usage;
use crate::worker_runner::run_worker_job;
use crate::workflow_commands::handle_workflow_command;

fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = std::thread::Builder::new()
        .name("hellox-cli-parse".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(Cli::parse)
        .map_err(|error| anyhow!("failed to spawn cli parser thread: {error}"))?
        .join()
        .map_err(|_| anyhow!("cli parser thread panicked"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async_main(cli))
}

async fn async_main(cli: Cli) -> Result<()> {
    let Cli {
        prompt,
        print,
        continue_last,
        resume,
        model,
        gateway_url,
        config,
        cwd,
        max_turns,
        command,
    } = cli;

    match command {
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
            run_single_prompt_session(
                prompt,
                model,
                gateway_url,
                config,
                cwd,
                session_id,
                max_turns,
            )
            .await?
        }
        Some(Commands::Repl {
            model,
            gateway_url,
            config,
            cwd,
            session_id,
            max_turns,
        }) => {
            run_interactive_session(model, gateway_url, config, cwd, session_id, max_turns, None)
                .await?
        }
        Some(Commands::WorkerRunAgent { job }) => run_worker_job(job).await?,
        None => {
            run_root_command(
                prompt,
                print,
                continue_last,
                resume,
                model,
                gateway_url,
                config,
                cwd,
                max_turns,
            )
            .await?
        }
    }

    Ok(())
}

async fn run_root_command(
    prompt: Option<String>,
    print: bool,
    continue_last: bool,
    resume: Option<Option<String>>,
    model: Option<String>,
    gateway_url: Option<String>,
    config: Option<PathBuf>,
    cwd: Option<PathBuf>,
    max_turns: usize,
) -> Result<()> {
    let stdin_is_terminal = io::stdin().is_terminal();
    let stdout_is_terminal = io::stdout().is_terminal();
    let app_language = resolve_cli_language(config.as_ref());
    let prompt = resolve_root_prompt(prompt, stdin_is_terminal)?;
    let session_id =
        match resolve_root_session_id(continue_last, resume, cwd.as_ref(), app_language)? {
            RootSessionSelection::Use(session_id) => session_id,
            RootSessionSelection::PrintListing(listing) => {
                println!("{listing}");
                return Ok(());
            }
        };

    if should_run_root_interactive(print, stdin_is_terminal, stdout_is_terminal) {
        run_interactive_session(
            model,
            gateway_url,
            config,
            cwd,
            session_id,
            max_turns,
            prompt,
        )
        .await?;
        return Ok(());
    }

    if let Some(prompt) = prompt {
        return run_single_prompt_session(
            prompt,
            model,
            gateway_url,
            config,
            cwd,
            session_id,
            max_turns,
        )
        .await;
    }

    if print || session_id.is_some() {
        return Err(anyhow!(noninteractive_prompt_required_text(app_language)));
    }

    print_usage(app_language);
    Ok(())
}

async fn run_interactive_session(
    model: Option<String>,
    gateway_url: Option<String>,
    config: Option<PathBuf>,
    cwd: Option<PathBuf>,
    session_id: Option<String>,
    max_turns: usize,
    initial_prompt: Option<String>,
) -> Result<()> {
    let config_arg = config.clone();
    let gateway_url_arg = gateway_url.clone();
    let cwd_arg = cwd.clone();
    let mut active_session_id = session_id.clone();
    let mut active_model_override = model.clone();
    let mut pending_prompt = initial_prompt;

    loop {
        match prepare_interactive_session_launch(
            config_arg.clone(),
            active_session_id.as_deref(),
            active_model_override.as_deref(),
            gateway_url_arg.as_deref(),
        )? {
            LaunchPreparation::Continue { model_override } => {
                if let Some(model_override) = model_override {
                    active_model_override = Some(model_override);
                }
            }
            LaunchPreparation::Exit => break,
        }
        let mut bootstrap = build_session(
            config_arg.clone(),
            gateway_url_arg.clone(),
            active_model_override.clone(),
            cwd_arg.clone(),
            active_session_id.clone(),
            max_turns,
        )?;
        let workspace_trusted = ensure_workspace_trusted(
            &bootstrap.repl_metadata.config_path,
            resolve_app_language(&bootstrap.repl_metadata.config),
            bootstrap.session.working_directory(),
        )?;
        if !workspace_trusted {
            break;
        }
        if let Some(prompt) = pending_prompt.take() {
            run_prompt_with_session(
                prompt,
                &mut bootstrap.session,
                &bootstrap.repl_metadata.memory_root,
                &bootstrap.repl_metadata.config,
                &bootstrap.repl_metadata.config_path,
            )
            .await?;
        }
        match run_repl(
            &mut bootstrap.session,
            &bootstrap.repl_metadata,
            workspace_trusted,
        )
        .await?
        {
            ReplExit::Exit => break,
            ReplExit::Resume(session_id) => {
                active_session_id = Some(session_id);
                active_model_override = None;
            }
        }
    }

    Ok(())
}

pub(crate) fn should_launch_default_repl(
    stdin_is_terminal: bool,
    stdout_is_terminal: bool,
) -> bool {
    stdin_is_terminal && stdout_is_terminal
}

pub(crate) fn should_run_root_interactive(
    print: bool,
    stdin_is_terminal: bool,
    stdout_is_terminal: bool,
) -> bool {
    !print && should_launch_default_repl(stdin_is_terminal, stdout_is_terminal)
}

async fn run_single_prompt_session(
    prompt: String,
    model: Option<String>,
    gateway_url: Option<String>,
    config: Option<PathBuf>,
    cwd: Option<PathBuf>,
    session_id: Option<String>,
    max_turns: usize,
) -> Result<()> {
    prepare_noninteractive_session_launch(
        config.clone(),
        session_id.as_deref(),
        model.as_deref(),
        gateway_url.as_deref(),
    )?;
    let mut bootstrap = build_session(config, gateway_url, model, cwd, session_id, max_turns)?;
    run_prompt_with_session(
        prompt,
        &mut bootstrap.session,
        &bootstrap.repl_metadata.memory_root,
        &bootstrap.repl_metadata.config,
        &bootstrap.repl_metadata.config_path,
    )
    .await
}

async fn run_prompt_with_session(
    prompt: String,
    session: &mut AgentSession,
    memory_root: &Path,
    config: &hellox_config::HelloxConfig,
    config_path: &Path,
) -> Result<()> {
    let result = session.run_user_prompt(prompt).await.map_err(|error| {
        anyhow!(
            "{}",
            format_prompt_submission_error(
                resolve_app_language(config),
                &error,
                config,
                session.model(),
                Some(config_path),
            )
        )
    })?;
    println!("{}", result.final_text);
    match maybe_auto_compact_session(session, memory_root)? {
        Some(outcome) => println!("{}", format_auto_compact_notice(&outcome)),
        None => match maybe_auto_refresh_session_memory(session, memory_root)? {
            Some(outcome) => println!("{}", format_auto_memory_refresh_notice(&outcome)),
            None => {}
        },
    }
    Ok(())
}

fn resolve_root_prompt(prompt: Option<String>, stdin_is_terminal: bool) -> Result<Option<String>> {
    if prompt.is_some() || stdin_is_terminal {
        return Ok(prompt);
    }

    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn resolve_root_session_id(
    continue_last: bool,
    resume: Option<Option<String>>,
    cwd: Option<&PathBuf>,
    language: AppLanguage,
) -> Result<RootSessionSelection> {
    if continue_last {
        let working_directory = resolve_root_working_directory(cwd)?;
        let session_id = find_latest_session_for_working_directory(&working_directory)?;
        return match session_id {
            Some(session_id) => Ok(RootSessionSelection::Use(Some(session_id))),
            None => Err(anyhow!(no_persisted_session_for_directory_text(
                language,
                &normalize_working_directory(&working_directory)
            ))),
        };
    }

    match resume {
        None => Ok(RootSessionSelection::Use(None)),
        Some(None) => Ok(RootSessionSelection::PrintListing(root_resume_help_text(
            language,
        )?)),
        Some(Some(session_id)) => {
            load_session(&sessions_root(), &session_id)?;
            Ok(RootSessionSelection::Use(Some(session_id)))
        }
    }
}

fn resolve_root_working_directory(cwd: Option<&PathBuf>) -> Result<PathBuf> {
    match cwd {
        Some(path) if path.is_absolute() => Ok(path.clone()),
        Some(path) => Ok(env::current_dir()?.join(path)),
        None => Ok(env::current_dir()?),
    }
}

fn find_latest_session_for_working_directory(working_directory: &Path) -> Result<Option<String>> {
    let expected = normalize_working_directory(working_directory);
    Ok(list_sessions(&sessions_root())?
        .into_iter()
        .find(|session| session.working_directory == expected)
        .map(|session| session.session_id))
}

fn normalize_working_directory(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn root_resume_help_text(language: AppLanguage) -> Result<String> {
    let sessions = list_sessions(&sessions_root())?;
    if sessions.is_empty() {
        return Ok(no_persisted_sessions_text(language).to_string());
    }
    Ok(format!(
        "{}\n\n{}",
        root_resume_usage_text(language),
        format_session_list(&sessions)
    ))
}

fn resolve_cli_language(config_path: Option<&PathBuf>) -> AppLanguage {
    let path = config_path.cloned().unwrap_or_else(default_config_path);
    let config =
        load_or_default(Some(path)).unwrap_or_else(|_| hellox_config::HelloxConfig::default());
    resolve_app_language(&config)
}

fn noninteractive_prompt_required_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => {
            "A prompt argument or piped stdin input is required for non-interactive root mode."
        }
        AppLanguage::SimplifiedChinese => {
            "非交互根命令模式需要提供 prompt 参数，或通过 stdin 管道输入内容。"
        }
    }
}

fn no_persisted_sessions_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => {
            "No persisted sessions found. Start a session with persistence enabled first."
        }
        AppLanguage::SimplifiedChinese => "未找到已持久化会话。请先启动一个启用了持久化的会话。",
    }
}

fn root_resume_usage_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Use `hellox --resume <session-id>` to switch sessions.",
        AppLanguage::SimplifiedChinese => "使用 `hellox --resume <session-id>` 可切换到指定会话。",
    }
}

fn no_persisted_session_for_directory_text(
    language: AppLanguage,
    working_directory: &str,
) -> String {
    match language {
        AppLanguage::English => format!(
            "No persisted session found for `{working_directory}`. Use `hellox session list` to inspect available sessions."
        ),
        AppLanguage::SimplifiedChinese => format!(
            "目录 `{working_directory}` 还没有已持久化会话。请先使用 `hellox session list` 查看可用会话。"
        ),
    }
}

pub(crate) enum RootSessionSelection {
    Use(Option<String>),
    PrintListing(String),
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
                app_language: Some(resolve_app_language(&current).locale_tag().to_string()),
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
        app_language: Some(resolve_app_language(&current).locale_tag().to_string()),
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
