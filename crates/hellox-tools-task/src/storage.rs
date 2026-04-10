use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const TASK_FILE_NAME: &str = "todos.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TaskRecord {
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) priority: Option<String>,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) output: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct TaskOutput {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) priority: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) output: Option<String>,
}

impl TaskRecord {
    pub(crate) fn to_output(&self) -> TaskOutput {
        TaskOutput {
            id: self.id.clone(),
            title: self.content.clone(),
            status: self.status.clone(),
            priority: self.priority.clone(),
            description: self.description.clone(),
            output: self.output.clone(),
        }
    }
}

pub fn task_file_path(root: &Path) -> PathBuf {
    root.join(".hellox").join(TASK_FILE_NAME)
}

pub(crate) fn load_tasks(path: &Path) -> Result<Vec<TaskRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read task file {}", path.display()))?;
    let tasks = serde_json::from_str::<Vec<TaskRecord>>(&raw)
        .with_context(|| format!("failed to parse task file {}", path.display()))?;
    Ok(tasks)
}

pub(crate) fn save_tasks(path: &Path, tasks: &[TaskRecord]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create task directory {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(tasks).context("failed to serialize tasks")?;
    fs::write(path, raw).with_context(|| format!("failed to write task file {}", path.display()))
}

pub(crate) fn find_task<'a>(tasks: &'a [TaskRecord], task_id: &str) -> Result<&'a TaskRecord> {
    tasks
        .iter()
        .find(|task| task.id == task_id)
        .ok_or_else(|| anyhow!("task `{task_id}` was not found"))
}

pub(crate) fn find_task_mut<'a>(
    tasks: &'a mut [TaskRecord],
    task_id: &str,
) -> Result<&'a mut TaskRecord> {
    tasks
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| anyhow!("task `{task_id}` was not found"))
}

pub(crate) fn next_task_id(tasks: &[TaskRecord]) -> String {
    let next = tasks
        .iter()
        .filter_map(|task| task.id.strip_prefix("task-"))
        .filter_map(|suffix| suffix.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    format!("task-{next}")
}

pub(crate) fn optional_string(input: &Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn value_to_optional_string(value: Option<&Value>) -> Result<Option<Option<String>>> {
    match value {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Ok(Some(None))
            } else {
                Ok(Some(Some(text.to_string())))
            }
        }
        Some(_) => Err(anyhow!("expected string or null")),
    }
}

pub(crate) fn validate_status(status: &str) -> Result<()> {
    if matches!(
        status.trim(),
        "pending" | "in_progress" | "completed" | "cancelled"
    ) {
        Ok(())
    } else {
        Err(anyhow!(
            "unsupported task status `{status}`; use pending, in_progress, completed, or cancelled"
        ))
    }
}

pub(crate) fn render_json(value: Value) -> Result<String> {
    serde_json::to_string_pretty(&value).context("failed to serialize task tool result")
}
