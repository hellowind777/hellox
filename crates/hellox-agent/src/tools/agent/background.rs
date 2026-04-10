use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use anyhow::{anyhow, Result};
use hellox_config::PermissionMode;
use hellox_gateway_api::extract_text;
use serde_json::Value;
use tokio::task::AbortHandle;

use crate::storage::StoredAgentRuntime;
use crate::{AgentSession, StoredSession, StoredSessionSnapshot};

use super::shared::normalize_path;

static BACKGROUND_AGENTS: LazyLock<Mutex<BTreeMap<String, AgentJobRecord>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static BACKGROUND_ABORTS: LazyLock<Mutex<BTreeMap<String, AbortHandle>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
pub(super) use hellox_tools_agent::background_runtime::AgentJobRecord;
pub(super) struct BackgroundRuntimeBridge;

impl hellox_tools_agent::background_runtime::BackgroundRuntimeContext for BackgroundRuntimeBridge {
    fn load_background_record(&self, session_id: &str) -> Result<Option<AgentJobRecord>> {
        load_background_record_from_registry(session_id)
    }

    fn load_persisted_agent_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<hellox_tools_agent::background_runtime::PersistedAgentSnapshot>> {
        load_persisted_agent_snapshot_from_store(session_id)
    }

    fn save_persisted_runtime(
        &self,
        session_id: &str,
        runtime: hellox_tools_agent::background_runtime::PersistedAgentRuntime,
    ) -> Result<()> {
        let mut stored = StoredSession::load(session_id)?;
        stored.save_runtime(persisted_runtime_to_stored(runtime))
    }

    fn update_session_permission_mode(
        &self,
        session_id: &str,
        permission_mode: &PermissionMode,
    ) -> Result<()> {
        let mut stored = StoredSession::load(session_id)?;
        stored.snapshot.permission_mode = Some(permission_mode.clone());
        if let Some(runtime) = stored.snapshot.agent_runtime.as_mut() {
            runtime.permission_mode = Some(permission_mode.clone());
        }
        let messages = stored.restore_messages();
        stored.save(&messages)
    }

    fn update_background_record_permission_mode(
        &self,
        session_id: &str,
        permission_mode: &PermissionMode,
    ) -> Result<()> {
        let Some(mut record) = load_background_record_from_registry(session_id)? else {
            return Ok(());
        };
        record.permission_mode = permission_mode.as_str().to_string();
        persist_background_record_to_store(&record)?;
        store_background_record_in_registry(record)
    }
}

pub(super) fn running_record(
    session: &AgentSession,
    session_id: &str,
    resumed: bool,
    background: bool,
    backend: &str,
    pid: Option<u32>,
    pane_target: Option<String>,
    layout_slot: Option<String>,
) -> AgentJobRecord {
    hellox_tools_agent::background_runtime::running_record(
        &session_metadata(session),
        session_id,
        resumed,
        background,
        backend,
        pid,
        pane_target,
        layout_slot,
    )
}

pub(super) fn completed_record(
    session: &AgentSession,
    session_id: &str,
    resumed: bool,
    background: bool,
    backend: &str,
    pid: Option<u32>,
    pane_target: Option<String>,
    layout_slot: Option<String>,
    iterations: usize,
    result: String,
) -> AgentJobRecord {
    hellox_tools_agent::background_runtime::completed_record(
        &session_metadata(session),
        session_id,
        resumed,
        background,
        backend,
        pid,
        pane_target,
        layout_slot,
        iterations,
        result,
    )
}

pub(super) fn failed_record(
    session: &AgentSession,
    session_id: &str,
    resumed: bool,
    background: bool,
    backend: &str,
    pid: Option<u32>,
    pane_target: Option<String>,
    layout_slot: Option<String>,
    error: String,
) -> AgentJobRecord {
    hellox_tools_agent::background_runtime::failed_record(
        &session_metadata(session),
        session_id,
        resumed,
        background,
        backend,
        pid,
        pane_target,
        layout_slot,
        error,
    )
}

pub(super) fn cancelled_record(record: &AgentJobRecord, reason: Option<String>) -> AgentJobRecord {
    hellox_tools_agent::background_runtime::cancelled_record(record, reason)
}

pub(super) fn load_background_record(session_id: &str) -> Result<Option<AgentJobRecord>> {
    load_background_record_from_registry(session_id)
}

fn load_background_record_from_registry(session_id: &str) -> Result<Option<AgentJobRecord>> {
    BACKGROUND_AGENTS
        .lock()
        .map(|records| records.get(session_id).cloned())
        .map_err(|_| anyhow!("background agent registry lock was poisoned"))
}

pub(super) fn store_background_record(record: AgentJobRecord) -> Result<()> {
    hellox_tools_agent::background_runtime::persist_background_record(
        &BackgroundRuntimeBridge,
        &record,
    )?;
    store_background_record_in_registry(record)
}

fn store_background_record_in_registry(record: AgentJobRecord) -> Result<()> {
    BACKGROUND_AGENTS
        .lock()
        .map(|mut records| {
            records.insert(record.session_id.clone(), record);
        })
        .map_err(|_| anyhow!("background agent registry lock was poisoned"))
}

pub(super) fn list_background_records() -> Result<Vec<AgentJobRecord>> {
    BACKGROUND_AGENTS
        .lock()
        .map(|records| {
            let mut items = records.values().cloned().collect::<Vec<_>>();
            items.sort_by(|left, right| {
                right
                    .started_at
                    .cmp(&left.started_at)
                    .then_with(|| left.session_id.cmp(&right.session_id))
            });
            items
        })
        .map_err(|_| anyhow!("background agent registry lock was poisoned"))
}

pub(super) fn register_abort_handle(session_id: &str, handle: AbortHandle) -> Result<()> {
    BACKGROUND_ABORTS
        .lock()
        .map(|mut handles| {
            handles.insert(session_id.to_string(), handle);
        })
        .map_err(|_| anyhow!("background agent abort registry lock was poisoned"))
}

pub(super) fn take_abort_handle(session_id: &str) -> Result<Option<AbortHandle>> {
    BACKGROUND_ABORTS
        .lock()
        .map(|mut handles| handles.remove(session_id))
        .map_err(|_| anyhow!("background agent abort registry lock was poisoned"))
}

pub(super) fn clear_abort_handle(session_id: &str) -> Result<()> {
    let _ = take_abort_handle(session_id)?;
    Ok(())
}

#[cfg(test)]
pub(super) fn reset_background_state() -> Result<()> {
    BACKGROUND_AGENTS
        .lock()
        .map(|mut records| records.clear())
        .map_err(|_| anyhow!("background agent registry lock was poisoned"))?;
    BACKGROUND_ABORTS
        .lock()
        .map(|mut handles| handles.clear())
        .map_err(|_| anyhow!("background agent abort registry lock was poisoned"))?;
    Ok(())
}

pub(super) fn agent_status_value(session_id: &str) -> Result<Value> {
    hellox_tools_agent::background_runtime::agent_status_value(&BackgroundRuntimeBridge, session_id)
}

pub(super) fn effective_background_record(session_id: &str) -> Result<Option<AgentJobRecord>> {
    hellox_tools_agent::background_runtime::effective_background_record(
        &BackgroundRuntimeBridge,
        session_id,
    )
}

pub(super) fn is_running_session(session_id: &str) -> Result<bool> {
    hellox_tools_agent::background_runtime::is_running_session(&BackgroundRuntimeBridge, session_id)
}

fn session_metadata(
    session: &AgentSession,
) -> hellox_tools_agent::background_runtime::BackgroundAgentSessionMetadata {
    hellox_tools_agent::background_runtime::BackgroundAgentSessionMetadata {
        model: session.model().to_string(),
        permission_mode: session.permission_mode().as_str().to_string(),
        working_directory: normalize_path(session.working_directory()),
    }
}

fn snapshot_result_text(snapshot: &StoredSessionSnapshot) -> Option<String> {
    snapshot
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .and_then(|message| {
            let text = extract_text(&message.content);
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        })
}

fn load_persisted_agent_snapshot_from_store(
    session_id: &str,
) -> Result<Option<hellox_tools_agent::background_runtime::PersistedAgentSnapshot>> {
    Ok(match StoredSession::load(session_id) {
        Ok(stored) => Some(stored_snapshot_to_persisted(stored.snapshot)),
        Err(_) => None,
    })
}

pub(super) fn stored_runtime_to_persisted(
    runtime: StoredAgentRuntime,
) -> hellox_tools_agent::background_runtime::PersistedAgentRuntime {
    hellox_tools_agent::background_runtime::PersistedAgentRuntime {
        status: runtime.status,
        background: runtime.background,
        resumed: runtime.resumed,
        backend: runtime.backend,
        permission_mode: runtime
            .permission_mode
            .map(|mode| mode.as_str().to_string()),
        started_at: runtime.started_at,
        finished_at: runtime.finished_at,
        pid: runtime.pid,
        pane_target: runtime.pane_target,
        layout_slot: runtime.layout_slot,
        iterations: runtime.iterations,
        result: runtime.result,
        error: runtime.error,
    }
}

pub(super) fn persisted_runtime_to_stored(
    runtime: hellox_tools_agent::background_runtime::PersistedAgentRuntime,
) -> StoredAgentRuntime {
    StoredAgentRuntime {
        status: runtime.status,
        background: runtime.background,
        resumed: runtime.resumed,
        backend: runtime.backend,
        permission_mode: runtime.permission_mode.and_then(|mode| mode.parse().ok()),
        started_at: runtime.started_at,
        finished_at: runtime.finished_at,
        pid: runtime.pid,
        pane_target: runtime.pane_target,
        layout_slot: runtime.layout_slot,
        iterations: runtime.iterations,
        result: runtime.result,
        error: runtime.error,
    }
}

fn stored_snapshot_to_persisted(
    snapshot: StoredSessionSnapshot,
) -> hellox_tools_agent::background_runtime::PersistedAgentSnapshot {
    let result_text = snapshot_result_text(&snapshot);
    hellox_tools_agent::background_runtime::PersistedAgentSnapshot {
        model: snapshot.model,
        permission_mode: snapshot
            .permission_mode
            .map(|mode| mode.as_str().to_string()),
        working_directory: normalize_path(Path::new(&snapshot.working_directory)),
        updated_at: snapshot.updated_at,
        messages: snapshot.messages.len(),
        result_text,
        runtime: snapshot.agent_runtime.map(stored_runtime_to_persisted),
    }
}

fn persist_background_record_to_store(record: &AgentJobRecord) -> Result<()> {
    let mut stored = StoredSession::load(&record.session_id)?;
    stored.save_runtime(persisted_runtime_to_stored(
        hellox_tools_agent::background_runtime::persisted_runtime_from_record(record),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_record_marks_terminal_state() {
        let record = AgentJobRecord {
            session_id: "agent-1".to_string(),
            status: "running".to_string(),
            background: true,
            resumed: false,
            backend: "in_process".to_string(),
            pid: None,
            pane_target: None,
            layout_slot: None,
            model: "mock".to_string(),
            permission_mode: "default".to_string(),
            working_directory: "D:/repo".to_string(),
            started_at: 1,
            finished_at: None,
            iterations: Some(2),
            result: Some("done".to_string()),
            error: None,
        };

        let cancelled = cancelled_record(&record, Some("stopped".to_string()));
        assert_eq!(cancelled.status, "cancelled");
        assert!(cancelled.finished_at.is_some());
        assert_eq!(cancelled.error.as_deref(), Some("stopped"));
    }
}
