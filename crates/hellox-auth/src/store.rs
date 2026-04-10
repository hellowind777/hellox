use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use hellox_config::config_root;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::unix_timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AuthStore {
    #[serde(default)]
    pub accounts: BTreeMap<String, AuthAccount>,
    #[serde(default)]
    pub provider_keys: BTreeMap<String, ProviderKey>,
    #[serde(default)]
    pub trusted_devices: BTreeMap<String, TrustedDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthAccount {
    pub account_id: String,
    pub provider: String,
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub updated_at: u64,
    #[serde(default)]
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderKey {
    pub provider: String,
    pub api_key: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedDevice {
    pub device_id: String,
    pub account_id: String,
    pub device_name: String,
    pub device_token: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub last_validated_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteIdentity {
    pub account_id: String,
    pub provider: String,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub device_name: Option<String>,
}

pub fn auth_store_path() -> PathBuf {
    config_root().join("oauth-tokens.json")
}

pub fn provider_keys_path() -> PathBuf {
    config_root().join("provider-keys.json")
}

pub fn trusted_devices_path() -> PathBuf {
    config_root().join("trusted-devices.json")
}

pub fn load_auth_store(
    store_path: Option<PathBuf>,
    keys_path: Option<PathBuf>,
) -> Result<AuthStore> {
    let store_path = store_path.unwrap_or_else(auth_store_path);
    let keys_path = keys_path.unwrap_or_else(provider_keys_path);
    let devices_path = companion_path(
        Some(store_path.clone()),
        Some(keys_path.clone()),
        "trusted-devices.json",
        trusted_devices_path,
    );

    let mut store: AuthStore = read_json_or_default(store_path)?;
    store.provider_keys = read_json_or_default(keys_path)?;
    store.trusted_devices = read_json_or_default(devices_path)?;
    Ok(store)
}

pub fn save_auth_store(
    store_path: Option<PathBuf>,
    keys_path: Option<PathBuf>,
    store: &AuthStore,
) -> Result<()> {
    let store_path = store_path.unwrap_or_else(auth_store_path);
    let keys_path = keys_path.unwrap_or_else(provider_keys_path);
    let devices_path = companion_path(
        Some(store_path.clone()),
        Some(keys_path.clone()),
        "trusted-devices.json",
        trusted_devices_path,
    );

    write_json(store_path, store)?;
    write_json(keys_path, &store.provider_keys)?;
    write_json(devices_path, &store.trusted_devices)?;
    Ok(())
}

pub fn login_account(
    store: &mut AuthStore,
    account_id: String,
    provider: String,
    access_token: String,
    refresh_token: Option<String>,
    scopes: Vec<String>,
) {
    upsert_auth_account(
        store,
        AuthAccount {
            account_id,
            provider,
            access_token,
            refresh_token: sanitize_optional(refresh_token),
            scopes: sanitize_scopes(scopes),
            updated_at: unix_timestamp(),
            expires_at: None,
        },
    );
}

pub fn upsert_auth_account(store: &mut AuthStore, account: AuthAccount) {
    store.accounts.insert(account.account_id.clone(), account);
}

pub fn logout_account(store: &mut AuthStore, account_id: &str) -> Result<AuthAccount> {
    store
        .accounts
        .remove(account_id)
        .ok_or_else(|| anyhow!("Auth account `{account_id}` was not found"))
}

pub fn set_provider_key(store: &mut AuthStore, provider: String, api_key: String) {
    store.provider_keys.insert(
        provider.clone(),
        ProviderKey {
            provider,
            api_key,
            updated_at: unix_timestamp(),
        },
    );
}

pub fn remove_provider_key(store: &mut AuthStore, provider: &str) -> Result<ProviderKey> {
    store
        .provider_keys
        .remove(provider)
        .ok_or_else(|| anyhow!("Provider key `{provider}` was not found"))
}

/// Register a trusted device for a stored account and return the generated device secret once.
pub fn trust_device(
    store: &mut AuthStore,
    account_id: &str,
    device_name: String,
    scopes: Vec<String>,
) -> Result<TrustedDevice> {
    find_auth_account(store, account_id)?;
    let now = unix_timestamp();
    let device = TrustedDevice {
        device_id: Uuid::new_v4().to_string(),
        account_id: account_id.to_string(),
        device_name: device_name.trim().to_string(),
        device_token: format!("hdv_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
        scopes: sanitize_scopes(scopes),
        created_at: now,
        updated_at: now,
        last_validated_at: None,
    };
    store
        .trusted_devices
        .insert(device.device_id.clone(), device.clone());
    Ok(device)
}

pub fn revoke_device(store: &mut AuthStore, device_id: &str) -> Result<TrustedDevice> {
    store
        .trusted_devices
        .remove(device_id)
        .ok_or_else(|| anyhow!("Trusted device `{device_id}` was not found"))
}

pub fn mark_device_validated(store: &mut AuthStore, device_id: &str) -> Result<()> {
    let device = store
        .trusted_devices
        .get_mut(device_id)
        .ok_or_else(|| anyhow!("Trusted device `{device_id}` was not found"))?;
    let now = unix_timestamp();
    device.updated_at = now;
    device.last_validated_at = Some(now);
    Ok(())
}

pub fn find_auth_account<'a>(store: &'a AuthStore, account_id: &str) -> Result<&'a AuthAccount> {
    store
        .accounts
        .get(account_id)
        .ok_or_else(|| anyhow!("Auth account `{account_id}` was not found"))
}

pub fn find_trusted_device<'a>(store: &'a AuthStore, device_id: &str) -> Result<&'a TrustedDevice> {
    store
        .trusted_devices
        .get(device_id)
        .ok_or_else(|| anyhow!("Trusted device `{device_id}` was not found"))
}

pub fn find_account_by_access_token<'a>(
    store: &'a AuthStore,
    access_token: &str,
) -> Option<&'a AuthAccount> {
    store
        .accounts
        .values()
        .find(|account| account.access_token == access_token)
}

pub fn find_device_by_token<'a>(
    store: &'a AuthStore,
    device_token: &str,
) -> Option<&'a TrustedDevice> {
    store
        .trusted_devices
        .values()
        .find(|device| device.device_token == device_token)
}

/// Validate remote bearer credentials and an optional trusted-device token.
pub fn validate_remote_identity(
    store: &AuthStore,
    access_token: &str,
    device_token: Option<&str>,
) -> Result<RemoteIdentity> {
    let account = find_account_by_access_token(store, access_token)
        .ok_or_else(|| anyhow!("Remote bearer token does not match a stored auth account"))?;

    let (device_id, device_name) = match device_token {
        Some(token) => {
            let device = find_device_by_token(store, token)
                .ok_or_else(|| anyhow!("Remote device token does not match a trusted device"))?;
            if device.account_id != account.account_id {
                return Err(anyhow!(
                    "Trusted device `{}` does not belong to account `{}`",
                    device.device_id,
                    account.account_id
                ));
            }
            (
                Some(device.device_id.clone()),
                Some(device.device_name.clone()),
            )
        }
        None => (None, None),
    };

    Ok(RemoteIdentity {
        account_id: account.account_id.clone(),
        provider: account.provider.clone(),
        device_id,
        device_name,
    })
}

pub fn format_auth_summary(store: &AuthStore) -> String {
    format!(
        "accounts: {}\nprovider_keys: {}\ntrusted_devices: {}",
        store.accounts.len(),
        store.provider_keys.len(),
        store.trusted_devices.len()
    )
}

pub fn format_account_list(store: &AuthStore) -> String {
    if store.accounts.is_empty() {
        return "No auth accounts stored.".to_string();
    }

    let mut lines =
        vec!["account_id\tprovider\tscopes\tupdated_at\texpires_at\taccess_token".to_string()];
    for account in store.accounts.values() {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            account.account_id,
            account.provider,
            display_scopes(&account.scopes),
            account.updated_at,
            account
                .expires_at
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            mask_secret(&account.access_token)
        ));
    }
    lines.join("\n")
}

pub fn format_account_detail(account: &AuthAccount) -> String {
    format!(
        "account_id: {}\nprovider: {}\naccess_token: {}\nrefresh_token: {}\nscopes: {}\nupdated_at: {}\nexpires_at: {}",
        account.account_id,
        account.provider,
        mask_secret(&account.access_token),
        account
            .refresh_token
            .as_deref()
            .map(mask_secret)
            .unwrap_or_else(|| "(none)".to_string()),
        display_scopes(&account.scopes),
        account.updated_at,
        account
            .expires_at
            .map(|value| value.to_string())
            .unwrap_or_else(|| "(none)".to_string())
    )
}

pub fn format_provider_key_list(store: &AuthStore) -> String {
    if store.provider_keys.is_empty() {
        return "No provider keys stored.".to_string();
    }

    let mut lines = vec!["provider\tupdated_at\tapi_key".to_string()];
    for key in store.provider_keys.values() {
        lines.push(format!(
            "{}\t{}\t{}",
            key.provider,
            key.updated_at,
            mask_secret(&key.api_key)
        ));
    }
    lines.join("\n")
}

pub fn format_device_list(store: &AuthStore) -> String {
    if store.trusted_devices.is_empty() {
        return "No trusted devices stored.".to_string();
    }

    let mut lines =
        vec!["device_id\taccount_id\tdevice_name\tscopes\tupdated_at\tdevice_token".to_string()];
    for device in store.trusted_devices.values() {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            device.device_id,
            device.account_id,
            device.device_name,
            display_scopes(&device.scopes),
            device.updated_at,
            mask_secret(&device.device_token)
        ));
    }
    lines.join("\n")
}

pub fn format_device_detail(device: &TrustedDevice) -> String {
    format!(
        "device_id: {}\naccount_id: {}\ndevice_name: {}\ndevice_token: {}\nscopes: {}\ncreated_at: {}\nupdated_at: {}\nlast_validated_at: {}",
        device.device_id,
        device.account_id,
        device.device_name,
        mask_secret(&device.device_token),
        display_scopes(&device.scopes),
        device.created_at,
        device.updated_at,
        device
            .last_validated_at
            .map(|value| value.to_string())
            .unwrap_or_else(|| "(never)".to_string())
    )
}

fn read_json_or_default<T>(path: PathBuf) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read auth file {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse auth file {}", path.display()))
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create auth dir {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(value).context("failed to serialize auth store")?;
    fs::write(&path, raw).with_context(|| format!("failed to write auth file {}", path.display()))
}

fn companion_path(
    store_path: Option<PathBuf>,
    alternate_path: Option<PathBuf>,
    file_name: &str,
    fallback: impl FnOnce() -> PathBuf,
) -> PathBuf {
    store_path
        .as_ref()
        .and_then(|path| path.parent().map(|parent| parent.join(file_name)))
        .or_else(|| {
            alternate_path
                .as_ref()
                .and_then(|path| path.parent().map(|parent| parent.join(file_name)))
        })
        .unwrap_or_else(fallback)
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn sanitize_scopes(scopes: Vec<String>) -> Vec<String> {
    scopes
        .into_iter()
        .map(|scope| scope.trim().to_string())
        .filter(|scope| !scope.is_empty())
        .collect()
}

fn display_scopes(scopes: &[String]) -> String {
    if scopes.is_empty() {
        "-".to_string()
    } else {
        scopes.join(",")
    }
}

fn mask_secret(value: &str) -> String {
    if value.len() <= 8 {
        "********".to_string()
    } else {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    }
}
