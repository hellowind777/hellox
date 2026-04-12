use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::{ToolExecutionContext, ToolRegistry};
use super::background::agent_status_value;
use super::team_coordination_support::{
    persist_team_runtime_reconciliation, refresh_team_record_runtime,
};
use super::team_layout_runtime::summarize_layout_runtime;

pub(super) use hellox_tools_agent::team_status_tool::TeamStatusTool;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(TeamStatusTool);
}

pub(super) fn team_status_value(
    context: &ToolExecutionContext,
    requested_name: Option<&str>,
) -> Result<Value> {
    hellox_tools_agent::team_status_tool::team_status_value(context, requested_name)
}

#[async_trait]
impl hellox_tools_agent::team_status_tool::TeamStatusToolContext for ToolExecutionContext {
    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    async fn persist_team_runtime_reconciliation(
        &self,
        requested_name: Option<&str>,
    ) -> Result<()> {
        persist_team_runtime_reconciliation(self, requested_name).await
    }

    fn refresh_team_record_runtime(
        &self,
        team: &hellox_tools_agent::team_storage::TeamRecord,
    ) -> Result<hellox_tools_agent::team_storage::TeamRecord> {
        refresh_team_record_runtime(self, team)
    }

    fn summarize_layout_runtime(
        &self,
        team: &hellox_tools_agent::team_storage::TeamRecord,
    ) -> Value {
        summarize_layout_runtime(team)
    }

    fn background_agent_status_value(&self, session_id: &str) -> Value {
        agent_status_value(session_id).unwrap_or_else(|error| {
            json!({
                "session_id": session_id,
                "status": "missing",
                "error": error.to_string(),
            })
        })
    }
}
