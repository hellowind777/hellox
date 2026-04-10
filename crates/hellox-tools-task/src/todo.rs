use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::TaskToolContext;
use hellox_tool_runtime::{display_path, LocalTool, LocalToolResult, ToolRegistry};

pub(crate) fn register_todo_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TaskToolContext + Send + Sync + 'static,
{
    registry.register(TodoWriteTool);
}

pub struct TodoWriteTool;

#[async_trait]
impl<C> LocalTool<C> for TodoWriteTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TodoWrite".to_string(),
            description: Some("Persist the current todo list for the workspace.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "content": { "type": "string" },
                                "status": { "type": "string" },
                                "priority": { "type": "string" }
                            },
                            "required": ["content"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let todos = parse_todos(&input)?;
        let path = todo_file_path(context.working_directory());
        let label = display_path(context.working_directory(), &path);
        let old_todos = read_todos(&path)?;
        context.ensure_write_allowed(&path).await?;
        write_todos(&path, &todos)?;

        Ok(LocalToolResult::text(
            serde_json::to_string_pretty(&json!({
                "path": label,
                "oldTodos": old_todos,
                "newTodos": todos,
            }))
            .context("failed to serialize TodoWrite result")?,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TodoItem {
    #[serde(default)]
    id: Option<String>,
    content: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    priority: Option<String>,
}

fn parse_todos(input: &Value) -> Result<Vec<TodoItem>> {
    let todos_value = input
        .get("todos")
        .ok_or_else(|| anyhow!("missing required field `todos`"))?;
    let todos = serde_json::from_value::<Vec<TodoItem>>(todos_value.clone())
        .context("failed to parse todos array")?;
    for todo in &todos {
        if let Some(status) = &todo.status {
            let allowed = ["pending", "in_progress", "completed", "cancelled"];
            if !allowed.iter().any(|item| item == status) {
                return Err(anyhow!("unsupported todo status `{status}`"));
            }
        }
    }
    Ok(todos)
}

fn todo_file_path(root: &std::path::Path) -> PathBuf {
    root.join(".hellox").join("todos.json")
}

fn read_todos(path: &PathBuf) -> Result<Vec<TodoItem>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read todo file {}", path.display()))?;
    serde_json::from_str::<Vec<TodoItem>>(&raw)
        .with_context(|| format!("failed to parse todo file {}", path.display()))
}

fn write_todos(path: &PathBuf, todos: &[TodoItem]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create todo directory {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(todos).context("failed to serialize todos")?;
    fs::write(path, format!("{raw}\n"))
        .with_context(|| format!("failed to write todo file {}", path.display()))
}
