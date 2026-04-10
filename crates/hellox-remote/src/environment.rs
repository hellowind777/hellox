use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_agent::StoredSessionSnapshot;
use hellox_auth::{find_auth_account, find_trusted_device, AuthStore};
use hellox_config::{HelloxConfig, RemoteEnvironmentConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEnvironmentSummary {
    pub name: String,
    pub enabled: bool,
    pub server_url: String,
    pub token_env: Option<String>,
    pub account_id: Option<String>,
    pub device_id: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TeleportOverrides {
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeleportPlan {
    pub environment_name: String,
    pub enabled: bool,
    pub server_url: String,
    pub connect_url: String,
    pub token_env: Option<String>,
    pub account_id: Option<String>,
    pub device_id: Option<String>,
    pub auth_source: String,
    pub session_id: String,
    pub model: String,
    pub working_directory: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRemoteEnvironment {
    pub environment_name: String,
    pub enabled: bool,
    pub server_url: String,
    pub access_token: String,
    pub device_token: Option<String>,
    pub account_id: Option<String>,
    pub device_id: Option<String>,
}

pub fn list_remote_environments(config: &HelloxConfig) -> Vec<RemoteEnvironmentSummary> {
    config
        .remote
        .environments
        .iter()
        .map(|(name, environment)| RemoteEnvironmentSummary {
            name: name.clone(),
            enabled: environment.enabled,
            server_url: environment.server_url.clone(),
            token_env: environment.token_env.clone(),
            account_id: environment.account_id.clone(),
            device_id: environment.device_id.clone(),
            description: environment.description.clone(),
        })
        .collect()
}

pub fn format_remote_environment_list(environments: &[RemoteEnvironmentSummary]) -> String {
    if environments.is_empty() {
        return "No remote environments configured.".to_string();
    }

    let mut lines = vec![
        "name\tenabled\tserver_url\ttoken_env\taccount_id\tdevice_id\tdescription".to_string(),
    ];
    for environment in environments {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            environment.name,
            environment.enabled,
            environment.server_url,
            environment.token_env.as_deref().unwrap_or("-"),
            environment.account_id.as_deref().unwrap_or("-"),
            environment.device_id.as_deref().unwrap_or("-"),
            environment.description.as_deref().unwrap_or("-")
        ));
    }
    lines.join("\n")
}

pub fn format_remote_environment_detail(environment: &RemoteEnvironmentSummary) -> String {
    let mut lines = vec![
        format!("name: {}", environment.name),
        format!("enabled: {}", environment.enabled),
        format!("server_url: {}", environment.server_url),
        format!(
            "token_env: {}",
            environment.token_env.as_deref().unwrap_or("(none)")
        ),
        format!(
            "account_id: {}",
            environment.account_id.as_deref().unwrap_or("(none)")
        ),
        format!(
            "device_id: {}",
            environment.device_id.as_deref().unwrap_or("(none)")
        ),
    ];
    if let Some(description) = &environment.description {
        lines.push(format!("description: {description}"));
    }
    lines.join("\n")
}

pub fn get_remote_environment<'a>(
    config: &'a HelloxConfig,
    environment_name: &str,
) -> Result<&'a RemoteEnvironmentConfig> {
    config
        .remote
        .environments
        .get(environment_name)
        .ok_or_else(|| anyhow!("Remote environment `{environment_name}` was not found"))
}

pub fn resolve_remote_environment(
    config: &HelloxConfig,
    auth_store: &AuthStore,
    environment_name: &str,
) -> Result<ResolvedRemoteEnvironment> {
    let environment = get_remote_environment(config, environment_name)?;
    if !environment.enabled {
        return Err(anyhow!(
            "Remote environment `{environment_name}` is disabled"
        ));
    }

    let access_token = if let Some(token_env) = environment.token_env.as_deref() {
        std::env::var(token_env)
            .map_err(|_| anyhow!("Environment variable `{token_env}` is not set"))?
    } else if let Some(account_id) = environment.account_id.as_deref() {
        find_auth_account(auth_store, account_id)?
            .access_token
            .clone()
    } else {
        return Err(anyhow!(
            "Remote environment `{environment_name}` is missing token_env or account_id"
        ));
    };

    let device_token = environment
        .device_id
        .as_deref()
        .map(|device_id| {
            find_trusted_device(auth_store, device_id).map(|device| device.device_token.clone())
        })
        .transpose()?;

    Ok(ResolvedRemoteEnvironment {
        environment_name: environment_name.to_string(),
        enabled: environment.enabled,
        server_url: environment.server_url.clone(),
        access_token,
        device_token,
        account_id: environment.account_id.clone(),
        device_id: environment.device_id.clone(),
    })
}

pub fn add_remote_environment(
    config: &mut HelloxConfig,
    environment_name: String,
    environment: RemoteEnvironmentConfig,
) -> Result<()> {
    if config.remote.environments.contains_key(&environment_name) {
        return Err(anyhow!(
            "Remote environment `{environment_name}` already exists"
        ));
    }
    config
        .remote
        .environments
        .insert(environment_name, environment);
    Ok(())
}

pub fn set_remote_environment_enabled(
    config: &mut HelloxConfig,
    environment_name: &str,
    enabled: bool,
) -> Result<()> {
    let environment = config
        .remote
        .environments
        .get_mut(environment_name)
        .ok_or_else(|| anyhow!("Remote environment `{environment_name}` was not found"))?;
    environment.enabled = enabled;
    Ok(())
}

pub fn remove_remote_environment(
    config: &mut HelloxConfig,
    environment_name: &str,
) -> Result<RemoteEnvironmentConfig> {
    config
        .remote
        .environments
        .remove(environment_name)
        .ok_or_else(|| anyhow!("Remote environment `{environment_name}` was not found"))
}

pub fn build_remote_environment(
    server_url: String,
    token_env: Option<String>,
    account_id: Option<String>,
    device_id: Option<String>,
    description: Option<String>,
) -> RemoteEnvironmentConfig {
    RemoteEnvironmentConfig {
        enabled: true,
        server_url: server_url.trim().trim_end_matches('/').to_string(),
        token_env: sanitize_optional(token_env),
        account_id: sanitize_optional(account_id),
        device_id: sanitize_optional(device_id),
        description: sanitize_optional(description),
    }
}

pub fn build_teleport_plan(
    config: &HelloxConfig,
    environment_name: &str,
    session: Option<&StoredSessionSnapshot>,
    overrides: TeleportOverrides,
) -> Result<TeleportPlan> {
    let environment = get_remote_environment(config, environment_name)?;
    let session_id = overrides
        .session_id
        .or_else(|| session.map(|snapshot| snapshot.session_id.clone()))
        .ok_or_else(|| anyhow!("Teleport requires a session id or a persisted session"))?;
    let model = overrides
        .model
        .or_else(|| session.map(|snapshot| snapshot.model.clone()))
        .unwrap_or_else(|| "opus".to_string());
    let working_directory = overrides
        .working_directory
        .or_else(|| session.map(|snapshot| normalize_path(Path::new(&snapshot.working_directory))))
        .unwrap_or_else(|| ".".to_string());

    Ok(TeleportPlan {
        environment_name: environment_name.to_string(),
        enabled: environment.enabled,
        server_url: environment.server_url.clone(),
        connect_url: build_connect_url(&environment.server_url, &session_id, None),
        token_env: environment.token_env.clone(),
        account_id: environment.account_id.clone(),
        device_id: environment.device_id.clone(),
        auth_source: auth_source(environment),
        session_id,
        model,
        working_directory,
        source: if session.is_some() {
            "persisted_session".to_string()
        } else {
            "ad_hoc".to_string()
        },
    })
}

pub fn format_teleport_plan(plan: &TeleportPlan) -> String {
    format!(
        "environment: {}\nenabled: {}\nserver_url: {}\nconnect_url: {}\ntoken_env: {}\naccount_id: {}\ndevice_id: {}\nauth_source: {}\nsession_id: {}\nmodel: {}\nworking_directory: {}\nsource: {}",
        plan.environment_name,
        plan.enabled,
        plan.server_url,
        plan.connect_url,
        plan.token_env.as_deref().unwrap_or("(none)"),
        plan.account_id.as_deref().unwrap_or("(none)"),
        plan.device_id.as_deref().unwrap_or("(none)"),
        plan.auth_source,
        plan.session_id,
        plan.model,
        plan.working_directory,
        plan.source
    )
}

pub(crate) fn build_connect_url(
    server_url: &str,
    session_id: &str,
    auth_token: Option<&str>,
) -> String {
    let normalized = server_url
        .trim()
        .trim_end_matches('/')
        .replace("https://", "")
        .replace("http://", "");
    match auth_token {
        Some(auth_token) => {
            format!("cc://{normalized}?session_id={session_id}&auth_token={auth_token}")
        }
        None => format!("cc://{normalized}?session_id={session_id}"),
    }
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn auth_source(environment: &RemoteEnvironmentConfig) -> String {
    if let Some(token_env) = environment.token_env.as_deref() {
        format!("env:{token_env}")
    } else if let Some(account_id) = environment.account_id.as_deref() {
        format!("account:{account_id}")
    } else {
        "none".to_string()
    }
}
