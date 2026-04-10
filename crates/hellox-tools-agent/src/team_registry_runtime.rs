use anyhow::{anyhow, Result};
use hellox_config::PermissionMode;
use serde_json::Value;

use crate::background_runtime::{AgentJobRecord, PersistedAgentRuntime};
use crate::team_storage::TeamMemberRecord;

pub trait TeamMemberRuntimePersistenceContext {
    fn sync_session_permission_mode(
        &self,
        session_id: &str,
        permission_mode: &PermissionMode,
    ) -> Result<Value>;

    fn persist_member_runtime_metadata(
        &self,
        member: &TeamMemberRecord,
        clear_missing_pane_targets: bool,
    ) -> Result<()>;
}

pub fn sync_member_permissions(
    context: &impl TeamMemberRuntimePersistenceContext,
    members: &[TeamMemberRecord],
    permission_mode: Option<&PermissionMode>,
) -> Result<Vec<Value>> {
    let Some(permission_mode) = permission_mode else {
        return Ok(Vec::new());
    };

    members
        .iter()
        .map(|member| context.sync_session_permission_mode(&member.session_id, permission_mode))
        .collect()
}

pub fn sync_member_runtime_metadata(
    context: &impl TeamMemberRuntimePersistenceContext,
    members: &[TeamMemberRecord],
) -> Result<()> {
    sync_member_runtime_metadata_with_policy(context, members, false)
}

pub fn reconcile_member_runtime_metadata(
    context: &impl TeamMemberRuntimePersistenceContext,
    members: &[TeamMemberRecord],
) -> Result<()> {
    sync_member_runtime_metadata_with_policy(context, members, true)
}

fn sync_member_runtime_metadata_with_policy(
    context: &impl TeamMemberRuntimePersistenceContext,
    members: &[TeamMemberRecord],
    clear_missing_pane_targets: bool,
) -> Result<()> {
    for member in members {
        context.persist_member_runtime_metadata(member, clear_missing_pane_targets)?;
    }
    Ok(())
}

pub fn parse_member_permission_mode(
    value: Option<&str>,
    default_permission_mode: Option<&PermissionMode>,
) -> Result<Option<PermissionMode>> {
    match value {
        Some(value) => value
            .parse::<PermissionMode>()
            .map(Some)
            .map_err(|error| anyhow!(error)),
        None => Ok(default_permission_mode.cloned()),
    }
}

pub fn merged_member_runtime(
    current: Option<PersistedAgentRuntime>,
    default_permission_mode: Option<String>,
    member: &TeamMemberRecord,
    clear_missing_pane_targets: bool,
) -> PersistedAgentRuntime {
    let mut runtime = current.unwrap_or_else(|| PersistedAgentRuntime {
        status: "persisted".to_string(),
        background: false,
        resumed: true,
        backend: None,
        permission_mode: default_permission_mode,
        started_at: None,
        finished_at: None,
        pid: None,
        pane_target: None,
        layout_slot: None,
        iterations: None,
        result: None,
        error: None,
    });

    if member.backend.is_some() {
        runtime.backend = member.backend.clone();
    }
    if member.layout_slot.is_some() {
        runtime.layout_slot = member.layout_slot.clone();
    }
    if clear_missing_pane_targets || member.pane_target.is_some() || runtime.pane_target.is_none() {
        runtime.pane_target = member.pane_target.clone();
    }

    runtime
}

pub fn merged_member_background_record(
    current: &AgentJobRecord,
    member: &TeamMemberRecord,
    clear_missing_pane_targets: bool,
) -> AgentJobRecord {
    let mut updated = current.clone();
    if member.backend.is_some() {
        updated.backend = member.backend.clone().expect("checked above");
    }
    if member.layout_slot.is_some() {
        updated.layout_slot = member.layout_slot.clone();
    }
    if clear_missing_pane_targets || member.pane_target.is_some() || updated.pane_target.is_none() {
        updated.pane_target = member.pane_target.clone();
    }
    updated
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;

    #[derive(Clone, Default)]
    struct TestContext {
        permissions: Arc<Mutex<Vec<(String, String)>>>,
        runtime_updates: Arc<Mutex<Vec<(String, bool)>>>,
    }

    impl TeamMemberRuntimePersistenceContext for TestContext {
        fn sync_session_permission_mode(
            &self,
            session_id: &str,
            permission_mode: &PermissionMode,
        ) -> Result<Value> {
            self.permissions
                .lock()
                .expect("lock permissions")
                .push((session_id.to_string(), permission_mode.as_str().to_string()));
            Ok(json!({
                "session_id": session_id,
                "permission_mode": permission_mode.as_str(),
            }))
        }

        fn persist_member_runtime_metadata(
            &self,
            member: &TeamMemberRecord,
            clear_missing_pane_targets: bool,
        ) -> Result<()> {
            self.runtime_updates
                .lock()
                .expect("lock runtime updates")
                .push((member.session_id.clone(), clear_missing_pane_targets));
            Ok(())
        }
    }

    fn sample_members() -> Vec<TeamMemberRecord> {
        vec![TeamMemberRecord {
            name: "alice".to_string(),
            session_id: "session-a".to_string(),
            backend: Some("in_process".to_string()),
            layout_slot: Some("primary".to_string()),
            pane_target: None,
        }]
    }

    #[test]
    fn sync_member_permissions_skips_when_permission_mode_is_missing() {
        let context = TestContext::default();
        let results =
            sync_member_permissions(&context, &sample_members(), None).expect("sync permissions");
        assert!(results.is_empty());
        assert!(context
            .permissions
            .lock()
            .expect("lock permissions")
            .is_empty());
    }

    #[test]
    fn reconcile_member_runtime_metadata_marks_clear_policy() {
        let context = TestContext::default();
        reconcile_member_runtime_metadata(&context, &sample_members())
            .expect("reconcile runtime metadata");
        assert_eq!(
            context
                .runtime_updates
                .lock()
                .expect("lock runtime updates")
                .as_slice(),
            [("session-a".to_string(), true)]
        );
    }

    #[test]
    fn merged_member_runtime_preserves_existing_runtime_defaults() {
        let merged = merged_member_runtime(
            Some(PersistedAgentRuntime {
                status: "running".to_string(),
                background: true,
                resumed: false,
                backend: Some("detached_process".to_string()),
                permission_mode: Some("accept_edits".to_string()),
                started_at: Some(1),
                finished_at: None,
                pid: Some(7),
                pane_target: Some("%1".to_string()),
                layout_slot: Some("primary".to_string()),
                iterations: None,
                result: None,
                error: None,
            }),
            Some("default".to_string()),
            &TeamMemberRecord {
                name: "alice".to_string(),
                session_id: "session-a".to_string(),
                backend: Some("tmux_pane".to_string()),
                layout_slot: Some("right".to_string()),
                pane_target: None,
            },
            true,
        );

        assert_eq!(merged.status, "running");
        assert_eq!(merged.backend.as_deref(), Some("tmux_pane"));
        assert_eq!(merged.layout_slot.as_deref(), Some("right"));
        assert!(merged.pane_target.is_none());
        assert_eq!(merged.permission_mode.as_deref(), Some("accept_edits"));
    }

    #[test]
    fn merged_member_background_record_updates_runtime_metadata() {
        let updated = merged_member_background_record(
            &AgentJobRecord {
                session_id: "session-a".to_string(),
                status: "running".to_string(),
                background: true,
                resumed: false,
                backend: "detached_process".to_string(),
                pid: Some(7),
                pane_target: Some("%1".to_string()),
                layout_slot: Some("primary".to_string()),
                model: "mock".to_string(),
                permission_mode: "default".to_string(),
                working_directory: "D:/repo".to_string(),
                started_at: 1,
                finished_at: None,
                iterations: None,
                result: None,
                error: None,
            },
            &TeamMemberRecord {
                name: "alice".to_string(),
                session_id: "session-a".to_string(),
                backend: Some("tmux_pane".to_string()),
                layout_slot: Some("right".to_string()),
                pane_target: None,
            },
            true,
        );

        assert_eq!(updated.backend, "tmux_pane");
        assert_eq!(updated.layout_slot.as_deref(), Some("right"));
        assert!(updated.pane_target.is_none());
    }
}
