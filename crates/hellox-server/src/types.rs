use serde::{Deserialize, Serialize};

use hellox_bridge::BridgeStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerStatus {
    pub listen: String,
    pub base_url: String,
    pub bridge: BridgeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectConnectRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectConnectConfig {
    pub server_url: String,
    pub connect_url: String,
    pub session_id: String,
    pub model: String,
    pub working_directory: String,
    pub source: String,
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default)]
    pub owner_account_id: Option<String>,
    #[serde(default)]
    pub owner_device_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSessionSummary {
    pub session_id: String,
    pub model: String,
    pub working_directory: String,
    pub source: String,
    pub owner_account_id: String,
    #[serde(default)]
    pub owner_device_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub persisted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSessionDetail {
    #[serde(flatten)]
    pub summary: ServerSessionSummary,
    #[serde(default)]
    pub owner_device_name: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub shell_name: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    pub message_count: usize,
}

pub fn format_server_status(status: &ServerStatus) -> String {
    format!(
        "listen: {}\nbase_url: {}\n{}",
        status.listen,
        status.base_url,
        hellox_bridge::format_bridge_status(&status.bridge)
    )
}

pub fn format_direct_connect_config(config: &DirectConnectConfig) -> String {
    format!(
        "server_url: {}\nconnect_url: {}\nsession_id: {}\nmodel: {}\nworking_directory: {}\nsource: {}\nauth_token: {}\nowner_account_id: {}\nowner_device_id: {}",
        config.server_url,
        config.connect_url,
        config.session_id,
        config.model,
        config.working_directory,
        config.source,
        config.auth_token.as_deref().unwrap_or("(none)"),
        config.owner_account_id.as_deref().unwrap_or("(none)"),
        config.owner_device_id.as_deref().unwrap_or("(none)")
    )
}

pub fn format_server_session_list(sessions: &[ServerSessionSummary]) -> String {
    if sessions.is_empty() {
        return "No remote sessions available.".to_string();
    }

    let mut lines = vec![
        "session_id\tmodel\tworking_directory\tsource\towner_account_id\towner_device_id\tupdated_at\tpersisted"
            .to_string(),
    ];
    for session in sessions {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            session.session_id,
            session.model,
            session.working_directory,
            session.source,
            session.owner_account_id,
            session.owner_device_id.as_deref().unwrap_or("-"),
            session.updated_at,
            session.persisted
        ));
    }
    lines.join("\n")
}

pub fn format_server_session_detail(detail: &ServerSessionDetail) -> String {
    format!(
        "session_id: {}\nmodel: {}\nworking_directory: {}\nsource: {}\nowner_account_id: {}\nowner_device_id: {}\nowner_device_name: {}\npermission_mode: {}\nshell_name: {}\nsystem_prompt: {}\nmessage_count: {}\ncreated_at: {}\nupdated_at: {}\npersisted: {}",
        detail.summary.session_id,
        detail.summary.model,
        detail.summary.working_directory,
        detail.summary.source,
        detail.summary.owner_account_id,
        detail.summary.owner_device_id.as_deref().unwrap_or("(none)"),
        detail.owner_device_name.as_deref().unwrap_or("(none)"),
        detail.permission_mode.as_deref().unwrap_or("(none)"),
        detail.shell_name.as_deref().unwrap_or("(none)"),
        detail.system_prompt.as_deref().unwrap_or("(none)"),
        detail.message_count,
        detail.summary.created_at,
        detail.summary.updated_at,
        detail.summary.persisted
    )
}
