use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use super::super::{ToolExecutionContext, ToolRegistry};
use super::runtime::run_agent_prompt;

pub(super) use hellox_tools_agent::workflow::WorkflowTool;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(WorkflowTool);
}

#[async_trait]
impl hellox_tools_agent::workflow_support::WorkflowToolContext for ToolExecutionContext {
    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    fn resolve_path(&self, raw: &str) -> std::path::PathBuf {
        ToolExecutionContext::resolve_path(self, raw)
    }

    async fn run_workflow_step(
        &self,
        request: hellox_tools_agent::shared::AgentRunRequest,
    ) -> Result<Value> {
        run_agent_prompt(self, request).await
    }
}
