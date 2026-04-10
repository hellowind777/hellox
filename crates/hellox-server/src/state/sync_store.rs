use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use hellox_auth::RemoteIdentity;
use hellox_sync::{
    FileManagedSettingsSource, FilePolicyLimitsSource, ManagedSettingsDocument,
    ManagedSettingsSource, PolicyLimitsDocument, PolicyLimitsSource, SettingsSyncSnapshot,
    TeamMemorySnapshot,
};

use super::base::{
    managed_settings_path, policy_limits_path, read_json_if_exists, unix_timestamp, write_json,
    ServerState,
};

pub(crate) fn save_settings_snapshot(
    state: &ServerState,
    identity: &RemoteIdentity,
    snapshot: &SettingsSyncSnapshot,
) -> Result<SettingsSyncSnapshot> {
    write_json(
        state
            .data_paths
            .settings_root
            .join(format!("{}.json", identity.account_id)),
        snapshot,
    )?;
    Ok(snapshot.clone())
}

pub(crate) fn load_settings_snapshot(
    state: &ServerState,
    identity: &RemoteIdentity,
) -> Result<Option<SettingsSyncSnapshot>> {
    inspect_settings_snapshot(state, &identity.account_id)
}

pub(crate) fn inspect_settings_snapshot(
    state: &ServerState,
    account_id: &str,
) -> Result<Option<SettingsSyncSnapshot>> {
    read_json_if_exists(
        &state
            .data_paths
            .settings_root
            .join(format!("{account_id}.json")),
    )
}

pub(crate) fn sync_team_memory(
    state: &ServerState,
    identity: &RemoteIdentity,
    repo_id: &str,
    incoming: TeamMemorySnapshot,
) -> Result<TeamMemorySnapshot> {
    let path = team_memory_path(state, &identity.account_id, repo_id);
    let mut current =
        read_json_if_exists::<TeamMemorySnapshot>(&path)?.unwrap_or(TeamMemorySnapshot {
            repo_id: repo_id.to_string(),
            exported_at: 0,
            entries: BTreeMap::new(),
        });
    current.repo_id = repo_id.to_string();
    current.exported_at = unix_timestamp();
    for (key, value) in incoming.entries {
        match current.entries.get(&key) {
            Some(existing) if existing.updated_at > value.updated_at => {}
            _ => {
                current.entries.insert(key, value);
            }
        }
    }
    write_json(path, &current)?;
    Ok(current)
}

pub(crate) fn inspect_team_memory_snapshot(
    state: &ServerState,
    account_id: &str,
    repo_id: &str,
) -> Result<Option<TeamMemorySnapshot>> {
    read_json_if_exists(&team_memory_path(state, account_id, repo_id))
}

pub(crate) fn inspect_managed_settings(
    state: &ServerState,
) -> Result<Option<ManagedSettingsDocument>> {
    FileManagedSettingsSource::new(managed_settings_path(state)).load_managed_settings()
}

pub(crate) fn save_managed_settings(
    state: &ServerState,
    config_toml: String,
    signature: Option<String>,
) -> Result<ManagedSettingsDocument> {
    let document = ManagedSettingsDocument {
        updated_at: unix_timestamp(),
        config_toml,
        signature: signature
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    };
    write_managed_settings(managed_settings_path(state), &document)?;
    Ok(document)
}

pub(crate) fn inspect_policy_limits(state: &ServerState) -> Result<Option<PolicyLimitsDocument>> {
    FilePolicyLimitsSource::new(policy_limits_path(state)).load_policy_limits()
}

pub(crate) fn save_policy_limits(
    state: &ServerState,
    disabled_commands: Vec<String>,
    disabled_tools: Vec<String>,
    notes: Option<String>,
) -> Result<PolicyLimitsDocument> {
    let document = PolicyLimitsDocument {
        updated_at: unix_timestamp(),
        disabled_commands: normalize_values(disabled_commands),
        disabled_tools: normalize_values(disabled_tools),
        notes: notes
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    };
    write_policy_limits(policy_limits_path(state), &document)?;
    Ok(document)
}

pub(crate) fn write_managed_settings(
    path: &Path,
    document: &ManagedSettingsDocument,
) -> Result<()> {
    write_json(path.to_path_buf(), document)
}

pub(crate) fn write_policy_limits(path: &Path, document: &PolicyLimitsDocument) -> Result<()> {
    write_json(path.to_path_buf(), document)
}

fn team_memory_path(state: &ServerState, account_id: &str, repo_id: &str) -> std::path::PathBuf {
    state
        .data_paths
        .team_memory_root
        .join(account_id)
        .join(format!("{repo_id}.json"))
}

fn normalize_values(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}
