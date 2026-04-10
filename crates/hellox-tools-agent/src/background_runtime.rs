use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use hellox_config::{sessions_root, PermissionMode};
use serde::Serialize;
use serde_json::{json, Value};

use crate::shared::{normalize_path, unix_timestamp};

#[derive(Debug, Clone, Serialize)]
pub struct AgentJobRecord {
    pub session_id: String,
    pub status: String,
    pub background: bool,
    pub resumed: bool,
    pub backend: String,
    pub pid: Option<u32>,
    pub pane_target: Option<String>,
    pub layout_slot: Option<String>,
    pub model: String,
    pub permission_mode: String,
    pub working_directory: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub iterations: Option<usize>,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackgroundAgentSessionMetadata {
    pub model: String,
    pub permission_mode: String,
    pub working_directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedAgentRuntime {
    pub status: String,
    pub background: bool,
    pub resumed: bool,
    pub backend: Option<String>,
    pub permission_mode: Option<String>,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub pid: Option<u32>,
    pub pane_target: Option<String>,
    pub layout_slot: Option<String>,
    pub iterations: Option<usize>,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedAgentSnapshot {
    pub model: String,
    pub permission_mode: Option<String>,
    pub working_directory: String,
    pub updated_at: u64,
    pub messages: usize,
    pub result_text: Option<String>,
    pub runtime: Option<PersistedAgentRuntime>,
}

pub trait BackgroundRuntimeContext {
    fn load_background_record(&self, session_id: &str) -> Result<Option<AgentJobRecord>>;
    fn load_persisted_agent_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<PersistedAgentSnapshot>>;
    fn save_persisted_runtime(
        &self,
        session_id: &str,
        runtime: PersistedAgentRuntime,
    ) -> Result<()>;
    fn update_session_permission_mode(
        &self,
        session_id: &str,
        permission_mode: &PermissionMode,
    ) -> Result<()>;
    fn update_background_record_permission_mode(
        &self,
        session_id: &str,
        permission_mode: &PermissionMode,
    ) -> Result<()>;
}

pub fn running_record(
    session: &BackgroundAgentSessionMetadata,
    session_id: &str,
    resumed: bool,
    background: bool,
    backend: &str,
    pid: Option<u32>,
    pane_target: Option<String>,
    layout_slot: Option<String>,
) -> AgentJobRecord {
    AgentJobRecord {
        session_id: session_id.to_string(),
        status: "running".to_string(),
        background,
        resumed,
        backend: backend.to_string(),
        pid,
        pane_target,
        layout_slot,
        model: session.model.clone(),
        permission_mode: session.permission_mode.clone(),
        working_directory: session.working_directory.clone(),
        started_at: unix_timestamp(),
        finished_at: None,
        iterations: None,
        result: None,
        error: None,
    }
}

pub fn completed_record(
    session: &BackgroundAgentSessionMetadata,
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
    AgentJobRecord {
        status: "completed".to_string(),
        finished_at: Some(unix_timestamp()),
        iterations: Some(iterations),
        result: Some(result),
        error: None,
        ..running_record(
            session,
            session_id,
            resumed,
            background,
            backend,
            pid,
            pane_target,
            layout_slot,
        )
    }
}

pub fn failed_record(
    session: &BackgroundAgentSessionMetadata,
    session_id: &str,
    resumed: bool,
    background: bool,
    backend: &str,
    pid: Option<u32>,
    pane_target: Option<String>,
    layout_slot: Option<String>,
    error: String,
) -> AgentJobRecord {
    AgentJobRecord {
        status: "failed".to_string(),
        finished_at: Some(unix_timestamp()),
        iterations: None,
        result: None,
        error: Some(error),
        ..running_record(
            session,
            session_id,
            resumed,
            background,
            backend,
            pid,
            pane_target,
            layout_slot,
        )
    }
}

pub fn cancelled_record(record: &AgentJobRecord, reason: Option<String>) -> AgentJobRecord {
    let mut updated = record.clone();
    updated.status = "cancelled".to_string();
    updated.finished_at = Some(unix_timestamp());
    updated.pid = None;
    updated.iterations = None;
    updated.result = None;
    updated.error = reason;
    updated
}

pub fn persisted_agent_statuses(
    context: &impl BackgroundRuntimeContext,
    working_directory: Option<&Path>,
    known_session_ids: &[String],
) -> Result<Vec<Value>> {
    persisted_agent_statuses_with_root(
        context,
        &sessions_root(),
        working_directory,
        known_session_ids,
    )
}

fn persisted_agent_statuses_with_root(
    context: &impl BackgroundRuntimeContext,
    root: &Path,
    working_directory: Option<&Path>,
    known_session_ids: &[String],
) -> Result<Vec<Value>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let known = known_session_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let working_directory = working_directory.map(normalize_path);
    let mut statuses = Vec::new();

    for entry in fs::read_dir(&root)
        .with_context(|| format!("failed to read session directory {}", root.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to inspect session directory {}", root.display()))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let Some(session_id) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if known.contains(session_id) {
            continue;
        }

        let Some(snapshot) = context.load_persisted_agent_snapshot(session_id)? else {
            continue;
        };
        if working_directory
            .as_ref()
            .is_some_and(|expected| &snapshot.working_directory != expected)
        {
            continue;
        }

        statuses.push(agent_status_value_from_parts(
            session_id,
            context.load_background_record(session_id)?,
            Some(snapshot),
        )?);
    }

    statuses.sort_by(|left, right| {
        right["updated_at"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&left["updated_at"].as_u64().unwrap_or(0))
            .then_with(|| {
                left["session_id"]
                    .as_str()
                    .unwrap_or_default()
                    .cmp(right["session_id"].as_str().unwrap_or_default())
            })
    });
    Ok(statuses)
}

pub fn persist_background_record(
    context: &impl BackgroundRuntimeContext,
    record: &AgentJobRecord,
) -> Result<()> {
    context.save_persisted_runtime(&record.session_id, persisted_runtime_from_record(record))
}

pub fn sync_session_permission_mode(
    context: &impl BackgroundRuntimeContext,
    session_id: &str,
    permission_mode: &PermissionMode,
) -> Result<Value> {
    context.update_session_permission_mode(session_id, permission_mode)?;
    context.update_background_record_permission_mode(session_id, permission_mode)?;
    Ok(json!({
        "session_id": session_id,
        "permission_mode": permission_mode.as_str(),
    }))
}

pub fn effective_background_record(
    context: &impl BackgroundRuntimeContext,
    session_id: &str,
) -> Result<Option<AgentJobRecord>> {
    let record = context.load_background_record(session_id)?;
    let snapshot = context.load_persisted_agent_snapshot(session_id)?;
    Ok(effective_background_record_from_parts(
        session_id, record, snapshot,
    ))
}

pub fn agent_status_value(
    context: &impl BackgroundRuntimeContext,
    session_id: &str,
) -> Result<Value> {
    agent_status_value_from_parts(
        session_id,
        context.load_background_record(session_id)?,
        context.load_persisted_agent_snapshot(session_id)?,
    )
}

pub fn is_running_session(
    context: &impl BackgroundRuntimeContext,
    session_id: &str,
) -> Result<bool> {
    Ok(effective_background_record(context, session_id)?
        .map(|record| record.status == "running")
        .unwrap_or(false))
}

pub fn persisted_runtime_from_record(record: &AgentJobRecord) -> PersistedAgentRuntime {
    PersistedAgentRuntime {
        status: record.status.clone(),
        background: record.background,
        resumed: record.resumed,
        backend: Some(record.backend.clone()),
        permission_mode: Some(record.permission_mode.clone()),
        started_at: Some(record.started_at),
        finished_at: record.finished_at,
        pid: record.pid,
        pane_target: record.pane_target.clone(),
        layout_slot: record.layout_slot.clone(),
        iterations: record.iterations,
        result: record.result.clone(),
        error: record.error.clone(),
    }
}

pub fn persisted_runtime_record(
    session_id: &str,
    snapshot: &PersistedAgentSnapshot,
) -> Option<AgentJobRecord> {
    let runtime = snapshot.runtime.as_ref()?;
    Some(AgentJobRecord {
        session_id: session_id.to_string(),
        status: runtime.status.clone(),
        background: runtime.background,
        resumed: runtime.resumed,
        backend: runtime
            .backend
            .clone()
            .unwrap_or_else(|| "persisted".to_string()),
        pid: runtime.pid,
        pane_target: runtime.pane_target.clone(),
        layout_slot: runtime.layout_slot.clone(),
        model: snapshot.model.clone(),
        permission_mode: runtime
            .permission_mode
            .clone()
            .or_else(|| snapshot.permission_mode.clone())
            .unwrap_or_else(|| "default".to_string()),
        working_directory: snapshot.working_directory.clone(),
        started_at: runtime.started_at.unwrap_or(snapshot.updated_at),
        finished_at: runtime.finished_at,
        iterations: runtime.iterations,
        result: runtime.result.clone(),
        error: runtime.error.clone(),
    })
}

fn effective_background_record_from_parts(
    session_id: &str,
    record: Option<AgentJobRecord>,
    snapshot: Option<PersistedAgentSnapshot>,
) -> Option<AgentJobRecord> {
    let persisted = snapshot
        .as_ref()
        .and_then(|snapshot| persisted_runtime_record(session_id, snapshot));

    match (record, persisted) {
        (Some(record), Some(persisted))
            if record.status == "running" && persisted.status != "running" =>
        {
            Some(persisted)
        }
        (Some(record), _) => Some(record),
        (None, Some(persisted)) => Some(persisted),
        (None, None) => None,
    }
}

fn agent_status_value_from_parts(
    session_id: &str,
    record: Option<AgentJobRecord>,
    snapshot: Option<PersistedAgentSnapshot>,
) -> Result<Value> {
    let effective =
        effective_background_record_from_parts(session_id, record.clone(), snapshot.clone());

    if effective.is_none() && snapshot.is_none() {
        anyhow::bail!("agent session `{session_id}` was not found");
    }

    let runtime = snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.runtime.as_ref());
    let status = effective
        .as_ref()
        .map(|item| item.status.clone())
        .or_else(|| runtime.map(|runtime| runtime.status.clone()))
        .unwrap_or_else(|| "persisted".to_string());

    Ok(json!({
        "session_id": session_id,
        "status": status,
        "background": effective
            .as_ref()
            .map(|item| item.background)
            .or_else(|| runtime.map(|runtime| runtime.background))
            .unwrap_or(false),
        "resumed": effective
            .as_ref()
            .map(|item| item.resumed)
            .or_else(|| runtime.map(|runtime| runtime.resumed))
            .unwrap_or(false),
        "backend": effective
            .as_ref()
            .map(|item| item.backend.clone())
            .or_else(|| runtime.and_then(|runtime| runtime.backend.clone()))
            .unwrap_or_else(|| "persisted".to_string()),
        "pid": effective
            .as_ref()
            .and_then(|item| item.pid)
            .or_else(|| runtime.and_then(|runtime| runtime.pid)),
        "pane_target": effective
            .as_ref()
            .and_then(|item| item.pane_target.clone())
            .or_else(|| runtime.and_then(|runtime| runtime.pane_target.clone())),
        "layout_slot": effective
            .as_ref()
            .and_then(|item| item.layout_slot.clone())
            .or_else(|| runtime.and_then(|runtime| runtime.layout_slot.clone())),
        "model": effective
            .as_ref()
            .map(|item| item.model.clone())
            .or_else(|| snapshot.as_ref().map(|snapshot| snapshot.model.clone())),
        "permission_mode": effective
            .as_ref()
            .map(|item| item.permission_mode.clone())
            .or_else(|| runtime.and_then(|runtime| runtime.permission_mode.clone()))
            .or_else(|| snapshot.as_ref().and_then(|snapshot| snapshot.permission_mode.clone())),
        "working_directory": effective
            .as_ref()
            .map(|item| item.working_directory.clone())
            .or_else(|| snapshot.as_ref().map(|snapshot| snapshot.working_directory.clone())),
        "started_at": effective
            .as_ref()
            .map(|item| item.started_at)
            .or_else(|| runtime.and_then(|runtime| runtime.started_at)),
        "finished_at": effective
            .as_ref()
            .and_then(|item| item.finished_at)
            .or_else(|| runtime.and_then(|runtime| runtime.finished_at)),
        "iterations": effective
            .as_ref()
            .and_then(|item| item.iterations)
            .or_else(|| runtime.and_then(|runtime| runtime.iterations)),
        "result": effective
            .as_ref()
            .and_then(|item| item.result.clone())
            .or_else(|| runtime.and_then(|runtime| runtime.result.clone()))
            .or_else(|| snapshot.as_ref().and_then(|snapshot| snapshot.result_text.clone())),
        "error": effective
            .as_ref()
            .and_then(|item| item.error.clone())
            .or_else(|| runtime.and_then(|runtime| runtime.error.clone())),
        "messages": snapshot.as_ref().map(|snapshot| snapshot.messages),
        "updated_at": snapshot.as_ref().map(|snapshot| snapshot.updated_at),
        "session_path": normalize_path(&hellox_config::session_file_path(session_id)),
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct TestContext {
        background: Arc<Mutex<BTreeMap<String, AgentJobRecord>>>,
        snapshots: Arc<Mutex<BTreeMap<String, PersistedAgentSnapshot>>>,
        permissions: Arc<Mutex<Vec<(String, String)>>>,
        background_permissions: Arc<Mutex<Vec<(String, String)>>>,
        saved_runtimes: Arc<Mutex<Vec<(String, PersistedAgentRuntime)>>>,
    }

    impl BackgroundRuntimeContext for TestContext {
        fn load_background_record(&self, session_id: &str) -> Result<Option<AgentJobRecord>> {
            Ok(self
                .background
                .lock()
                .expect("lock background")
                .get(session_id)
                .cloned())
        }

        fn load_persisted_agent_snapshot(
            &self,
            session_id: &str,
        ) -> Result<Option<PersistedAgentSnapshot>> {
            Ok(self
                .snapshots
                .lock()
                .expect("lock snapshots")
                .get(session_id)
                .cloned())
        }

        fn save_persisted_runtime(
            &self,
            session_id: &str,
            runtime: PersistedAgentRuntime,
        ) -> Result<()> {
            self.saved_runtimes
                .lock()
                .expect("lock saved runtimes")
                .push((session_id.to_string(), runtime));
            Ok(())
        }

        fn update_session_permission_mode(
            &self,
            session_id: &str,
            permission_mode: &PermissionMode,
        ) -> Result<()> {
            self.permissions
                .lock()
                .expect("lock permissions")
                .push((session_id.to_string(), permission_mode.as_str().to_string()));
            Ok(())
        }

        fn update_background_record_permission_mode(
            &self,
            session_id: &str,
            permission_mode: &PermissionMode,
        ) -> Result<()> {
            self.background_permissions
                .lock()
                .expect("lock background permissions")
                .push((session_id.to_string(), permission_mode.as_str().to_string()));
            Ok(())
        }
    }

    #[test]
    fn persisted_agent_statuses_filters_known_sessions_and_workspace() {
        let root = std::env::temp_dir().join(format!(
            "hellox-background-runtime-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create session root");
        let first = root.join("agent-a.json");
        let second = root.join("agent-b.json");
        std::fs::write(&first, "{}").expect("write agent-a");
        std::fs::write(&second, "{}").expect("write agent-b");

        let context = TestContext {
            snapshots: Arc::new(Mutex::new(BTreeMap::from([
                (
                    "agent-a".to_string(),
                    PersistedAgentSnapshot {
                        model: "mock".to_string(),
                        permission_mode: Some("default".to_string()),
                        working_directory: "D:/repo".to_string(),
                        updated_at: 10,
                        messages: 3,
                        result_text: None,
                        runtime: None,
                    },
                ),
                (
                    "agent-b".to_string(),
                    PersistedAgentSnapshot {
                        model: "mock".to_string(),
                        permission_mode: Some("default".to_string()),
                        working_directory: "D:/other".to_string(),
                        updated_at: 1,
                        messages: 0,
                        result_text: None,
                        runtime: None,
                    },
                ),
            ]))),
            ..TestContext::default()
        };

        let statuses = persisted_agent_statuses_with_root(
            &context,
            &root,
            Some(Path::new("D:/repo")),
            &["agent-b".to_string()],
        )
        .expect("persisted statuses");

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0]["session_id"].as_str(), Some("agent-a"));
        assert_eq!(statuses[0]["status"].as_str(), Some("persisted"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_session_permission_mode_updates_both_layers() {
        let context = TestContext::default();
        let result =
            sync_session_permission_mode(&context, "agent-a", &PermissionMode::AcceptEdits)
                .expect("sync permission mode");

        assert_eq!(result["permission_mode"].as_str(), Some("accept_edits"));
        assert_eq!(
            context
                .permissions
                .lock()
                .expect("lock permissions")
                .as_slice(),
            [("agent-a".to_string(), "accept_edits".to_string())]
        );
        assert_eq!(
            context
                .background_permissions
                .lock()
                .expect("lock background permissions")
                .as_slice(),
            [("agent-a".to_string(), "accept_edits".to_string())]
        );
    }

    #[test]
    fn effective_background_record_prefers_terminal_persisted_over_running_background() {
        let context = TestContext {
            background: Arc::new(Mutex::new(BTreeMap::from([(
                "agent-a".to_string(),
                AgentJobRecord {
                    session_id: "agent-a".to_string(),
                    status: "running".to_string(),
                    background: true,
                    resumed: false,
                    backend: "detached_process".to_string(),
                    pid: Some(42),
                    pane_target: None,
                    layout_slot: None,
                    model: "mock".to_string(),
                    permission_mode: "default".to_string(),
                    working_directory: "D:/repo".to_string(),
                    started_at: 1,
                    finished_at: None,
                    iterations: None,
                    result: None,
                    error: None,
                },
            )]))),
            snapshots: Arc::new(Mutex::new(BTreeMap::from([(
                "agent-a".to_string(),
                PersistedAgentSnapshot {
                    model: "mock".to_string(),
                    permission_mode: Some("default".to_string()),
                    working_directory: "D:/repo".to_string(),
                    updated_at: 10,
                    messages: 2,
                    result_text: Some("done".to_string()),
                    runtime: Some(PersistedAgentRuntime {
                        status: "completed".to_string(),
                        background: true,
                        resumed: false,
                        backend: Some("detached_process".to_string()),
                        permission_mode: Some("default".to_string()),
                        started_at: Some(1),
                        finished_at: Some(10),
                        pid: None,
                        pane_target: None,
                        layout_slot: None,
                        iterations: Some(4),
                        result: Some("done".to_string()),
                        error: None,
                    }),
                },
            )]))),
            ..TestContext::default()
        };

        let record = effective_background_record(&context, "agent-a")
            .expect("effective record")
            .expect("record exists");
        assert_eq!(record.status, "completed");
        assert_eq!(record.iterations, Some(4));
    }

    #[test]
    fn persist_background_record_saves_runtime_projection() {
        let context = TestContext::default();
        let record = AgentJobRecord {
            session_id: "agent-a".to_string(),
            status: "completed".to_string(),
            background: true,
            resumed: false,
            backend: "detached_process".to_string(),
            pid: Some(7),
            pane_target: None,
            layout_slot: Some("primary".to_string()),
            model: "mock".to_string(),
            permission_mode: "accept_edits".to_string(),
            working_directory: "D:/repo".to_string(),
            started_at: 1,
            finished_at: Some(2),
            iterations: Some(3),
            result: Some("done".to_string()),
            error: None,
        };

        persist_background_record(&context, &record).expect("persist background record");

        let saved = context.saved_runtimes.lock().expect("lock saved runtimes");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0, "agent-a");
        assert_eq!(saved[0].1.permission_mode.as_deref(), Some("accept_edits"));
        assert_eq!(saved[0].1.layout_slot.as_deref(), Some("primary"));
    }
}
