use anyhow::Result;
use hellox_auth::load_auth_store;
use hellox_bridge::{list_bridge_sessions, load_bridge_session, BridgeRuntimePaths};
use hellox_config::{load_or_default, save_config};
use hellox_remote::{
    add_remote_environment, build_remote_environment, build_teleport_plan,
    create_remote_direct_connect, format_remote_environment_detail, format_remote_environment_list,
    format_teleport_plan, list_remote_environments, list_remote_sessions, load_remote_session,
    remove_remote_environment, set_remote_environment_enabled, TeleportOverrides,
};
use hellox_server::{format_direct_connect_config, DirectConnectRequest};

use crate::assistant_panel::{
    render_local_assistant_detail_panel, render_local_assistant_list_panel,
    render_remote_assistant_detail_panel, render_remote_assistant_list_panel,
};
use crate::remote_panel::{render_remote_env_panel, render_teleport_plan_panel};
use crate::sessions::load_session;

use super::commands::{AssistantCommand, RemoteEnvCommand, TeleportCommand};
use super::ReplMetadata;

pub(super) fn handle_remote_env_command(
    command: RemoteEnvCommand,
    metadata: &ReplMetadata,
) -> Result<String> {
    let mut config = load_or_default(Some(metadata.config_path.clone()))?;

    match command {
        RemoteEnvCommand::Help => Ok(remote_env_help_text()),
        RemoteEnvCommand::Panel { environment_name } => {
            render_remote_env_panel(&metadata.config_path, &config, environment_name.as_deref())
        }
        RemoteEnvCommand::List => Ok(format_remote_environment_list(&list_remote_environments(
            &config,
        ))),
        RemoteEnvCommand::Show {
            environment_name: None,
        } => Ok("Usage: /remote-env show <name>".to_string()),
        RemoteEnvCommand::Show {
            environment_name: Some(environment_name),
        } => {
            let environment = list_remote_environments(&config)
                .into_iter()
                .find(|environment| environment.name == environment_name)
                .ok_or_else(|| {
                    anyhow::anyhow!("Remote environment `{environment_name}` was not found")
                })?;
            Ok(format_remote_environment_detail(&environment))
        }
        RemoteEnvCommand::Add {
            environment_name: None,
            ..
        } => Ok(
            "Usage: /remote-env add <name> <url> [token-env] [account-id] [device-id]".to_string(),
        ),
        RemoteEnvCommand::Add { url: None, .. } => Ok(
            "Usage: /remote-env add <name> <url> [token-env] [account-id] [device-id]".to_string(),
        ),
        RemoteEnvCommand::Add {
            environment_name: Some(environment_name),
            url: Some(url),
            token_env,
            account_id,
            device_id,
        } => {
            add_remote_environment(
                &mut config,
                environment_name.clone(),
                build_remote_environment(url, token_env, account_id, device_id, None),
            )?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Added remote environment `{environment_name}`."))
        }
        RemoteEnvCommand::Enable {
            environment_name: None,
        } => Ok("Usage: /remote-env enable <name>".to_string()),
        RemoteEnvCommand::Enable {
            environment_name: Some(environment_name),
        } => {
            set_remote_environment_enabled(&mut config, &environment_name, true)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Enabled remote environment `{environment_name}`."))
        }
        RemoteEnvCommand::Disable {
            environment_name: None,
        } => Ok("Usage: /remote-env disable <name>".to_string()),
        RemoteEnvCommand::Disable {
            environment_name: Some(environment_name),
        } => {
            set_remote_environment_enabled(&mut config, &environment_name, false)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Disabled remote environment `{environment_name}`."))
        }
        RemoteEnvCommand::Remove {
            environment_name: None,
        } => Ok("Usage: /remote-env remove <name>".to_string()),
        RemoteEnvCommand::Remove {
            environment_name: Some(environment_name),
        } => {
            remove_remote_environment(&mut config, &environment_name)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Removed remote environment `{environment_name}`."))
        }
    }
}

pub(super) fn handle_teleport_command(
    command: TeleportCommand,
    session: &hellox_agent::AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    let config = load_or_default(Some(metadata.config_path.clone()))?;

    match command {
        TeleportCommand::Help => Ok(teleport_help_text()),
        TeleportCommand::Panel {
            environment_name: None,
            ..
        } => Ok("Usage: /teleport panel <environment-name> [session-id]".to_string()),
        TeleportCommand::Panel {
            environment_name: Some(environment_name),
            session_id,
        } => {
            let stored = match session_id.as_deref() {
                Some(session_id) => Some(load_session(&metadata.sessions_root, session_id)?),
                None => None,
            };
            let plan = build_teleport_plan(
                &config,
                &environment_name,
                stored.as_ref(),
                TeleportOverrides {
                    session_id: session_id
                        .or_else(|| session.session_id().map(ToString::to_string)),
                    model: Some(session.model().to_string()),
                    working_directory: Some(
                        session
                            .working_directory()
                            .display()
                            .to_string()
                            .replace('\\', "/"),
                    ),
                },
            )?;
            Ok(render_teleport_plan_panel(&plan))
        }
        TeleportCommand::Plan {
            environment_name: None,
            ..
        } => Ok("Usage: /teleport plan <environment-name> [session-id]".to_string()),
        TeleportCommand::Plan {
            environment_name: Some(environment_name),
            session_id,
        } => {
            let stored = match session_id.as_deref() {
                Some(session_id) => Some(load_session(&metadata.sessions_root, session_id)?),
                None => None,
            };
            let plan = build_teleport_plan(
                &config,
                &environment_name,
                stored.as_ref(),
                TeleportOverrides {
                    session_id: session_id
                        .or_else(|| session.session_id().map(ToString::to_string)),
                    model: Some(session.model().to_string()),
                    working_directory: Some(
                        session
                            .working_directory()
                            .display()
                            .to_string()
                            .replace('\\', "/"),
                    ),
                },
            )?;
            Ok(format_teleport_plan(&plan))
        }
        TeleportCommand::Connect {
            environment_name: None,
            ..
        } => Ok("Usage: /teleport connect <environment-name> [session-id]".to_string()),
        TeleportCommand::Connect {
            environment_name: Some(environment_name),
            session_id,
        } => {
            let stored = match session_id.as_deref() {
                Some(session_id) => Some(load_session(&metadata.sessions_root, session_id)?),
                None => None,
            };
            let auth_store = load_auth_store(None, None)?;
            let direct = create_remote_direct_connect(
                &config,
                &auth_store,
                &environment_name,
                DirectConnectRequest {
                    session_id: session_id
                        .or_else(|| session.session_id().map(ToString::to_string)),
                    model: Some(session.model().to_string()),
                    working_directory: Some(
                        stored
                            .as_ref()
                            .map(|item| item.working_directory.replace('\\', "/"))
                            .unwrap_or_else(|| {
                                session
                                    .working_directory()
                                    .display()
                                    .to_string()
                                    .replace('\\', "/")
                            }),
                    ),
                    base_url: None,
                },
            )?;
            Ok(format_direct_connect_config(&direct))
        }
    }
}

pub(super) fn handle_assistant_command(
    command: AssistantCommand,
    metadata: &ReplMetadata,
) -> Result<String> {
    let paths = BridgeRuntimePaths::new(
        metadata.config_path.clone(),
        metadata.sessions_root.clone(),
        metadata.plugins_root.clone(),
    );

    match command {
        AssistantCommand::Help => Ok(assistant_help_text()),
        AssistantCommand::List {
            environment_name: None,
        } => Ok(render_local_assistant_list_panel(
            &paths.sessions_root,
            &list_bridge_sessions(&paths)?,
        )),
        AssistantCommand::List {
            environment_name: Some(environment_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let auth_store = load_auth_store(None, None)?;
            Ok(render_remote_assistant_list_panel(
                &environment_name,
                &list_remote_sessions(&config, &auth_store, &environment_name)?,
            ))
        }
        AssistantCommand::Show {
            session_id: None, ..
        } => Ok("Usage: /assistant show <session-id>".to_string()),
        AssistantCommand::Show {
            session_id: Some(session_id),
            environment_name: None,
        } => Ok(render_local_assistant_detail_panel(
            &paths.sessions_root,
            &load_bridge_session(&paths, &session_id)?,
        )),
        AssistantCommand::Show {
            session_id: Some(session_id),
            environment_name: Some(environment_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let auth_store = load_auth_store(None, None)?;
            Ok(render_remote_assistant_detail_panel(
                &environment_name,
                &load_remote_session(&config, &auth_store, &environment_name, &session_id)?,
            ))
        }
    }
}

fn remote_env_help_text() -> String {
    [
        "Usage (user-managed remote capability):",
        "  /remote-env",
        "  /remote-env panel [name]",
        "  /remote-env show <name>",
        "  /remote-env add <name> <url> [token-env] [account-id] [device-id]",
        "  /remote-env enable <name>",
        "  /remote-env disable <name>",
        "  /remote-env remove <name>",
    ]
    .join("\n")
}

fn teleport_help_text() -> String {
    [
        "Usage (user-managed remote capability):",
        "  /teleport panel <environment-name> [session-id]",
        "  /teleport plan <environment-name> [session-id]",
        "  /teleport connect <environment-name> [session-id]",
    ]
    .join("\n")
}

fn assistant_help_text() -> String {
    [
        "Usage (user-managed remote capability):",
        "  /assistant                     Open the local assistant viewer panel",
        "  /assistant list [environment-name] Show the local/remote assistant session viewer",
        "  /assistant show <session-id> [environment-name] Inspect one assistant-viewable session",
    ]
    .join("\n")
}
