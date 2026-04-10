use anyhow::anyhow;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use serde_json::{json, Value};

use hellox_sync::{
    compute_document_etag, RemoteDocument, SettingsSyncSnapshot, TeamMemorySnapshot,
};
#[cfg(test)]
use hellox_sync::{ManagedSettingsDocument, PolicyLimitsDocument};

use crate::state::{
    inspect_managed_settings, inspect_policy_limits, list_owned_sessions, load_owned_session,
    load_settings_snapshot, save_settings_snapshot, sync_team_memory, validate_remote_access,
    ServerState,
};
#[cfg(test)]
use crate::state::{
    managed_settings_path, policy_limits_path, write_managed_settings, write_policy_limits,
};
use crate::types::{
    DirectConnectConfig, DirectConnectRequest, ServerSessionDetail, ServerSessionSummary,
    ServerStatus,
};

pub(crate) fn router(state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{session_id}", get(show_session))
        .route(
            "/sync/settings",
            get(get_settings_snapshot).put(put_settings_snapshot),
        )
        .route("/sync/team-memory/{repo_id}", put(put_team_memory_snapshot))
        .route("/managed-settings", get(get_managed_settings))
        .route("/policy-limits", get(get_policy_limits))
        .with_state(state)
}

pub(crate) async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub(crate) async fn status(State(state): State<ServerState>) -> Json<ServerStatus> {
    Json(crate::state::build_server_status(
        &state.config,
        &state.runtime_paths,
    ))
}

async fn list_sessions(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerSessionSummary>>, ServerHttpError> {
    let identity =
        validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    Ok(Json(
        list_owned_sessions(&state, &identity).map_err(ServerHttpError::from_anyhow)?,
    ))
}

async fn show_session(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(session_id): AxumPath<String>,
) -> Result<Json<ServerSessionDetail>, ServerHttpError> {
    let identity =
        validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    Ok(Json(
        load_owned_session(&state, &identity, &session_id).map_err(ServerHttpError::from_anyhow)?,
    ))
}

async fn create_session(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<DirectConnectRequest>,
) -> Result<Json<DirectConnectConfig>, ServerHttpError> {
    let identity =
        validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    Ok(Json(
        crate::state::create_owned_session(&state, &identity, request)
            .map_err(ServerHttpError::from_anyhow)?,
    ))
}

async fn get_settings_snapshot(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<SettingsSyncSnapshot>, ServerHttpError> {
    let identity =
        validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    let Some(snapshot) =
        load_settings_snapshot(&state, &identity).map_err(ServerHttpError::from_anyhow)?
    else {
        return Err(ServerHttpError::not_found(
            "settings snapshot was not found",
        ));
    };
    Ok(Json(snapshot))
}

async fn put_settings_snapshot(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(snapshot): Json<SettingsSyncSnapshot>,
) -> Result<Json<SettingsSyncSnapshot>, ServerHttpError> {
    let identity =
        validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    Ok(Json(
        save_settings_snapshot(&state, &identity, &snapshot)
            .map_err(ServerHttpError::from_anyhow)?,
    ))
}

async fn put_team_memory_snapshot(
    State(state): State<ServerState>,
    headers: HeaderMap,
    AxumPath(repo_id): AxumPath<String>,
    Json(snapshot): Json<TeamMemorySnapshot>,
) -> Result<Json<TeamMemorySnapshot>, ServerHttpError> {
    let identity =
        validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    Ok(Json(
        sync_team_memory(&state, &identity, &repo_id, snapshot)
            .map_err(ServerHttpError::from_anyhow)?,
    ))
}

async fn get_managed_settings(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Response, ServerHttpError> {
    validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    get_document_response(
        inspect_managed_settings(&state).map_err(ServerHttpError::from_anyhow)?,
        &headers,
    )
}

async fn get_policy_limits(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Response, ServerHttpError> {
    validate_remote_access(&state, &headers).map_err(ServerHttpError::from_anyhow)?;
    get_document_response(
        inspect_policy_limits(&state).map_err(ServerHttpError::from_anyhow)?,
        &headers,
    )
}

fn get_document_response<T>(
    value: Option<T>,
    headers: &HeaderMap,
) -> Result<Response, ServerHttpError>
where
    T: for<'de> serde::Deserialize<'de> + serde::Serialize,
{
    let Some(value) = value else {
        return Err(ServerHttpError::not_found("remote document was not found"));
    };
    let document = RemoteDocument {
        etag: Some(compute_document_etag(&value).map_err(ServerHttpError::from_anyhow)?),
        value,
    };
    let requested_etag = headers
        .get("if-none-match")
        .and_then(|value| value.to_str().ok())
        .map(str::trim);
    if requested_etag == document.etag.as_deref() {
        return Ok((StatusCode::NOT_MODIFIED, "").into_response());
    }

    let mut response = Json(document.value).into_response();
    if let Some(etag) = document.etag {
        let etag_header = HeaderValue::from_str(&etag)
            .map_err(|error| ServerHttpError::internal(anyhow!("invalid etag header: {error}")))?;
        response.headers_mut().insert("etag", etag_header);
    }
    Ok(response)
}

#[cfg(test)]
pub(crate) fn persist_managed_settings(
    state: &ServerState,
    document: &ManagedSettingsDocument,
) -> Result<(), ServerHttpError> {
    write_managed_settings(managed_settings_path(state), document)
        .map_err(ServerHttpError::from_anyhow)
}

#[cfg(test)]
pub(crate) fn persist_policy_limits(
    state: &ServerState,
    document: &PolicyLimitsDocument,
) -> Result<(), ServerHttpError> {
    write_policy_limits(policy_limits_path(state), document).map_err(ServerHttpError::from_anyhow)
}

#[derive(Debug)]
pub(crate) struct ServerHttpError {
    status: StatusCode,
    error_type: &'static str,
    error: anyhow::Error,
}

impl ServerHttpError {
    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error_type: "not_found",
            error: anyhow!(message.to_string()),
        }
    }

    fn internal(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_type: "server_error",
            error,
        }
    }

    fn from_anyhow(error: anyhow::Error) -> Self {
        let message = error.to_string();
        if let Some(message) = message.strip_prefix("unauthorized: ") {
            return Self {
                status: StatusCode::UNAUTHORIZED,
                error_type: "unauthorized",
                error: anyhow!(message.to_string()),
            };
        }
        if let Some(message) = message.strip_prefix("forbidden: ") {
            return Self {
                status: StatusCode::FORBIDDEN,
                error_type: "forbidden",
                error: anyhow!(message.to_string()),
            };
        }
        if let Some(message) = message.strip_prefix("not_found: ") {
            return Self {
                status: StatusCode::NOT_FOUND,
                error_type: "not_found",
                error: anyhow!(message.to_string()),
            };
        }
        Self::internal(error)
    }
}

impl IntoResponse for ServerHttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": {
                    "type": self.error_type,
                    "message": self.error.to_string()
                }
            })),
        )
            .into_response()
    }
}
