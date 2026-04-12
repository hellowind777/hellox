use super::{ToolExecutionContext, ToolRegistry};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_mcp::McpTool);
    registry.register_runtime(hellox_tools_mcp::ListMcpResourcesTool);
    registry.register_runtime(hellox_tools_mcp::ReadMcpResourceTool);
    registry.register_runtime(hellox_tools_mcp::ListMcpPromptsTool);
    registry.register_runtime(hellox_tools_mcp::GetMcpPromptTool);
    registry.register_runtime(hellox_tools_mcp::McpAuthTool);
    registry.register_runtime(hellox_tools_mcp::LspTool);
}

impl hellox_tools_mcp::McpToolContext for ToolExecutionContext {
    fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }
}

#[cfg(test)]
mod tests {
    use super::register_tools;
    use crate::tools::ToolRegistry;

    #[test]
    fn default_mcp_registry_exposes_lsp_tool() {
        let mut registry = ToolRegistry::default();
        register_tools(&mut registry);
        let names = registry
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        assert!(names.iter().any(|name| name == "LSP"));
    }
}
