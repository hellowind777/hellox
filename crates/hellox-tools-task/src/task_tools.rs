use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::storage::{
    find_task, find_task_mut, load_tasks, next_task_id, optional_string, render_json, save_tasks,
    task_file_path, validate_status, value_to_optional_string, TaskRecord,
};
use crate::TaskToolContext;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};

pub(crate) fn register_task_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TaskToolContext + Send + Sync + 'static,
{
    registry.register(TaskCreateTool);
    registry.register(TaskGetTool);
    registry.register(TaskListTool);
    registry.register(TaskUpdateTool);
    registry.register(TaskStopTool);
    registry.register(TaskOutputTool);
}

pub struct TaskCreateTool;
pub struct TaskGetTool;
pub struct TaskListTool;
pub struct TaskUpdateTool;
pub struct TaskStopTool;
pub struct TaskOutputTool;

#[async_trait]
impl<C> LocalTool<C> for TaskCreateTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TaskCreate".to_string(),
            description: Some(
                "Create a structured local task in the workspace task store.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "priority": { "type": "string" }
                },
                "required": ["title"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let title = required_string(&input, "title")?.trim().to_string();
        if title.is_empty() {
            return Err(anyhow!("task title cannot be empty"));
        }

        let path = task_file_path(context.working_directory());
        let mut tasks = load_tasks(&path)?;
        let task = TaskRecord {
            id: next_task_id(&tasks),
            content: title,
            status: "pending".to_string(),
            priority: optional_string(&input, "priority"),
            description: optional_string(&input, "description"),
            output: None,
        };

        context.ensure_write_allowed(&path).await?;
        tasks.push(task.clone());
        save_tasks(&path, &tasks)?;

        Ok(LocalToolResult::text(render_json(json!({
            "task": task.to_output(),
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TaskGetTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TaskGet".to_string(),
            description: Some("Load a single local task by id.".to_string()),
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
        let path = task_file_path(context.working_directory());
        let tasks = load_tasks(&path)?;
        let task = find_task(&tasks, task_id)?.to_output();

        Ok(LocalToolResult::text(render_json(json!({
            "task": task,
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TaskListTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TaskList".to_string(),
            description: Some("List local tasks, optionally filtered by status.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 }
                }
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let status = optional_string(&input, "status");
        if let Some(status) = &status {
            validate_status(status)?;
        }

        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value as usize);
        let path = task_file_path(context.working_directory());
        let mut tasks = load_tasks(&path)?;
        if let Some(status) = status {
            tasks.retain(|task| task.status == status);
        }
        if let Some(limit) = limit {
            tasks.truncate(limit);
        }

        Ok(LocalToolResult::text(render_json(json!({
            "tasks": tasks.iter().map(TaskRecord::to_output).collect::<Vec<_>>(),
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TaskUpdateTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TaskUpdate".to_string(),
            description: Some(
                "Update task fields such as title, description, status, priority, or output."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "priority": { "type": "string" },
                    "status": { "type": "string" },
                    "output": { "type": "string" }
                },
                "required": ["id"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let task_id = required_string(&input, "id")?;
        let path = task_file_path(context.working_directory());
        let mut tasks = load_tasks(&path)?;
        let task = find_task_mut(&mut tasks, task_id)?;

        if let Some(title) = optional_string(&input, "title") {
            if title.trim().is_empty() {
                return Err(anyhow!("task title cannot be empty"));
            }
            task.content = title;
        }
        if let Some(description) = value_to_optional_string(input.get("description"))? {
            task.description = description;
        }
        if let Some(priority) = value_to_optional_string(input.get("priority"))? {
            task.priority = priority;
        }
        if let Some(status) = optional_string(&input, "status") {
            validate_status(&status)?;
            task.status = status;
        }
        if let Some(output) = value_to_optional_string(input.get("output"))? {
            task.output = output;
        }

        let task = task.clone();
        context.ensure_write_allowed(&path).await?;
        save_tasks(&path, &tasks)?;

        Ok(LocalToolResult::text(render_json(json!({
            "task": task.to_output(),
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TaskStopTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TaskStop".to_string(),
            description: Some(
                "Cancel a local task and optionally record the stop reason.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "reason": { "type": "string" }
                },
                "required": ["id"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let task_id = required_string(&input, "id")?;
        let path = task_file_path(context.working_directory());
        let mut tasks = load_tasks(&path)?;
        let task = find_task_mut(&mut tasks, task_id)?;
        task.status = "cancelled".to_string();
        if let Some(reason) = optional_string(&input, "reason") {
            task.output = Some(reason);
        }

        let task = task.clone();
        context.ensure_write_allowed(&path).await?;
        save_tasks(&path, &tasks)?;

        Ok(LocalToolResult::text(render_json(json!({
            "task": task.to_output(),
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TaskOutputTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TaskOutput".to_string(),
            description: Some("Read the latest stored output for a local task.".to_string()),
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
        let path = task_file_path(context.working_directory());
        let tasks = load_tasks(&path)?;
        let task = find_task(&tasks, task_id)?;

        Ok(LocalToolResult::text(render_json(json!({
            "task": task.to_output(),
            "output": task.output,
        }))?))
    }
}
