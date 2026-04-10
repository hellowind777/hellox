mod brief;
mod config;
mod search;

#[cfg(test)]
mod tests;

use std::path::Path;

use async_trait::async_trait;
use hellox_gateway_api::ToolDefinition;
use hellox_tool_runtime::ToolRegistry;

pub use brief::{load_brief, BriefAttachment, BriefRecord, BriefTool};
pub use config::ConfigTool;
pub use search::ToolSearchTool;

/// Minimal context contract shared by UI-facing local tools.
#[async_trait]
pub trait UiToolContext: Send + Sync {
    /// Validates whether a write to the provided path is allowed.
    async fn ensure_write_allowed(&self, path: &Path) -> anyhow::Result<()>;

    /// Returns the current workspace root.
    fn working_directory(&self) -> &Path;

    /// Returns the active local config path.
    fn config_path(&self) -> &Path;

    /// Returns the tool definitions visible to the current runtime.
    fn available_tool_definitions(&self) -> Vec<ToolDefinition>;
}

/// Registers UI-facing tools into a shared runtime registry.
pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: UiToolContext + Send + Sync + 'static,
{
    registry.register(BriefTool);
    registry.register(ConfigTool);
    registry.register(ToolSearchTool);
}
