use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_config::load_or_default;
use serde_json::{json, Value};

use crate::cron::parse_cron_expression;
use crate::cron_storage::{add_task, list_tasks, remove_task};
use crate::storage::render_json;
use crate::TaskToolContext;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};

pub(crate) fn register_cron_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TaskToolContext + Send + Sync + 'static,
{
    registry.register(CronCreateTool);
    registry.register(CronDeleteTool);
    registry.register(CronListTool);
}

pub struct CronCreateTool;
pub struct CronDeleteTool;
pub struct CronListTool;

#[async_trait]
impl<C> LocalTool<C> for CronCreateTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "CronCreate".to_string(),
            description: Some(
                "Schedule a local recurring or one-shot prompt using a 5-field cron expression."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cron": { "type": "string" },
                    "prompt": { "type": "string" },
                    "recurring": { "type": "boolean" },
                    "durable": { "type": "boolean" }
                },
                "required": ["cron", "prompt"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let cron = required_string(&input, "cron")?;
        let prompt = required_string(&input, "prompt")?;
        if prompt.trim().is_empty() {
            return Err(anyhow!("scheduled prompt cannot be empty"));
        }

        parse_cron_expression(cron)?;
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        if !config.scheduler.enabled {
            return Err(anyhow!("local scheduler is disabled in config"));
        }

        let durable = input
            .get("durable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let recurring = input
            .get("recurring")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let path = hellox_config::scheduled_tasks_path_for(context.config_path());
        context.ensure_write_allowed(&path).await?;
        let record = add_task(
            context.config_path(),
            &config,
            cron,
            prompt,
            recurring,
            durable,
        )?;

        Ok(LocalToolResult::text(render_json(json!({
            "task": record,
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for CronDeleteTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "CronDelete".to_string(),
            description: Some("Cancel a scheduled local cron task by id.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let task_id = required_string(&input, "id")?;
        let path = hellox_config::scheduled_tasks_path_for(context.config_path());
        context.ensure_write_allowed(&path).await?;
        if !remove_task(context.config_path(), task_id)? {
            return Err(anyhow!("scheduled task `{task_id}` was not found"));
        }

        Ok(LocalToolResult::text(render_json(json!({
            "deleted": task_id,
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for CronListTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "CronList".to_string(),
            description: Some("List scheduled local cron tasks.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, _input: Value, context: &C) -> Result<LocalToolResult> {
        let tasks = list_tasks(context.config_path())?;
        Ok(LocalToolResult::text(render_json(json!({
            "tasks": tasks,
        }))?))
    }
}
