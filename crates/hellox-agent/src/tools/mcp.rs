use super::{ToolExecutionContext, ToolRegistry};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_mcp::McpTool);
    registry.register_runtime(hellox_tools_mcp::ListMcpResourcesTool);
    registry.register_runtime(hellox_tools_mcp::ReadMcpResourceTool);
    registry.register_runtime(hellox_tools_mcp::ListMcpPromptsTool);
    registry.register_runtime(hellox_tools_mcp::GetMcpPromptTool);
    registry.register_runtime(hellox_tools_mcp::McpAuthTool);
}

impl hellox_tools_mcp::McpToolContext for ToolExecutionContext {
    fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }
}
