use anyhow::Result;
use async_trait::async_trait;
use hellox_tool_runtime::LocalTool as RuntimeLocalTool;
use serde_json::Value;

use super::super::{LocalTool, LocalToolResult, ToolExecutionContext, ToolRegistry};
use super::runtime::run_agent_prompt;
use super::team_coordination_support::{
    persist_team_member_runtime_updates, resolve_team_selection,
};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register(SendMessageTool);
    registry.register(TeamRunTool);
}

pub(super) struct SendMessageTool;
pub(super) struct TeamRunTool;

#[async_trait]
impl hellox_tools_agent::coordination_tool::TeamCoordinationToolContext for ToolExecutionContext {
    async fn resolve_team_selection(
        &self,
        team_name: &str,
        targets: Option<Vec<String>>,
    ) -> Result<hellox_tools_agent::coordination_tool::ResolvedTeamSelection> {
        let selection = resolve_team_selection(self, team_name, targets).await?;
        Ok(
            hellox_tools_agent::coordination_tool::ResolvedTeamSelection {
                team: selection.team,
                members: selection.members,
            },
        )
    }

    async fn persist_team_member_runtime_updates(
        &self,
        team_name: &str,
        updated_members: &[hellox_tools_agent::team_storage::TeamMemberRecord],
    ) -> Result<()> {
        persist_team_member_runtime_updates(self, team_name, updated_members).await
    }

    async fn run_agent_prompt(
        &self,
        request: hellox_tools_agent::shared::AgentRunRequest,
    ) -> Result<Value> {
        run_agent_prompt(self, request).await
    }
}

#[async_trait]
impl LocalTool for SendMessageTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        RuntimeLocalTool::<ToolExecutionContext>::definition(
            &hellox_tools_agent::coordination_tool::SendMessageTool,
        )
    }

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        RuntimeLocalTool::<ToolExecutionContext>::call(
            &hellox_tools_agent::coordination_tool::SendMessageTool,
            input,
            context,
        )
        .await
    }
}

#[async_trait]
impl LocalTool for TeamRunTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        RuntimeLocalTool::<ToolExecutionContext>::definition(
            &hellox_tools_agent::coordination_tool::TeamRunTool,
        )
    }

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        RuntimeLocalTool::<ToolExecutionContext>::call(
            &hellox_tools_agent::coordination_tool::TeamRunTool,
            input,
            context,
        )
        .await
    }
}
