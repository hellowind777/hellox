use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use hellox_config::PermissionMode;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AgentRunRequest {
    pub prompt: String,
    pub model: Option<String>,
    pub backend: Option<String>,
    pub isolation: Option<String>,
    pub worktree_name: Option<String>,
    pub worktree_base_ref: Option<String>,
    pub permission_mode: Option<PermissionMode>,
    pub agent_name: Option<String>,
    pub pane_group: Option<String>,
    pub layout_strategy: Option<String>,
    pub layout_slot: Option<String>,
    pub pane_anchor_target: Option<String>,
    pub cwd: Option<String>,
    pub session_id: Option<String>,
    pub max_turns: usize,
    pub reuse_existing_worktree: bool,
    pub run_in_background: bool,
    pub allow_interaction: bool,
}

pub fn optional_string(input: &Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn parse_permission_mode(input: &Value, key: &str) -> Result<Option<PermissionMode>> {
    match optional_string(input, key) {
        Some(value) => value
            .parse::<PermissionMode>()
            .map(Some)
            .map_err(|error| anyhow!(error)),
        None => Ok(None),
    }
}

pub fn render_json(value: Value) -> Result<String> {
    serde_json::to_string_pretty(&value).map_err(Into::into)
}

pub fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

pub fn current_shell_name() -> String {
    std::env::var("SHELL")
        .ok()
        .or_else(|| std::env::var("COMSPEC").ok())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "powershell".to_string()
            } else {
                "sh".to_string()
            }
        })
}

pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
