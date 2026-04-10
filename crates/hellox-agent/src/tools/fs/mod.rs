#[cfg(test)]
mod tests;

use async_trait::async_trait;

use super::{ToolExecutionContext, ToolRegistry};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_fs::ListFilesTool);
    registry.register_runtime(hellox_tools_fs::ReadFileTool);
    registry.register_runtime(hellox_tools_fs::WriteFileTool);
    registry.register_runtime(hellox_tools_fs::EditFileTool);
    registry.register_runtime(hellox_tools_fs::NotebookEditTool);
    registry.register_runtime(hellox_tools_fs::GlobTool);
    registry.register_runtime(hellox_tools_fs::GrepTool);
}

#[async_trait]
impl hellox_tools_fs::FsToolContext for ToolExecutionContext {
    fn resolve_path(&self, raw: &str) -> std::path::PathBuf {
        ToolExecutionContext::resolve_path(self, raw)
    }

    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    async fn ensure_write_allowed(&self, path: &std::path::Path) -> anyhow::Result<()> {
        ToolExecutionContext::ensure_write_allowed(self, path).await
    }
}
