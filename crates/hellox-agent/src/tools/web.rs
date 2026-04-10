use super::ToolRegistry;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_web::WebFetchTool);
    registry.register_runtime(hellox_tools_web::WebSearchTool);
}
