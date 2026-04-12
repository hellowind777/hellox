use anyhow::Result;
use async_trait::async_trait;
use hellox_config::PermissionMode;
use serde_json::Value;

use super::super::{ToolExecutionContext, ToolRegistry};
use super::team_layout_runtime::sync_team_layout_runtime;
use super::team_member_support::{materialize_members, render_removed_members};
use super::team_registry_support::{sync_member_permissions, sync_member_runtime_metadata};

pub(super) use hellox_tools_agent::team_registry_tool::{
    TeamCreateTool, TeamDeleteTool, TeamUpdateTool,
};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(TeamCreateTool);
    registry.register_runtime(TeamUpdateTool);
    registry.register_runtime(TeamDeleteTool);
}

#[async_trait]
impl hellox_tools_agent::team_registry_tool::TeamRegistryToolContext for ToolExecutionContext {
    async fn ensure_write_allowed(&self, path: &std::path::Path) -> Result<()> {
        ToolExecutionContext::ensure_write_allowed(self, path).await
    }

    async fn materialize_members(
        &self,
        members: Vec<hellox_tools_agent::team_member_contract::PlannedTeamMember>,
        default_permission_mode: Option<PermissionMode>,
        pane_group: Option<&str>,
        layout_strategy: Option<&str>,
        existing_members: &[hellox_tools_agent::team_storage::TeamMemberRecord],
    ) -> Result<(
        Vec<hellox_tools_agent::team_storage::TeamMemberRecord>,
        Vec<Value>,
    )> {
        materialize_members(
            self,
            members,
            default_permission_mode,
            pane_group,
            layout_strategy,
            existing_members,
        )
        .await
    }

    fn render_removed_members(
        &self,
        members: &[hellox_tools_agent::team_storage::TeamMemberRecord],
        stop_removed: bool,
        reason: Option<String>,
    ) -> Result<Vec<Value>> {
        render_removed_members(members, stop_removed, reason)
    }

    fn sync_member_runtime_metadata(
        &self,
        members: &[hellox_tools_agent::team_storage::TeamMemberRecord],
    ) -> Result<()> {
        sync_member_runtime_metadata(self, members)
    }

    fn sync_member_permissions(
        &self,
        members: &[hellox_tools_agent::team_storage::TeamMemberRecord],
        permission_mode: Option<&PermissionMode>,
    ) -> Result<Vec<Value>> {
        sync_member_permissions(self, members, permission_mode)
    }

    fn sync_team_layout_runtime(
        &self,
        pane_group: Option<&str>,
        team: &hellox_tools_agent::team_storage::TeamRecord,
    ) -> Result<Value> {
        sync_team_layout_runtime(pane_group, team)
    }
}
