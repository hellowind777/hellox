use std::fs;

use anyhow::{anyhow, Context, Result};
use hellox_server::{
    format_direct_connect_config, format_server_session_detail, format_server_session_list,
    format_server_status, DirectConnectRequest, LocalServerControlPlane,
};
use hellox_sync::{
    format_managed_settings_document, format_policy_limits_document, format_settings_snapshot,
    format_team_memory_snapshot,
};

use crate::cli_types::ServerCommands;
use crate::team_memory_panel::render_server_team_memory_panel;

pub async fn handle_server_command(command: ServerCommands) -> Result<()> {
    match command {
        ServerCommands::Serve { config } => LocalServerControlPlane::new(config).serve().await?,
        ServerCommands::Status { config } => {
            let control = LocalServerControlPlane::new(config);
            println!("{}", format_server_status(&control.inspect_status()?));
        }
        ServerCommands::CreateSession {
            config,
            base_url,
            session_id,
            model,
            cwd,
        } => {
            let control = LocalServerControlPlane::new(config);
            let direct = control.create_direct_connect(DirectConnectRequest {
                session_id,
                model,
                working_directory: cwd.map(|path| path.display().to_string().replace('\\', "/")),
                base_url,
            })?;
            println!("{}", format_direct_connect_config(&direct));
        }
        ServerCommands::Sessions { config } => {
            let control = LocalServerControlPlane::new(config);
            println!(
                "{}",
                format_server_session_list(&control.inspect_registered_sessions()?)
            );
        }
        ServerCommands::ShowSession { session_id, config } => {
            let control = LocalServerControlPlane::new(config);
            println!(
                "{}",
                format_server_session_detail(&control.inspect_registered_session(&session_id)?)
            );
        }
        ServerCommands::ManagedSettingsShow { config } => {
            let control = LocalServerControlPlane::new(config);
            let document = control
                .inspect_managed_settings()?
                .ok_or_else(|| anyhow!("Managed settings document was not found"))?;
            println!("{}", format_managed_settings_document(&document));
        }
        ServerCommands::ManagedSettingsSet {
            config_toml_file,
            config,
            signature,
        } => {
            let control = LocalServerControlPlane::new(config);
            let config_toml = fs::read_to_string(&config_toml_file).with_context(|| {
                format!(
                    "failed to read managed settings source {}",
                    config_toml_file.display()
                )
            })?;
            println!(
                "{}",
                format_managed_settings_document(
                    &control.set_managed_settings(config_toml, signature)?
                )
            );
        }
        ServerCommands::PolicyLimitsShow { config } => {
            let control = LocalServerControlPlane::new(config);
            let document = control
                .inspect_policy_limits()?
                .ok_or_else(|| anyhow!("Policy limits document was not found"))?;
            println!("{}", format_policy_limits_document(&document));
        }
        ServerCommands::PolicyLimitsSet {
            config,
            disabled_commands,
            disabled_tools,
            notes,
        } => {
            let control = LocalServerControlPlane::new(config);
            println!(
                "{}",
                format_policy_limits_document(&control.set_policy_limits(
                    disabled_commands,
                    disabled_tools,
                    notes,
                )?)
            );
        }
        ServerCommands::SettingsShow { account_id, config } => {
            let control = LocalServerControlPlane::new(config);
            let snapshot = control
                .inspect_synced_settings(&account_id)?
                .ok_or_else(|| anyhow!("Settings snapshot for `{account_id}` was not found"))?;
            println!("{}", format_settings_snapshot(&snapshot));
        }
        ServerCommands::TeamMemoryShow {
            account_id,
            repo_id,
            config,
        } => {
            let control = LocalServerControlPlane::new(config);
            let snapshot = control
                .inspect_synced_team_memory(&account_id, &repo_id)?
                .ok_or_else(|| {
                    anyhow!("Team memory snapshot for `{account_id}` / `{repo_id}` was not found")
                })?;
            println!("{}", format_team_memory_snapshot(&snapshot));
        }
        ServerCommands::TeamMemoryPanel {
            account_id,
            repo_id,
            config,
        } => {
            let control = LocalServerControlPlane::new(config);
            let snapshot = control
                .inspect_synced_team_memory(&account_id, &repo_id)?
                .ok_or_else(|| {
                    anyhow!("Team memory snapshot for `{account_id}` / `{repo_id}` was not found")
                })?;
            println!(
                "{}",
                render_server_team_memory_panel(&account_id, &snapshot)
            );
        }
    }

    Ok(())
}
