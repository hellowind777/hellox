use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const TASK_FILE_NAME: &str = "todos.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskTransition {
    Start,
    Complete,
    Cancel,
}

impl TaskTransition {
    pub fn status(&self) -> &'static str {
        match self {
            Self::Start => "in_progress",
            Self::Complete => "completed",
            Self::Cancel => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskPatch {
    pub content: Option<String>,
    pub status: Option<String>,
    pub priority: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub output: Option<Option<String>>,
}

impl TaskPatch {
    pub fn is_empty(&self) -> bool {
        self.content.is_none()
            && self.status.is_none()
            && self.priority.is_none()
            && self.description.is_none()
            && self.output.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskItem {
    pub id: String,
    pub content: String,
    pub status: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredTaskItem {
    #[serde(default)]
    id: Option<String>,
    content: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    output: Option<String>,
}

pub fn todo_file_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".hellox").join(TASK_FILE_NAME)
}

pub fn load_tasks(workspace_root: &Path) -> Result<Vec<TaskItem>> {
    let path = todo_file_path(workspace_root);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read task file {}", path.display()))?;
    let stored = serde_json::from_str::<Vec<StoredTaskItem>>(&raw)
        .with_context(|| format!("failed to parse task file {}", path.display()))?;

    Ok(stored
        .into_iter()
        .enumerate()
        .map(|(index, item)| TaskItem {
            id: item.id.unwrap_or_else(|| format!("task-{}", index + 1)),
            content: item.content,
            status: normalize_status(item.status.as_deref()).unwrap_or("pending".to_string()),
            priority: item.priority,
            description: item.description,
            output: item.output,
        })
        .collect())
}

pub fn save_tasks(workspace_root: &Path, tasks: &[TaskItem]) -> Result<()> {
    let path = todo_file_path(workspace_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create task directory {}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(tasks).context("failed to serialize tasks")?;
    fs::write(&path, raw).with_context(|| format!("failed to write task file {}", path.display()))
}

pub fn add_task(
    workspace_root: &Path,
    content: String,
    priority: Option<String>,
    description: Option<String>,
) -> Result<TaskItem> {
    let mut tasks = load_tasks(workspace_root)?;
    let task = TaskItem {
        id: next_task_id(&tasks),
        content: normalize_required_text(content, "task content")?,
        status: "pending".to_string(),
        priority: normalize_optional_text(priority),
        description: normalize_optional_text(description),
        output: None,
    };
    tasks.push(task.clone());
    save_tasks(workspace_root, &tasks)?;
    Ok(task)
}

pub fn get_task(workspace_root: &Path, task_id: &str) -> Result<TaskItem> {
    load_tasks(workspace_root)?
        .into_iter()
        .find(|task| task.id == task_id)
        .ok_or_else(|| anyhow!("task `{task_id}` was not found"))
}

pub fn update_task(workspace_root: &Path, task_id: &str, patch: TaskPatch) -> Result<TaskItem> {
    if patch.is_empty() {
        return Err(anyhow!("task update requires at least one field change"));
    }

    let mut tasks = load_tasks(workspace_root)?;
    let task = tasks
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| anyhow!("task `{task_id}` was not found"))?;

    if let Some(content) = patch.content {
        task.content = normalize_required_text(content, "task content")?;
    }
    if let Some(status) = patch.status {
        task.status = validate_task_status(&status)?;
    }
    if let Some(priority) = patch.priority {
        task.priority = normalize_optional_text(priority);
    }
    if let Some(description) = patch.description {
        task.description = normalize_optional_text(description);
    }
    if let Some(output) = patch.output {
        task.output = normalize_optional_text(output);
    }

    let updated = task.clone();
    save_tasks(workspace_root, &tasks)?;
    Ok(updated)
}

pub fn stop_task(workspace_root: &Path, task_id: &str, reason: Option<String>) -> Result<TaskItem> {
    let mut tasks = load_tasks(workspace_root)?;
    let task = tasks
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| anyhow!("task `{task_id}` was not found"))?;
    task.status = "cancelled".to_string();
    if let Some(reason) = reason {
        task.output = normalize_optional_text(Some(reason));
    }
    let updated = task.clone();
    save_tasks(workspace_root, &tasks)?;
    Ok(updated)
}

pub fn transition_task(
    workspace_root: &Path,
    task_id: &str,
    transition: TaskTransition,
) -> Result<TaskItem> {
    let mut tasks = load_tasks(workspace_root)?;
    let task = tasks
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| anyhow!("task `{task_id}` was not found"))?;
    task.status = transition.status().to_string();
    let updated = task.clone();
    save_tasks(workspace_root, &tasks)?;
    Ok(updated)
}

pub fn remove_task(workspace_root: &Path, task_id: &str) -> Result<TaskItem> {
    let mut tasks = load_tasks(workspace_root)?;
    let index = tasks
        .iter()
        .position(|task| task.id == task_id)
        .ok_or_else(|| anyhow!("task `{task_id}` was not found"))?;
    let removed = tasks.remove(index);
    save_tasks(workspace_root, &tasks)?;
    Ok(removed)
}

pub fn clear_tasks(
    workspace_root: &Path,
    completed_only: bool,
    all: bool,
) -> Result<Vec<TaskItem>> {
    if completed_only == all {
        return Err(anyhow!(
            "choose exactly one clear mode: `completed` or `all`"
        ));
    }

    let mut tasks = load_tasks(workspace_root)?;
    let removed = if completed_only {
        let removed = tasks
            .iter()
            .filter(|task| task.status == "completed")
            .cloned()
            .collect::<Vec<_>>();
        tasks.retain(|task| task.status != "completed");
        removed
    } else {
        let removed = tasks.clone();
        tasks.clear();
        removed
    };
    save_tasks(workspace_root, &tasks)?;
    Ok(removed)
}

pub fn format_task_list(tasks: &[TaskItem]) -> String {
    if tasks.is_empty() {
        return "No tasks found.".to_string();
    }

    let mut lines = Vec::with_capacity(tasks.len() + 1);
    lines.push("task_id\tstatus\tpriority\tcontent".to_string());
    for task in tasks {
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            task.id,
            task.status,
            task.priority.as_deref().unwrap_or("-"),
            task.content
        ));
    }
    lines.join("\n")
}

fn next_task_id(tasks: &[TaskItem]) -> String {
    let next = tasks
        .iter()
        .filter_map(|task| task.id.strip_prefix("task-"))
        .filter_map(|suffix| suffix.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    format!("task-{next}")
}

pub fn validate_task_status(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "pending" | "in_progress" | "completed" | "cancelled" => Ok(normalized),
        _ => Err(anyhow!(
            "unsupported task status `{value}`; use pending, in_progress, completed, or cancelled"
        )),
    }
}

fn normalize_status(value: Option<&str>) -> Option<String> {
    let value = value?.trim().to_ascii_lowercase().replace('-', "_");
    match value.as_str() {
        "pending" | "in_progress" | "completed" | "cancelled" => Some(value),
        _ => None,
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_required_text(value: String, label: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(anyhow!("{label} cannot be empty"))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        add_task, clear_tasks, format_task_list, get_task, load_tasks, remove_task, save_tasks,
        stop_task, transition_task, update_task, TaskItem, TaskPatch, TaskTransition,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-cli-tasks-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn add_task_assigns_incrementing_ids() {
        let root = temp_root();
        let first = add_task(&root, String::from("write docs"), None, None).expect("add first");
        let second = add_task(
            &root,
            String::from("verify docs"),
            Some(String::from("high")),
            Some(String::from("verify command outputs")),
        )
        .expect("add second");

        assert_eq!(first.id, "task-1");
        assert_eq!(second.id, "task-2");
        assert_eq!(second.priority.as_deref(), Some("high"));
        assert_eq!(
            second.description.as_deref(),
            Some("verify command outputs")
        );
    }

    #[test]
    fn transition_and_remove_task_persist_changes() {
        let root = temp_root();
        save_tasks(
            &root,
            &[TaskItem {
                id: String::from("task-1"),
                content: String::from("ship feature"),
                status: String::from("pending"),
                priority: None,
                description: None,
                output: None,
            }],
        )
        .expect("save");

        let updated = transition_task(&root, "task-1", TaskTransition::Start).expect("start");
        assert_eq!(updated.status, "in_progress");

        let removed = remove_task(&root, "task-1").expect("remove");
        assert_eq!(removed.id, "task-1");
        assert!(load_tasks(&root).expect("load").is_empty());
    }

    #[test]
    fn clear_completed_removes_only_completed_items() {
        let root = temp_root();
        save_tasks(
            &root,
            &[
                TaskItem {
                    id: String::from("task-1"),
                    content: String::from("keep"),
                    status: String::from("pending"),
                    priority: None,
                    description: None,
                    output: None,
                },
                TaskItem {
                    id: String::from("task-2"),
                    content: String::from("drop"),
                    status: String::from("completed"),
                    priority: None,
                    description: None,
                    output: None,
                },
            ],
        )
        .expect("save");

        let removed = clear_tasks(&root, true, false).expect("clear completed");
        assert_eq!(removed.len(), 1);
        let rendered = format_task_list(&load_tasks(&root).expect("load"));
        assert!(rendered.contains("task-1"));
        assert!(!rendered.contains("task-2"));
    }

    #[test]
    fn update_show_and_stop_task_roundtrip() {
        let root = temp_root();
        save_tasks(
            &root,
            &[TaskItem {
                id: String::from("task-1"),
                content: String::from("ship feature"),
                status: String::from("pending"),
                priority: None,
                description: None,
                output: None,
            }],
        )
        .expect("save");

        let updated = update_task(
            &root,
            "task-1",
            TaskPatch {
                content: None,
                status: Some(String::from("in_progress")),
                priority: Some(Some(String::from("high"))),
                description: Some(Some(String::from("needs review"))),
                output: Some(Some(String::from("started work"))),
            },
        )
        .expect("update");
        assert_eq!(updated.status, "in_progress");
        assert_eq!(updated.priority.as_deref(), Some("high"));

        let shown = get_task(&root, "task-1").expect("show");
        assert_eq!(shown.description.as_deref(), Some("needs review"));

        let stopped = stop_task(&root, "task-1", Some(String::from("blocked"))).expect("stop");
        assert_eq!(stopped.status, "cancelled");
        assert_eq!(stopped.output.as_deref(), Some("blocked"));
    }
}
