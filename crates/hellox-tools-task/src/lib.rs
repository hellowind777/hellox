mod cron;
mod cron_storage;
mod cron_tools;
mod planning;
mod storage;
mod task_tools;
mod todo;

#[cfg(test)]
mod tests;

use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use cron_tools::{CronCreateTool, CronDeleteTool, CronListTool};
pub use planning::{EnterPlanModeTool, ExitPlanModeTool};
pub use storage::task_file_path;
pub use task_tools::{
    TaskCreateTool, TaskGetTool, TaskListTool, TaskOutputTool, TaskStopTool, TaskUpdateTool,
};
pub use todo::TodoWriteTool;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanItem {
    pub step: String,
    pub status: String,
}

impl PlanItem {
    pub fn validate(&self) -> Result<()> {
        if self.step.trim().is_empty() {
            return Err(anyhow!("plan step cannot be empty"));
        }
        if !matches!(self.status.trim(), "pending" | "in_progress" | "completed") {
            return Err(anyhow!(
                "unsupported plan status `{}`; use pending, in_progress, or completed",
                self.status
            ));
        }
        Ok(())
    }

    pub fn normalized(&self) -> Self {
        Self {
            step: self.step.trim().to_string(),
            status: self.status.trim().to_string(),
        }
    }
}

/// Minimal context contract shared by task/planning tools.
#[async_trait]
pub trait TaskToolContext: Send + Sync {
    /// Returns the current workspace root.
    fn working_directory(&self) -> &Path;

    /// Returns the active config path for user-level task storage.
    fn config_path(&self) -> &Path;

    /// Validates whether a write to the provided path is allowed.
    async fn ensure_write_allowed(&self, path: &Path) -> anyhow::Result<()>;

    /// Enters plan mode and returns a serializable planning snapshot.
    fn enter_plan_mode(&self) -> anyhow::Result<Value>;

    /// Exits plan mode and returns a serializable planning snapshot.
    fn exit_plan_mode(
        &self,
        plan: Vec<PlanItem>,
        allowed_prompts: Vec<String>,
    ) -> anyhow::Result<Value>;
}

/// Registers task/planning tools into a shared runtime registry.
pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TaskToolContext + Send + Sync + 'static,
{
    cron_tools::register_cron_tools(registry);
    task_tools::register_task_tools(registry);
    planning::register_planning_tools(registry);
    todo::register_todo_tools(registry);
}
