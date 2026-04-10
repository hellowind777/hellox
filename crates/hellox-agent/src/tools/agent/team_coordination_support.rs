use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::StoredSession;

use super::super::ToolExecutionContext;
use super::background::effective_background_record;
use super::team_registry_support::reconcile_member_runtime_metadata;
use super::team_storage::{TeamMemberRecord, TeamRecord};

use hellox_tools_agent::team_coordination_runtime::{
    ResolvedTeamSelection, TeamRuntimePersistenceContext,
};
use hellox_tools_agent::team_runtime_reconciliation::{
    TeamMemberRuntimeSnapshot, TeamRuntimeSnapshotProvider,
};

#[async_trait]
impl TeamRuntimePersistenceContext for ToolExecutionContext {
    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    async fn ensure_write_allowed(&self, path: &std::path::Path) -> Result<()> {
        ToolExecutionContext::ensure_write_allowed(self, path).await
    }

    fn reconcile_member_runtime_metadata(
        &self,
        updated_members: &[TeamMemberRecord],
    ) -> Result<()> {
        reconcile_member_runtime_metadata(self, updated_members)
    }
}

impl TeamRuntimeSnapshotProvider for ToolExecutionContext {
    fn background_runtime_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<TeamMemberRuntimeSnapshot>> {
        Ok(
            effective_background_record(session_id)?.map(|record| TeamMemberRuntimeSnapshot {
                backend: Some(record.backend),
                layout_slot: record.layout_slot,
                pane_target: record.pane_target,
            }),
        )
    }

    fn persisted_runtime_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<TeamMemberRuntimeSnapshot>> {
        Ok(StoredSession::load(session_id)
            .ok()
            .and_then(|stored| stored.snapshot.agent_runtime)
            .map(|runtime| TeamMemberRuntimeSnapshot {
                backend: runtime.backend,
                layout_slot: runtime.layout_slot,
                pane_target: runtime.pane_target,
            }))
    }
}

pub(super) async fn resolve_team_members(
    context: &ToolExecutionContext,
    team_name: &str,
    targets: Option<Vec<String>>,
) -> Result<Vec<TeamMemberRecord>> {
    hellox_tools_agent::team_coordination_runtime::resolve_team_members(context, team_name, targets)
        .await
}

pub(super) async fn resolve_team_selection(
    context: &ToolExecutionContext,
    team_name: &str,
    targets: Option<Vec<String>>,
) -> Result<ResolvedTeamSelection> {
    hellox_tools_agent::team_coordination_runtime::resolve_team_selection(
        context, team_name, targets,
    )
    .await
}

pub(super) async fn persist_team_member_runtime_updates(
    context: &ToolExecutionContext,
    team_name: &str,
    updated_members: &[TeamMemberRecord],
) -> Result<()> {
    hellox_tools_agent::team_coordination_runtime::persist_team_member_runtime_updates(
        context,
        team_name,
        updated_members,
    )
    .await
}

pub(super) async fn persist_team_runtime_reconciliation(
    context: &ToolExecutionContext,
    requested_name: Option<&str>,
) -> Result<()> {
    hellox_tools_agent::team_coordination_runtime::persist_team_runtime_reconciliation(
        context,
        requested_name,
    )
    .await
}

pub(super) async fn reconcile_team_runtime_for_session(
    context: &ToolExecutionContext,
    session_id: &str,
) -> Result<()> {
    hellox_tools_agent::team_coordination_runtime::reconcile_team_runtime_for_session(
        context, session_id,
    )
    .await
}

pub(super) async fn persist_team_runtime_value_for_session(
    context: &ToolExecutionContext,
    session_id: &str,
    value: &Value,
) -> Result<()> {
    hellox_tools_agent::team_coordination_runtime::persist_team_runtime_value_for_session(
        context, session_id, value,
    )
    .await
}

pub(super) fn refresh_team_record_runtime(
    context: &ToolExecutionContext,
    team: &TeamRecord,
) -> Result<TeamRecord> {
    hellox_tools_agent::team_runtime_reconciliation::refresh_team_record_runtime(context, team)
}
