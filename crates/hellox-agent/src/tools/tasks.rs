use async_trait::async_trait;
use serde_json::Value;

use super::ToolRegistry;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_task::CronCreateTool);
    registry.register_runtime(hellox_tools_task::CronDeleteTool);
    registry.register_runtime(hellox_tools_task::CronListTool);
    registry.register_runtime(hellox_tools_task::TaskCreateTool);
    registry.register_runtime(hellox_tools_task::TaskGetTool);
    registry.register_runtime(hellox_tools_task::TaskListTool);
    registry.register_runtime(hellox_tools_task::TaskUpdateTool);
    registry.register_runtime(hellox_tools_task::TaskStopTool);
    registry.register_runtime(hellox_tools_task::TaskOutputTool);
    registry.register_runtime(hellox_tools_task::EnterPlanModeTool);
    registry.register_runtime(hellox_tools_task::ExitPlanModeTool);
    registry.register_runtime(hellox_tools_task::TodoWriteTool);
}

#[async_trait]
impl hellox_tools_task::TaskToolContext for crate::tools::ToolExecutionContext {
    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    async fn ensure_write_allowed(&self, path: &std::path::Path) -> anyhow::Result<()> {
        crate::tools::ToolExecutionContext::ensure_write_allowed(self, path).await
    }

    fn enter_plan_mode(&self) -> anyhow::Result<Value> {
        serde_json::to_value(crate::tools::ToolExecutionContext::enter_plan_mode(self)?)
            .map_err(Into::into)
    }

    fn exit_plan_mode(
        &self,
        plan: Vec<hellox_tools_task::PlanItem>,
        allowed_prompts: Vec<String>,
    ) -> anyhow::Result<Value> {
        let plan = plan
            .into_iter()
            .map(|item| crate::planning::PlanItem {
                step: item.step,
                status: item.status,
            })
            .collect::<Vec<_>>();
        serde_json::to_value(crate::tools::ToolExecutionContext::exit_plan_mode(
            self,
            plan,
            allowed_prompts,
        )?)
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
