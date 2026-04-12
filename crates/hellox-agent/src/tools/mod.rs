use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_gateway_api::ToolDefinition;
use serde_json::Value;

use crate::permissions::{resolve_permission, ApprovalHandler, PermissionPolicy, QuestionHandler};
use crate::planning::{PlanItem, PlanningState};
use crate::telemetry::SharedTelemetrySink;

mod agent;
mod fs;
mod mcp;
mod runtime;
mod shell;
mod tasks;
mod ui;
mod utility;
mod web;

pub use agent::{pane_backend_preflight, PaneBackendPreflight, PaneCommandPrefixStatus};

#[derive(Clone)]
pub struct ToolExecutionContext {
    pub config_path: PathBuf,
    pub planning_state: Arc<Mutex<PlanningState>>,
    pub working_directory: PathBuf,
    pub permission_policy: PermissionPolicy,
    pub approval_handler: Option<Arc<dyn ApprovalHandler>>,
    pub question_handler: Option<Arc<dyn QuestionHandler>>,
    pub telemetry_sink: Option<SharedTelemetrySink>,
}

impl ToolExecutionContext {
    pub fn resolve_path(&self, raw: &str) -> PathBuf {
        let path = PathBuf::from(raw);
        if path.is_absolute() {
            path
        } else {
            self.working_directory.join(path)
        }
    }

    pub async fn ensure_write_allowed(&self, path: &Path) -> Result<()> {
        let decision = self.permission_policy.check_write_path(path);
        if let Some(reason) = resolve_permission(decision, self.approval_handler.clone()).await? {
            return Err(anyhow!(reason));
        }
        Ok(())
    }

    pub async fn ensure_shell_allowed(&self, command: &str) -> Result<()> {
        let decision = self.permission_policy.check_shell_command(command);
        if let Some(reason) = resolve_permission(decision, self.approval_handler.clone()).await? {
            return Err(anyhow!(reason));
        }
        Ok(())
    }

    pub fn planning_state(&self) -> Result<PlanningState> {
        self.planning_state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| anyhow!("planning state lock was poisoned"))
    }

    pub fn enter_plan_mode(&self) -> Result<PlanningState> {
        let mut state = self
            .planning_state
            .lock()
            .map_err(|_| anyhow!("planning state lock was poisoned"))?;
        state.enter();
        Ok(state.clone())
    }

    pub fn exit_plan_mode(
        &self,
        plan: Vec<PlanItem>,
        allowed_prompts: Vec<String>,
    ) -> Result<PlanningState> {
        let mut state = self
            .planning_state
            .lock()
            .map_err(|_| anyhow!("planning state lock was poisoned"))?;
        state.exit(plan, allowed_prompts)?;
        Ok(state.clone())
    }

    pub fn set_planning_state(&self, planning: PlanningState) -> Result<PlanningState> {
        let mut state = self
            .planning_state
            .lock()
            .map_err(|_| anyhow!("planning state lock was poisoned"))?;
        *state = planning;
        Ok(state.clone())
    }
}

pub(crate) type LocalToolResult = hellox_tool_runtime::LocalToolResult;

#[async_trait]
pub(crate) trait LocalTool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult>;
}

struct ToolAdapter<T>(T);

#[async_trait]
impl<T> hellox_tool_runtime::LocalTool<ToolExecutionContext> for ToolAdapter<T>
where
    T: LocalTool + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        self.0.definition()
    }

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        self.0.call(input, context).await
    }
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    inner: hellox_tool_runtime::ToolRegistry<ToolExecutionContext>,
}

impl ToolRegistry {
    pub(crate) fn register<T>(&mut self, tool: T)
    where
        T: LocalTool + 'static,
    {
        self.inner.register(ToolAdapter(tool));
    }

    pub(crate) fn register_runtime<T>(&mut self, tool: T)
    where
        T: hellox_tool_runtime::LocalTool<ToolExecutionContext> + 'static,
    {
        self.inner.register(tool);
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.inner.definitions()
    }

    pub(crate) async fn execute(
        &self,
        name: &str,
        input: Value,
        context: &ToolExecutionContext,
    ) -> LocalToolResult {
        self.inner.execute(name, input, context).await
    }
}

pub fn default_tool_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::default();
    agent::register_tools(&mut registry);
    fs::register_tools(&mut registry);
    mcp::register_tools(&mut registry);
    runtime::register_tools(&mut registry);
    shell::register_tools(&mut registry);
    tasks::register_tools(&mut registry);
    ui::register_tools(&mut registry);
    utility::register_tools(&mut registry);
    web::register_tools(&mut registry);
    registry
}
