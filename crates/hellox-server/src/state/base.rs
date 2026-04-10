use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use hellox_agent::StoredSessionSnapshot;
use hellox_bridge::{inspect_bridge_status, BridgeRuntimePaths, BridgeStatus};
use hellox_config::{default_config_path, load_or_default, HelloxConfig};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{DirectConnectConfig, DirectConnectRequest, ServerStatus};

#[derive(Clone)]
pub(crate) struct ServerState {
    pub(crate) config: HelloxConfig,
    pub(crate) runtime_paths: BridgeRuntimePaths,
    pub(crate) data_paths: ServerDataPaths,
}

#[derive(Clone)]
pub(crate) struct ServerDataPaths {
    pub(crate) auth_store_path: PathBuf,
    pub(crate) provider_keys_path: PathBuf,
    pub(crate) session_registry_path: PathBuf,
    pub(crate) settings_root: PathBuf,
    pub(crate) team_memory_root: PathBuf,
    pub(crate) managed_settings_path: PathBuf,
    pub(crate) policy_limits_path: PathBuf,
}

pub(crate) fn build_state(config_path: Option<PathBuf>) -> Result<ServerState> {
    let config_path = config_path.unwrap_or_else(default_config_path);
    let config = load_or_default(Some(config_path.clone()))?;
    let runtime_paths = runtime_paths(&config_path);
    let data_paths = data_paths(&config_path);
    Ok(ServerState {
        config,
        runtime_paths,
        data_paths,
    })
}

pub(crate) fn build_server_status(
    config: &HelloxConfig,
    paths: &BridgeRuntimePaths,
) -> ServerStatus {
    ServerStatus {
        listen: config.server.listen.clone(),
        base_url: default_base_url(&config.server.listen),
        bridge: inspect_bridge_status(paths).unwrap_or_else(|_| BridgeStatus {
            config_path: paths.config_path.display().to_string().replace('\\', "/"),
            sessions_root: paths.sessions_root.display().to_string().replace('\\', "/"),
            plugins_root: paths.plugins_root.display().to_string().replace('\\', "/"),
            persisted_sessions: 0,
            configured_mcp_servers: 0,
            enabled_mcp_servers: 0,
            installed_plugins: 0,
            enabled_plugins: 0,
        }),
    }
}

pub(crate) fn build_direct_connect_config(
    config: &HelloxConfig,
    persisted: Option<&StoredSessionSnapshot>,
    request: DirectConnectRequest,
) -> DirectConnectConfig {
    let server_url = request
        .base_url
        .clone()
        .map(|value| sanitize_base_url(&value))
        .unwrap_or_else(|| default_base_url(&config.server.listen));
    let session_id = request
        .session_id
        .or_else(|| persisted.map(|snapshot| snapshot.session_id.clone()))
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let model = request
        .model
        .or_else(|| persisted.map(|snapshot| snapshot.model.clone()))
        .unwrap_or_else(|| "opus".to_string());
    let working_directory = request
        .working_directory
        .or_else(|| {
            persisted.map(|snapshot| normalize_path(Path::new(&snapshot.working_directory)))
        })
        .unwrap_or_else(|| ".".to_string());

    DirectConnectConfig {
        connect_url: build_connect_url(&server_url, &session_id, None),
        server_url,
        session_id,
        model,
        working_directory,
        source: if persisted.is_some() {
            "persisted_session".to_string()
        } else {
            "ad_hoc".to_string()
        },
        auth_token: None,
        owner_account_id: None,
        owner_device_id: None,
    }
}

pub(crate) fn read_session_snapshot(
    paths: &BridgeRuntimePaths,
    session_id: &str,
) -> Result<StoredSessionSnapshot> {
    let path = paths.sessions_root.join(format!("{session_id}.json"));
    if !path.exists() {
        return Err(anyhow!("not_found: session `{session_id}` was not found"));
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read session file {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse session file {}", path.display()))
}

pub(crate) fn read_json_if_exists<T>(path: &Path) -> Result<Option<T>>
where
    T: for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))
        .map(Some)
}

pub(crate) fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(value).context("failed to serialize server state")?;
    fs::write(&path, raw).with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn managed_settings_path(state: &ServerState) -> &Path {
    &state.data_paths.managed_settings_path
}

pub(crate) fn policy_limits_path(state: &ServerState) -> &Path {
    &state.data_paths.policy_limits_path
}

pub(crate) fn build_connect_url(
    server_url: &str,
    session_id: &str,
    auth_token: Option<&str>,
) -> String {
    let host = server_url
        .trim()
        .trim_end_matches('/')
        .replace("https://", "")
        .replace("http://", "");
    match auth_token {
        Some(auth_token) => format!("cc://{host}?session_id={session_id}&auth_token={auth_token}"),
        None => format!("cc://{host}?session_id={session_id}"),
    }
}

pub(crate) fn default_base_url(listen: &str) -> String {
    let normalized = if let Some(port) = listen.strip_prefix("0.0.0.0:") {
        format!("127.0.0.1:{port}")
    } else {
        listen.to_string()
    };
    format!("http://{}", normalized.trim_end_matches('/'))
}

pub(crate) fn sanitize_base_url(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

pub(crate) fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn runtime_paths(config_path: &Path) -> BridgeRuntimePaths {
    let root = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    BridgeRuntimePaths::new(
        config_path.to_path_buf(),
        root.join("sessions"),
        root.join("plugins"),
    )
}

fn data_paths(config_path: &Path) -> ServerDataPaths {
    let root = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let server_root = root.join("server");
    ServerDataPaths {
        auth_store_path: root.join("oauth-tokens.json"),
        provider_keys_path: root.join("provider-keys.json"),
        session_registry_path: server_root.join("session-ownership.json"),
        settings_root: server_root.join("settings"),
        team_memory_root: server_root.join("team-memory"),
        managed_settings_path: server_root.join("managed-settings.json"),
        policy_limits_path: server_root.join("policy-limits.json"),
    }
}
