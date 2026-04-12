use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use hellox_auth::LocalAuthStoreBackend;
use hellox_config::load_or_default;
use hellox_remote::remote_environment_access;
use hellox_sync::{
    format_managed_settings_document, format_policy_limits_document, format_team_memory_snapshot,
    CachedManagedSettingsSource, CachedPolicyLimitsSource, LocalSyncStore, ManagedSettingsSource,
    PolicyLimitsSource, RemoteFetch, RemoteSyncClient, RemoteSyncTransport, SettingsSyncSnapshot,
    TeamMemorySnapshot,
};

use crate::cli_types::SyncCommands;
use crate::team_memory_panel::render_local_team_memory_panel;

pub fn handle_sync_command(command: SyncCommands) -> Result<()> {
    let local_sync = LocalSyncStore::default();
    match command {
        SyncCommands::SettingsExport { output, config } => {
            let snapshot = local_sync.export_settings_snapshot(config)?;
            write_json(&output, &snapshot)?;
            println!(
                "Exported settings snapshot to `{}`.",
                output.display().to_string().replace('\\', "/")
            );
        }
        SyncCommands::SettingsImport { input, config } => {
            let snapshot = read_json::<SettingsSyncSnapshot>(&input)?;
            let path = local_sync.import_settings_snapshot(config, &snapshot)?;
            println!(
                "Imported settings snapshot into `{}`.",
                path.display().to_string().replace('\\', "/")
            );
        }
        SyncCommands::SettingsPush {
            environment_name,
            config,
        } => {
            let snapshot = local_sync.export_settings_snapshot(config)?;
            let client = build_remote_sync_client(&environment_name)?;
            client.push_settings(&snapshot)?;
            println!("Uploaded settings snapshot to `{environment_name}`.");
        }
        SyncCommands::SettingsPull {
            environment_name,
            config,
            output,
        } => {
            let client = build_remote_sync_client(&environment_name)?;
            let snapshot = client.pull_settings()?.ok_or_else(|| {
                anyhow::anyhow!("Remote environment `{environment_name}` has no settings snapshot")
            })?;
            if let Some(output) = output {
                write_json(&output, &snapshot)?;
                println!(
                    "Downloaded settings snapshot into `{}`.",
                    output.display().to_string().replace('\\', "/")
                );
            } else {
                let path = local_sync.import_settings_snapshot(config, &snapshot)?;
                println!(
                    "Downloaded settings snapshot into `{}`.",
                    path.display().to_string().replace('\\', "/")
                );
            }
        }
        SyncCommands::TeamMemoryShow { repo_id } => {
            println!(
                "{}",
                format_team_memory_snapshot(&local_sync.load_team_memory_snapshot(&repo_id)?)
            );
        }
        SyncCommands::TeamMemoryPanel { repo_id } => {
            println!(
                "{}",
                render_local_team_memory_panel(&local_sync.load_team_memory_snapshot(&repo_id)?)
            );
        }
        SyncCommands::TeamMemoryExport { repo_id, output } => {
            write_json(&output, &local_sync.load_team_memory_snapshot(&repo_id)?)?;
            println!(
                "Exported team memory snapshot to `{}`.",
                output.display().to_string().replace('\\', "/")
            );
        }
        SyncCommands::TeamMemoryImport { repo_id, input } => {
            let snapshot = read_json::<TeamMemorySnapshot>(&input)?;
            println!(
                "{}",
                format_team_memory_snapshot(
                    &local_sync.merge_team_memory_snapshot(&repo_id, snapshot)?
                )
            );
        }
        SyncCommands::TeamMemoryPut {
            repo_id,
            key,
            content,
        } => {
            println!(
                "{}",
                format_team_memory_snapshot(
                    &local_sync.put_team_memory_entry(&repo_id, key, content)?
                )
            );
        }
        SyncCommands::TeamMemoryRemove { repo_id, key } => {
            println!(
                "{}",
                format_team_memory_snapshot(&local_sync.remove_team_memory_entry(&repo_id, &key)?)
            );
        }
        SyncCommands::TeamMemorySync {
            environment_name,
            repo_id,
        } => {
            let client = build_remote_sync_client(&environment_name)?;
            let snapshot = client
                .sync_team_memory(&repo_id, &local_sync.load_team_memory_snapshot(&repo_id)?)?;
            println!("{}", format_team_memory_snapshot(&snapshot));
        }
        SyncCommands::ManagedSettingsFetch { environment_name } => {
            let client = build_remote_sync_client(&environment_name)?;
            let cache = CachedManagedSettingsSource::new(environment_name.clone());
            let cached = cache.inspect()?;
            match client.fetch_managed_settings_document(
                cached.as_ref().and_then(|item| item.etag.as_deref()),
            ) {
                Ok(RemoteFetch::Updated(document)) => {
                    cache.persist(&document)?;
                    println!("{}", format_managed_settings_document(&document.value));
                }
                Ok(RemoteFetch::NotModified { .. }) => {
                    let document = cache.load_managed_settings()?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "Managed settings cache is missing for `{environment_name}`"
                        )
                    })?;
                    println!("{}", format_managed_settings_document(&document));
                }
                Ok(RemoteFetch::Missing) => {
                    println!("No managed settings document is available for `{environment_name}`.");
                }
                Err(error) => {
                    if let Some(document) = cache.load_managed_settings()? {
                        println!("{}", format_managed_settings_document(&document));
                    } else {
                        return Err(error);
                    }
                }
            }
        }
        SyncCommands::PolicyLimitsFetch { environment_name } => {
            let client = build_remote_sync_client(&environment_name)?;
            let cache = CachedPolicyLimitsSource::new(environment_name.clone());
            let cached = cache.inspect()?;
            match client
                .fetch_policy_limits_document(cached.as_ref().and_then(|item| item.etag.as_deref()))
            {
                Ok(RemoteFetch::Updated(document)) => {
                    cache.persist(&document)?;
                    println!("{}", format_policy_limits_document(&document.value));
                }
                Ok(RemoteFetch::NotModified { .. }) => {
                    let document = cache.load_policy_limits()?.ok_or_else(|| {
                        anyhow::anyhow!("Policy limits cache is missing for `{environment_name}`")
                    })?;
                    println!("{}", format_policy_limits_document(&document));
                }
                Ok(RemoteFetch::Missing) => {
                    println!("No policy limits document is available for `{environment_name}`.");
                }
                Err(error) => {
                    if let Some(document) = cache.load_policy_limits()? {
                        println!("{}", format_policy_limits_document(&document));
                    } else {
                        return Err(error);
                    }
                }
            }
        }
    }

    Ok(())
}

fn build_remote_sync_client(environment_name: &str) -> Result<RemoteSyncClient> {
    let config = load_or_default(None)?;
    let auth_store = LocalAuthStoreBackend::default().load_auth_store()?;
    let access = remote_environment_access(&config, &auth_store, environment_name)?;
    RemoteSyncClient::new(access.server_url, access.access_token, access.device_token)
}

fn read_json<T>(path: &Path) -> Result<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read snapshot {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse snapshot {}", path.display()))
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create snapshot dir {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(value).context("failed to serialize sync snapshot")?;
    fs::write(path, raw).with_context(|| format!("failed to write snapshot {}", path.display()))
}
