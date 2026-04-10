use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use axum::http::HeaderMap;
use hellox_auth::{
    load_auth_store, mark_device_validated, save_auth_store, validate_remote_identity,
    RemoteIdentity,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{
    DirectConnectConfig, DirectConnectRequest, ServerSessionDetail, ServerSessionSummary,
};

use super::base::{
    build_connect_url, build_direct_connect_config, read_json_if_exists, read_session_snapshot,
    unix_timestamp, write_json, ServerState,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SessionRegistry {
    #[serde(default)]
    sessions: BTreeMap<String, OwnedSessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedSessionRecord {
    session_id: String,
    model: String,
    working_directory: String,
    source: String,
    owner_account_id: String,
    owner_provider: String,
    #[serde(default)]
    owner_device_id: Option<String>,
    #[serde(default)]
    owner_device_name: Option<String>,
    session_token: String,
    created_at: u64,
    updated_at: u64,
    persisted: bool,
}

pub(crate) fn validate_remote_access(
    state: &ServerState,
    headers: &HeaderMap,
) -> Result<RemoteIdentity> {
    let access_token = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("unauthorized: missing Bearer access token"))?;
    let device_token = headers
        .get("x-hellox-device-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut store = load_auth_store(
        Some(state.data_paths.auth_store_path.clone()),
        Some(state.data_paths.provider_keys_path.clone()),
    )?;
    let identity = validate_remote_identity(&store, access_token, device_token)
        .map_err(|error| anyhow!("unauthorized: {}", error))?;
    if let Some(device_id) = identity.device_id.as_deref() {
        mark_device_validated(&mut store, device_id)?;
        save_auth_store(
            Some(state.data_paths.auth_store_path.clone()),
            Some(state.data_paths.provider_keys_path.clone()),
            &store,
        )?;
    }
    Ok(identity)
}

pub(crate) fn create_owned_session(
    state: &ServerState,
    identity: &RemoteIdentity,
    request: DirectConnectRequest,
) -> Result<DirectConnectConfig> {
    let persisted = match request.session_id.as_deref() {
        Some(session_id) => Some(read_session_snapshot(&state.runtime_paths, session_id)?),
        None => None,
    };
    let mut direct = build_direct_connect_config(&state.config, persisted.as_ref(), request);
    let mut registry = load_session_registry(&state)?;
    let now = unix_timestamp();
    let existing = registry.sessions.get(&direct.session_id).cloned();

    if let Some(existing) = existing.as_ref() {
        if existing.owner_account_id != identity.account_id {
            return Err(anyhow!(
                "forbidden: session `{}` belongs to account `{}`",
                existing.session_id,
                existing.owner_account_id
            ));
        }
    }

    let session_token = existing
        .as_ref()
        .map(|record| record.session_token.clone())
        .unwrap_or_else(|| format!("hs_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()));
    registry.sessions.insert(
        direct.session_id.clone(),
        OwnedSessionRecord {
            session_id: direct.session_id.clone(),
            model: direct.model.clone(),
            working_directory: direct.working_directory.clone(),
            source: direct.source.clone(),
            owner_account_id: identity.account_id.clone(),
            owner_provider: identity.provider.clone(),
            owner_device_id: identity.device_id.clone(),
            owner_device_name: identity.device_name.clone(),
            session_token: session_token.clone(),
            created_at: existing
                .as_ref()
                .map(|record| record.created_at)
                .unwrap_or(now),
            updated_at: now,
            persisted: persisted.is_some(),
        },
    );
    save_session_registry(state, &registry)?;

    direct.connect_url =
        build_connect_url(&direct.server_url, &direct.session_id, Some(&session_token));
    direct.auth_token = Some(session_token);
    direct.owner_account_id = Some(identity.account_id.clone());
    direct.owner_device_id = identity.device_id.clone();
    Ok(direct)
}

pub(crate) fn list_owned_sessions(
    state: &ServerState,
    identity: &RemoteIdentity,
) -> Result<Vec<ServerSessionSummary>> {
    let registry = load_session_registry(state)?;
    Ok(registry
        .sessions
        .values()
        .filter(|record| record.owner_account_id == identity.account_id)
        .map(summary_from_record)
        .collect())
}

pub(crate) fn load_owned_session(
    state: &ServerState,
    identity: &RemoteIdentity,
    session_id: &str,
) -> Result<ServerSessionDetail> {
    let registry = load_session_registry(state)?;
    let record = registry
        .sessions
        .get(session_id)
        .ok_or_else(|| anyhow!("not_found: session `{session_id}` was not found"))?;
    if record.owner_account_id != identity.account_id {
        return Err(anyhow!(
            "forbidden: session `{session_id}` belongs to another account"
        ));
    }
    detail_from_record(state, record)
}

pub(crate) fn inspect_registered_sessions(
    state: &ServerState,
) -> Result<Vec<ServerSessionSummary>> {
    let registry = load_session_registry(state)?;
    Ok(registry
        .sessions
        .values()
        .map(summary_from_record)
        .collect())
}

pub(crate) fn inspect_registered_session(
    state: &ServerState,
    session_id: &str,
) -> Result<ServerSessionDetail> {
    let registry = load_session_registry(state)?;
    let record = registry
        .sessions
        .get(session_id)
        .ok_or_else(|| anyhow!("not_found: session `{session_id}` was not found"))?;
    detail_from_record(state, record)
}

fn detail_from_record(
    state: &ServerState,
    record: &OwnedSessionRecord,
) -> Result<ServerSessionDetail> {
    let snapshot = if record.persisted {
        Some(read_session_snapshot(
            &state.runtime_paths,
            &record.session_id,
        )?)
    } else {
        None
    };
    Ok(ServerSessionDetail {
        summary: summary_from_record(record),
        owner_device_name: record.owner_device_name.clone(),
        permission_mode: snapshot
            .as_ref()
            .and_then(|item| item.permission_mode.as_ref().map(ToString::to_string)),
        shell_name: snapshot.as_ref().map(|item| item.shell_name.clone()),
        system_prompt: snapshot.as_ref().map(|item| item.system_prompt.clone()),
        message_count: snapshot
            .as_ref()
            .map(|item| item.messages.len())
            .unwrap_or(0),
    })
}

fn summary_from_record(record: &OwnedSessionRecord) -> ServerSessionSummary {
    ServerSessionSummary {
        session_id: record.session_id.clone(),
        model: record.model.clone(),
        working_directory: record.working_directory.clone(),
        source: record.source.clone(),
        owner_account_id: record.owner_account_id.clone(),
        owner_device_id: record.owner_device_id.clone(),
        created_at: record.created_at,
        updated_at: record.updated_at,
        persisted: record.persisted,
    }
}

fn load_session_registry(state: &ServerState) -> Result<SessionRegistry> {
    read_json_if_exists(&state.data_paths.session_registry_path)
        .map(|value| value.unwrap_or_default())
}

fn save_session_registry(state: &ServerState, registry: &SessionRegistry) -> Result<()> {
    write_json(state.data_paths.session_registry_path.clone(), registry)
}
