use super::ToolRegistry;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_agent::sleep_tool::SleepTool);
    registry.register_runtime(hellox_tools_agent::worktree_tool::EnterWorktreeTool);
    registry.register_runtime(hellox_tools_agent::worktree_tool::ExitWorktreeTool);

    // Keep remote-control tools out of the default local-first tool surface unless explicitly
    // enabled. This prevents the model from attempting remote calls on single-machine setups.
    if std::env::var("HELLOX_ENABLE_REMOTE_TRIGGER").ok().is_some() {
        registry.register_runtime(hellox_tools_agent::remote_trigger_tool::RemoteTriggerTool);
    }
}

impl hellox_tools_agent::worktree_tool::WorktreeToolContext for super::ToolExecutionContext {
    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }
}

#[cfg(test)]
mod tests;
