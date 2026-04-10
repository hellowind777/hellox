mod client;
mod fetch;
mod search;
mod support;

use hellox_tool_runtime::ToolRegistry;

pub use fetch::WebFetchTool;
pub use search::WebSearchTool;

/// Registers web-domain tools into a shared runtime registry.
pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: Send + Sync + 'static,
{
    registry.register(WebFetchTool);
    registry.register(WebSearchTool);
}
