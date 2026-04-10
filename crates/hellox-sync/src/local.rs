use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use hellox_config::{config_root, default_config_path, load_or_default, save_config};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingsSyncSnapshot {
    pub exported_at: u64,
    pub config_toml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TeamMemorySnapshot {
    pub repo_id: String,
    pub exported_at: u64,
    #[serde(default)]
    pub entries: BTreeMap<String, TeamMemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamMemoryEntry {
    pub content: String,
    pub updated_at: u64,
}

pub fn export_settings_snapshot(config_path: Option<PathBuf>) -> Result<SettingsSyncSnapshot> {
    let config_path = config_path.unwrap_or_else(default_config_path);
    let config = load_or_default(Some(config_path))?;
    Ok(SettingsSyncSnapshot {
        exported_at: unix_timestamp(),
        config_toml: toml::to_string_pretty(&config)
            .context("failed to render settings snapshot")?,
    })
}

pub fn import_settings_snapshot(
    config_path: Option<PathBuf>,
    snapshot: &SettingsSyncSnapshot,
) -> Result<PathBuf> {
    let config =
        toml::from_str(&snapshot.config_toml).context("failed to parse settings snapshot")?;
    save_config(config_path, &config)
}

pub fn format_settings_snapshot(snapshot: &SettingsSyncSnapshot) -> String {
    format!(
        "exported_at: {}\nconfig_toml:\n{}",
        snapshot.exported_at, snapshot.config_toml
    )
}

pub fn load_team_memory_snapshot(repo_id: &str) -> Result<TeamMemorySnapshot> {
    load_team_memory_snapshot_from(&config_root(), repo_id)
}

pub fn load_team_memory_snapshot_from(root: &Path, repo_id: &str) -> Result<TeamMemorySnapshot> {
    let path = team_memory_snapshot_path_in(root, repo_id);
    if !path.exists() {
        return Ok(TeamMemorySnapshot {
            repo_id: repo_id.to_string(),
            exported_at: 0,
            entries: BTreeMap::new(),
        });
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read team memory snapshot {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse team memory snapshot {}", path.display()))
}

pub fn put_team_memory_entry(
    repo_id: &str,
    key: String,
    content: String,
) -> Result<TeamMemorySnapshot> {
    put_team_memory_entry_in(&config_root(), repo_id, key, content)
}

pub fn put_team_memory_entry_in(
    root: &Path,
    repo_id: &str,
    key: String,
    content: String,
) -> Result<TeamMemorySnapshot> {
    let mut snapshot = load_team_memory_snapshot_from(root, repo_id)?;
    snapshot.repo_id = repo_id.to_string();
    snapshot.exported_at = unix_timestamp();
    snapshot.entries.insert(
        key,
        TeamMemoryEntry {
            content,
            updated_at: unix_timestamp(),
        },
    );
    save_team_memory_snapshot_in(root, &snapshot)?;
    Ok(snapshot)
}

pub fn remove_team_memory_entry(repo_id: &str, key: &str) -> Result<TeamMemorySnapshot> {
    remove_team_memory_entry_in(&config_root(), repo_id, key)
}

pub fn remove_team_memory_entry_in(
    root: &Path,
    repo_id: &str,
    key: &str,
) -> Result<TeamMemorySnapshot> {
    let mut snapshot = load_team_memory_snapshot_from(root, repo_id)?;
    snapshot.entries.remove(key);
    snapshot.exported_at = unix_timestamp();
    save_team_memory_snapshot_in(root, &snapshot)?;
    Ok(snapshot)
}

pub fn merge_team_memory_snapshot(
    repo_id: &str,
    incoming: TeamMemorySnapshot,
) -> Result<TeamMemorySnapshot> {
    merge_team_memory_snapshot_in(&config_root(), repo_id, incoming)
}

pub fn merge_team_memory_snapshot_in(
    root: &Path,
    repo_id: &str,
    incoming: TeamMemorySnapshot,
) -> Result<TeamMemorySnapshot> {
    let mut local = load_team_memory_snapshot_from(root, repo_id)?;
    local.repo_id = repo_id.to_string();
    local.exported_at = unix_timestamp();
    for (key, value) in incoming.entries {
        local.entries.insert(key, value);
    }
    save_team_memory_snapshot_in(root, &local)?;
    Ok(local)
}

pub fn format_team_memory_snapshot(snapshot: &TeamMemorySnapshot) -> String {
    if snapshot.entries.is_empty() {
        return format!(
            "repo_id: {}\nexported_at: {}\nentries: (none)",
            snapshot.repo_id, snapshot.exported_at
        );
    }

    let mut lines = vec![
        format!("repo_id: {}", snapshot.repo_id),
        format!("exported_at: {}", snapshot.exported_at),
        "entries:".to_string(),
    ];
    for (key, value) in &snapshot.entries {
        lines.push(format!(
            "- {}: {} ({})",
            key, value.content, value.updated_at
        ));
    }
    lines.join("\n")
}

pub fn team_memory_snapshot_path(repo_id: &str) -> PathBuf {
    team_memory_snapshot_path_in(&config_root(), repo_id)
}

pub fn team_memory_snapshot_path_in(root: &Path, repo_id: &str) -> PathBuf {
    root.join("sync")
        .join("team-memory")
        .join(format!("{repo_id}.json"))
}

fn save_team_memory_snapshot_in(root: &Path, snapshot: &TeamMemorySnapshot) -> Result<()> {
    let path = team_memory_snapshot_path_in(root, &snapshot.repo_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create sync dir {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(snapshot)
        .context("failed to serialize team memory snapshot")?;
    fs::write(&path, raw)
        .with_context(|| format!("failed to write team memory snapshot {}", path.display()))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
