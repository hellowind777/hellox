use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use hellox_config::config_root;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{AUTHORIZATION, ETAG, IF_NONE_MATCH};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{SettingsSyncSnapshot, TeamMemorySnapshot};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedSettingsDocument {
    pub updated_at: u64,
    pub config_toml: String,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyLimitsDocument {
    pub updated_at: u64,
    #[serde(default)]
    pub disabled_commands: Vec<String>,
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteDocument<T> {
    pub value: T,
    #[serde(default)]
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedRemoteDocument<T> {
    pub environment_name: String,
    #[serde(default)]
    pub etag: Option<String>,
    pub cached_at: u64,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteFetch<T> {
    Updated(RemoteDocument<T>),
    NotModified { etag: Option<String> },
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSyncClient {
    server_url: String,
    access_token: String,
    device_token: Option<String>,
}

impl RemoteSyncClient {
    pub fn new(
        server_url: impl Into<String>,
        access_token: impl Into<String>,
        device_token: Option<String>,
    ) -> Self {
        Self {
            server_url: server_url.into().trim_end_matches('/').to_string(),
            access_token: access_token.into(),
            device_token,
        }
    }

    pub fn push_settings_snapshot(
        &self,
        snapshot: &SettingsSyncSnapshot,
    ) -> Result<SettingsSyncSnapshot> {
        self.request(Method::PUT, "/sync/settings")
            .json(snapshot)
            .send()
            .context("failed to upload settings snapshot")
            .and_then(read_json_response)
    }

    pub fn pull_settings_snapshot(&self) -> Result<Option<SettingsSyncSnapshot>> {
        let response = self
            .request(Method::GET, "/sync/settings")
            .send()
            .context("failed to download settings snapshot")?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        read_json_response(response).map(Some)
    }

    pub fn sync_team_memory_snapshot(
        &self,
        repo_id: &str,
        snapshot: &TeamMemorySnapshot,
    ) -> Result<TeamMemorySnapshot> {
        self.request(Method::PUT, &format!("/sync/team-memory/{repo_id}"))
            .json(snapshot)
            .send()
            .context("failed to sync team memory snapshot")
            .and_then(read_json_response)
    }

    pub fn fetch_managed_settings(
        &self,
        etag: Option<&str>,
    ) -> Result<RemoteFetch<ManagedSettingsDocument>> {
        self.fetch_document("/managed-settings", etag)
    }

    pub fn fetch_policy_limits(
        &self,
        etag: Option<&str>,
    ) -> Result<RemoteFetch<PolicyLimitsDocument>> {
        self.fetch_document("/policy-limits", etag)
    }

    fn fetch_document<T>(&self, path: &str, etag: Option<&str>) -> Result<RemoteFetch<T>>
    where
        T: DeserializeOwned,
    {
        let mut request = self.request(Method::GET, path);
        if let Some(etag) = etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        let response = request
            .send()
            .with_context(|| format!("failed to fetch remote document from {path}"))?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(RemoteFetch::Missing),
            StatusCode::NOT_MODIFIED => Ok(RemoteFetch::NotModified {
                etag: response
                    .headers()
                    .get(ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(ToString::to_string),
            }),
            _ => read_remote_document(response).map(RemoteFetch::Updated),
        }
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let mut request = Client::new()
            .request(method, format!("{}{}", self.server_url, path))
            .header(AUTHORIZATION, format!("Bearer {}", self.access_token));
        if let Some(device_token) = self.device_token.as_deref() {
            request = request.header("x-hellox-device-token", device_token);
        }
        request
    }
}

pub fn cache_root() -> PathBuf {
    config_root().join("sync").join("cache")
}

pub fn managed_settings_cache_path(environment_name: &str) -> PathBuf {
    cache_root()
        .join("managed-settings")
        .join(format!("{environment_name}.json"))
}

pub fn policy_limits_cache_path(environment_name: &str) -> PathBuf {
    cache_root()
        .join("policy-limits")
        .join(format!("{environment_name}.json"))
}

pub fn fetch_cached_managed_settings(
    environment_name: &str,
) -> Result<Option<CachedRemoteDocument<ManagedSettingsDocument>>> {
    fetch_cached_document(&managed_settings_cache_path(environment_name))
}

pub fn fetch_cached_policy_limits(
    environment_name: &str,
) -> Result<Option<CachedRemoteDocument<PolicyLimitsDocument>>> {
    fetch_cached_document(&policy_limits_cache_path(environment_name))
}

pub fn fetch_cached_document<T>(path: &Path) -> Result<Option<CachedRemoteDocument<T>>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read cache {}", path.display()))?;
    let cached = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse cache {}", path.display()))?;
    Ok(Some(cached))
}

pub fn persist_cached_document<T>(
    path: &Path,
    environment_name: &str,
    document: &RemoteDocument<T>,
) -> Result<PathBuf>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cache dir {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(&CachedRemoteDocument {
        environment_name: environment_name.to_string(),
        etag: document.etag.clone(),
        cached_at: unix_timestamp(),
        value: &document.value,
    })
    .context("failed to serialize cached remote document")?;
    fs::write(path, raw).with_context(|| format!("failed to write cache {}", path.display()))?;
    Ok(path.to_path_buf())
}

pub fn compute_document_etag(value: &impl Serialize) -> Result<String> {
    let raw = serde_json::to_vec(value).context("failed to serialize remote document")?;
    Ok(Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn format_managed_settings_document(document: &ManagedSettingsDocument) -> String {
    format!(
        "updated_at: {}\nsignature: {}\nconfig_toml:\n{}",
        document.updated_at,
        document.signature.as_deref().unwrap_or("(none)"),
        document.config_toml
    )
}

pub fn format_policy_limits_document(document: &PolicyLimitsDocument) -> String {
    format!(
        "updated_at: {}\ndisabled_commands: {}\ndisabled_tools: {}\nnotes: {}",
        document.updated_at,
        if document.disabled_commands.is_empty() {
            "(none)".to_string()
        } else {
            document.disabled_commands.join(", ")
        },
        if document.disabled_tools.is_empty() {
            "(none)".to_string()
        } else {
            document.disabled_tools.join(", ")
        },
        document.notes.as_deref().unwrap_or("(none)")
    )
}

fn read_remote_document<T>(response: reqwest::blocking::Response) -> Result<RemoteDocument<T>>
where
    T: DeserializeOwned,
{
    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .unwrap_or_else(|_| "unable to read response body".to_string());
        return Err(anyhow!(
            "remote request failed with status {}: {}",
            status,
            body.trim()
        ));
    }
    let value = response
        .json()
        .context("failed to parse remote JSON document")?;
    Ok(RemoteDocument { value, etag })
}

fn read_json_response<T>(response: reqwest::blocking::Response) -> Result<T>
where
    T: DeserializeOwned,
{
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .unwrap_or_else(|_| "unable to read response body".to_string());
        return Err(anyhow!(
            "remote request failed with status {}: {}",
            status,
            body.trim()
        ));
    }
    response
        .json()
        .context("failed to parse JSON response from remote server")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
