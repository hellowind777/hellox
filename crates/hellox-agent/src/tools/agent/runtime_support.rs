use std::path::Path;

use anyhow::Result;
use hellox_config::PermissionMode;
use serde_json::Value;

use super::background::BackgroundRuntimeBridge;

pub(super) fn persisted_agent_statuses(
    working_directory: Option<&Path>,
    known_session_ids: &[String],
) -> Result<Vec<Value>> {
    hellox_tools_agent::background_runtime::persisted_agent_statuses(
        &BackgroundRuntimeBridge,
        working_directory,
        known_session_ids,
    )
}

pub(super) fn sync_session_permission_mode(
    session_id: &str,
    permission_mode: &PermissionMode,
) -> Result<Value> {
    hellox_tools_agent::background_runtime::sync_session_permission_mode(
        &BackgroundRuntimeBridge,
        session_id,
        permission_mode,
    )
}
