use anyhow::Result;
use hellox_config::PermissionMode;
use serde_json::Value;

use crate::StoredSession;

use super::super::ToolExecutionContext;
use super::background::{
    load_background_record, persisted_runtime_to_stored, store_background_record,
    stored_runtime_to_persisted,
};
use super::runtime_support::sync_session_permission_mode;
use super::team_storage::TeamMemberRecord;

impl hellox_tools_agent::team_registry_runtime::TeamMemberRuntimePersistenceContext
    for ToolExecutionContext
{
    fn sync_session_permission_mode(
        &self,
        session_id: &str,
        permission_mode: &PermissionMode,
    ) -> Result<Value> {
        sync_session_permission_mode(session_id, permission_mode)
    }

    fn persist_member_runtime_metadata(
        &self,
        member: &TeamMemberRecord,
        clear_missing_pane_targets: bool,
    ) -> Result<()> {
        persist_member_runtime_metadata(member, clear_missing_pane_targets)
    }
}

pub(super) fn sync_member_permissions(
    context: &ToolExecutionContext,
    members: &[TeamMemberRecord],
    permission_mode: Option<&PermissionMode>,
) -> Result<Vec<Value>> {
    hellox_tools_agent::team_registry_runtime::sync_member_permissions(
        context,
        members,
        permission_mode,
    )
}

pub(super) fn sync_member_runtime_metadata(
    context: &ToolExecutionContext,
    members: &[TeamMemberRecord],
) -> Result<()> {
    hellox_tools_agent::team_registry_runtime::sync_member_runtime_metadata(context, members)
}

pub(super) fn reconcile_member_runtime_metadata(
    context: &ToolExecutionContext,
    members: &[TeamMemberRecord],
) -> Result<()> {
    hellox_tools_agent::team_registry_runtime::reconcile_member_runtime_metadata(context, members)
}

pub(super) fn parse_member_permission_mode(
    value: Option<&str>,
    default_permission_mode: Option<&PermissionMode>,
) -> Result<Option<PermissionMode>> {
    hellox_tools_agent::team_registry_runtime::parse_member_permission_mode(
        value,
        default_permission_mode,
    )
}

fn persist_member_runtime_metadata(
    member: &TeamMemberRecord,
    clear_missing_pane_targets: bool,
) -> Result<()> {
    let mut stored = StoredSession::load(&member.session_id)?;
    let runtime = hellox_tools_agent::team_registry_runtime::merged_member_runtime(
        stored
            .snapshot
            .agent_runtime
            .clone()
            .map(stored_runtime_to_persisted),
        stored
            .snapshot
            .permission_mode
            .as_ref()
            .map(|mode| mode.as_str().to_string()),
        member,
        clear_missing_pane_targets,
    );

    stored.save_runtime(persisted_runtime_to_stored(runtime))?;

    if let Some(record) = load_background_record(&member.session_id)? {
        let updated = hellox_tools_agent::team_registry_runtime::merged_member_background_record(
            &record,
            member,
            clear_missing_pane_targets,
        );
        store_background_record(updated)?;
    }

    Ok(())
}
