use async_trait::async_trait;

use super::{ToolExecutionContext, ToolRegistry};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_shell::RunShellTool);
}

#[async_trait]
impl hellox_tools_shell::ShellToolContext for ToolExecutionContext {
    async fn ensure_shell_allowed(&self, command: &str) -> anyhow::Result<()> {
        ToolExecutionContext::ensure_shell_allowed(self, command).await
    }

    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }
}
