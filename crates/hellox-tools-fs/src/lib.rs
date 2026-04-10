mod files;
mod metadata;
mod notebook;
mod search;
mod support;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use hellox_tool_runtime::ToolRegistry;

pub use files::{EditFileTool, ListFilesTool, ReadFileTool, WriteFileTool};
pub use notebook::NotebookEditTool;
pub use search::{GlobTool, GrepTool};

/// Minimal context contract shared by filesystem-facing local tools.
#[async_trait]
pub trait FsToolContext: Send + Sync {
    /// Resolves a user-provided path against the active workspace root.
    fn resolve_path(&self, raw: &str) -> PathBuf;

    /// Returns the current workspace root.
    fn working_directory(&self) -> &Path;

    /// Validates whether a write to the provided path is allowed.
    async fn ensure_write_allowed(&self, path: &Path) -> anyhow::Result<()>;
}

/// Registers filesystem-domain tools into a shared runtime registry.
pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: FsToolContext + Send + Sync + 'static,
{
    registry.register(ListFilesTool);
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(EditFileTool);
    registry.register(NotebookEditTool);
    registry.register(GlobTool);
    registry.register(GrepTool);
}
