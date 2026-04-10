mod http;
mod local_control;
mod state;
mod types;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_admin;

use std::path::PathBuf;

use anyhow::Result;
use hellox_sync::{
    ManagedSettingsDocument, PolicyLimitsDocument, SettingsSyncSnapshot, TeamMemorySnapshot,
};

pub use local_control::LocalServerControlPlane;
pub use types::{
    format_direct_connect_config, format_server_session_detail, format_server_session_list,
    format_server_status, DirectConnectConfig, DirectConnectRequest, ServerSessionDetail,
    ServerSessionSummary, ServerStatus,
};

pub async fn serve(config_path: Option<PathBuf>) -> Result<()> {
    LocalServerControlPlane::new(config_path).serve().await
}

pub fn inspect_server_status(config_path: Option<PathBuf>) -> Result<ServerStatus> {
    LocalServerControlPlane::new(config_path).inspect_status()
}

pub fn create_direct_connect_config(
    config_path: Option<PathBuf>,
    request: DirectConnectRequest,
) -> Result<DirectConnectConfig> {
    LocalServerControlPlane::new(config_path).create_direct_connect(request)
}

pub fn inspect_registered_sessions(
    config_path: Option<PathBuf>,
) -> Result<Vec<ServerSessionSummary>> {
    LocalServerControlPlane::new(config_path).inspect_registered_sessions()
}

pub fn inspect_registered_session(
    config_path: Option<PathBuf>,
    session_id: &str,
) -> Result<ServerSessionDetail> {
    LocalServerControlPlane::new(config_path).inspect_registered_session(session_id)
}

pub fn inspect_managed_settings(
    config_path: Option<PathBuf>,
) -> Result<Option<ManagedSettingsDocument>> {
    LocalServerControlPlane::new(config_path).inspect_managed_settings()
}

pub fn set_managed_settings(
    config_path: Option<PathBuf>,
    config_toml: String,
    signature: Option<String>,
) -> Result<ManagedSettingsDocument> {
    LocalServerControlPlane::new(config_path).set_managed_settings(config_toml, signature)
}

pub fn inspect_policy_limits(config_path: Option<PathBuf>) -> Result<Option<PolicyLimitsDocument>> {
    LocalServerControlPlane::new(config_path).inspect_policy_limits()
}

pub fn set_policy_limits(
    config_path: Option<PathBuf>,
    disabled_commands: Vec<String>,
    disabled_tools: Vec<String>,
    notes: Option<String>,
) -> Result<PolicyLimitsDocument> {
    LocalServerControlPlane::new(config_path).set_policy_limits(
        disabled_commands,
        disabled_tools,
        notes,
    )
}

pub fn inspect_synced_settings(
    config_path: Option<PathBuf>,
    account_id: &str,
) -> Result<Option<SettingsSyncSnapshot>> {
    LocalServerControlPlane::new(config_path).inspect_synced_settings(account_id)
}

pub fn inspect_synced_team_memory(
    config_path: Option<PathBuf>,
    account_id: &str,
    repo_id: &str,
) -> Result<Option<TeamMemorySnapshot>> {
    LocalServerControlPlane::new(config_path).inspect_synced_team_memory(account_id, repo_id)
}

#[cfg(test)]
pub(crate) use http::{persist_managed_settings, persist_policy_limits};
#[cfg(test)]
pub(crate) use state::build_state;
