use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRecord {
    pub name: String,
    #[serde(default)]
    pub layout: TeamLayoutRecord,
    pub members: Vec<TeamMemberRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamLayoutRecord {
    #[serde(default = "default_layout_strategy")]
    pub strategy: String,
    #[serde(default)]
    pub pane_group: Option<String>,
}

impl Default for TeamLayoutRecord {
    fn default() -> Self {
        Self {
            strategy: default_layout_strategy(),
            pane_group: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMemberRecord {
    pub name: String,
    pub session_id: String,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub layout_slot: Option<String>,
    #[serde(default)]
    pub pane_target: Option<String>,
}

pub fn team_file_path(root: &Path) -> PathBuf {
    root.join(".hellox").join("teams.json")
}

pub fn load_teams(path: &Path) -> Result<Vec<TeamRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read team file {}", path.display()))?;
    serde_json::from_str::<Vec<TeamRecord>>(&raw)
        .with_context(|| format!("failed to parse team file {}", path.display()))
}

pub fn save_teams(path: &Path, teams: &[TeamRecord]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create team directory {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(teams).context("failed to serialize teams")?;
    fs::write(path, raw).with_context(|| format!("failed to write team file {}", path.display()))
}

fn default_layout_strategy() -> String {
    "fanout".to_string()
}

pub(crate) fn default_layout_strategy_name() -> &'static str {
    "fanout"
}
