use anyhow::Result;
use async_trait::async_trait;
use hellox_tool_runtime::LocalTool as RuntimeLocalTool;
use serde_json::Value;

use super::super::{LocalTool, LocalToolResult, ToolExecutionContext, ToolRegistry};
use super::runtime::run_agent_prompt;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register(WorkflowTool);
}

pub(super) struct WorkflowTool;

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

#[async_trait]
impl LocalTool for WorkflowTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        RuntimeLocalTool::<ToolExecutionContext>::definition(
            &hellox_tools_agent::workflow::WorkflowTool,
        )
    }

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        RuntimeLocalTool::<ToolExecutionContext>::call(
            &hellox_tools_agent::workflow::WorkflowTool,
            input,
            context,
        )
        .await
    }
}
