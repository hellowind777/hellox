use anyhow::Result;
use hellox_auth::LocalAuthStoreBackend;
use hellox_bridge::{list_bridge_sessions, load_bridge_session, BridgeRuntimePaths};
use hellox_config::{
    default_config_path, load_or_default, plugins_root, save_config, sessions_root,
};
use hellox_remote::{
    add_remote_environment, build_remote_environment, build_teleport_plan,
    format_remote_environment_detail, format_remote_environment_list, format_teleport_plan,
    list_remote_environments, remote_session_transport, remove_remote_environment,
    set_remote_environment_enabled, HttpRemoteSessionTransport, RemoteSessionTransport,
    TeleportOverrides,
};
use hellox_server::{format_direct_connect_config, DirectConnectRequest};

use crate::assistant_panel::{
    render_local_assistant_detail_panel, render_local_assistant_list_panel,
    render_remote_assistant_detail_panel, render_remote_assistant_list_panel,
};
use crate::cli_types::{AssistantCommands, RemoteEnvCommands, TeleportCommands};
use crate::sessions::load_session;

pub fn handle_remote_env_command(command: RemoteEnvCommands) -> Result<()> {
    let config_path = default_config_path();
    let mut config = load_or_default(Some(config_path.clone()))?;

    match command {
        RemoteEnvCommands::List => {
            println!(
                "{}",
                format_remote_environment_list(&list_remote_environments(&config))
            );
        }
        RemoteEnvCommands::Show { environment_name } => {
            let environments = list_remote_environments(&config);
            let environment = environments
                .into_iter()
                .find(|environment| environment.name == environment_name)
                .ok_or_else(|| {
                    anyhow::anyhow!("Remote environment `{environment_name}` was not found")
                })?;
            println!("{}", format_remote_environment_detail(&environment));
        }
        RemoteEnvCommands::Add {
            environment_name,
            url,
            token_env,
            account_id,
            device_id,
            description,
        } => {
            add_remote_environment(
                &mut config,
                environment_name.clone(),
                build_remote_environment(url, token_env, account_id, device_id, description),
            )?;
            save_config(Some(config_path.clone()), &config)?;
            println!("Added remote environment `{environment_name}`.");
        }
        RemoteEnvCommands::Enable { environment_name } => {
            set_remote_environment_enabled(&mut config, &environment_name, true)?;
            save_config(Some(config_path.clone()), &config)?;
            println!("Enabled remote environment `{environment_name}`.");
        }
        RemoteEnvCommands::Disable { environment_name } => {
            set_remote_environment_enabled(&mut config, &environment_name, false)?;
            save_config(Some(config_path.clone()), &config)?;
            println!("Disabled remote environment `{environment_name}`.");
        }
        RemoteEnvCommands::Remove { environment_name } => {
            remove_remote_environment(&mut config, &environment_name)?;
            save_config(Some(config_path.clone()), &config)?;
            println!("Removed remote environment `{environment_name}`.");
        }
    }

    Ok(())
}

pub fn handle_teleport_command(command: TeleportCommands) -> Result<()> {
    let config = load_or_default(Some(default_config_path()))?;

    match command {
        TeleportCommands::Plan {
            environment_name,
            session_id,
            model,
            cwd,
        } => {
            let stored = match session_id.as_deref() {
                Some(session_id) => Some(load_session(&sessions_root(), session_id)?),
                None => None,
            };
            let plan = build_teleport_plan(
                &config,
                &environment_name,
                stored.as_ref(),
                TeleportOverrides {
                    session_id,
                    model,
                    working_directory: cwd
                        .map(|path| path.display().to_string().replace('\\', "/")),
                },
            )?;
            println!("{}", format_teleport_plan(&plan));
        }
        TeleportCommands::Connect {
            environment_name,
            session_id,
            model,
            cwd,
        } => {
            let stored = match session_id.as_deref() {
                Some(session_id) => Some(load_session(&sessions_root(), session_id)?),
                None => None,
            };
            let request = DirectConnectRequest {
                session_id: session_id
                    .or_else(|| stored.as_ref().map(|session| session.session_id.clone())),
                model: model.or_else(|| stored.as_ref().map(|session| session.model.clone())),
                working_directory: cwd
                    .map(|path| path.display().to_string().replace('\\', "/"))
                    .or_else(|| {
                        stored
                            .as_ref()
                            .map(|session| session.working_directory.replace('\\', "/"))
                    }),
                base_url: None,
            };
            let transport = build_remote_transport(&config, &environment_name)?;
            let direct = transport.create_direct_connect_session(request)?;
            println!("{}", format_direct_connect_config(&direct));
        }
    }

    Ok(())
}

pub fn handle_assistant_command(command: AssistantCommands) -> Result<()> {
    let paths = BridgeRuntimePaths::new(default_config_path(), sessions_root(), plugins_root());
    let config = load_or_default(Some(default_config_path()))?;

    match command {
        AssistantCommands::List { environment_name } => {
            if let Some(environment_name) = environment_name {
                let transport = build_remote_transport(&config, &environment_name)?;
                println!(
                    "{}",
                    render_remote_assistant_list_panel(
                        &environment_name,
                        &transport.list_sessions()?
                    )
                );
            } else {
                println!(
                    "{}",
                    render_local_assistant_list_panel(
                        &paths.sessions_root,
                        &list_bridge_sessions(&paths)?,
                    )
                );
            }
        }
        AssistantCommands::Show {
            session_id,
            environment_name,
        } => {
            if let Some(environment_name) = environment_name {
                let transport = build_remote_transport(&config, &environment_name)?;
                println!(
                    "{}",
                    render_remote_assistant_detail_panel(
                        &environment_name,
                        &transport.load_session_detail(&session_id)?,
                    )
                );
            } else {
                println!(
                    "{}",
                    render_local_assistant_detail_panel(
                        &paths.sessions_root,
                        &load_bridge_session(&paths, &session_id)?,
                    )
                );
            }
        }
    }

    Ok(())
}

fn build_remote_transport(
    config: &hellox_config::HelloxConfig,
    environment_name: &str,
) -> Result<HttpRemoteSessionTransport> {
    let auth_store = LocalAuthStoreBackend::default().load_auth_store()?;
    remote_session_transport(config, &auth_store, environment_name)
}
