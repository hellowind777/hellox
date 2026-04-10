mod background;
mod coordination;
mod process_backend;
mod runtime;
mod runtime_support;
mod supervision;
mod team;
mod team_coordination_support;
mod team_member_support;
mod team_registry;
mod team_registry_support;
mod workflow;

use super::ToolRegistry;

pub(crate) use hellox_tools_agent::{
    native_pane_backend, native_pane_backend_preflight, shared, team_layout_runtime,
    team_member_contract, team_storage,
};
pub use native_pane_backend_preflight::{PaneBackendPreflight, PaneCommandPrefixStatus};

/// Collects shared pane host preflight details for diagnostics and runtime checks.
pub fn pane_backend_preflight() -> PaneBackendPreflight {
    native_pane_backend_preflight::pane_backend_preflight()
}

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    runtime::register_tools(registry);
    coordination::register_tools(registry);
    supervision::register_tools(registry);
    team::register_tools(registry);
    team_registry::register_tools(registry);
    workflow::register_tools(registry);
}

#[cfg(test)]
mod tests;
