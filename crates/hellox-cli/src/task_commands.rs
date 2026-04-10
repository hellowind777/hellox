use std::env;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::cli_task_types::TaskCommands;
use crate::task_panel::render_task_panel;
use crate::tasks::{
    add_task, clear_tasks, format_task_list, get_task, load_tasks, remove_task, stop_task,
    transition_task, update_task, validate_task_status, TaskPatch, TaskTransition,
};

pub fn handle_tasks_command(command: TaskCommands) -> Result<()> {
    println!("{}", task_command_text(command)?);
    Ok(())
}

pub(crate) fn task_command_text(command: TaskCommands) -> Result<String> {
    match command {
        TaskCommands::Panel { task_id, cwd } => {
            let root = workspace_root(cwd)?;
            render_task_panel(&root, task_id.as_deref())
        }
        TaskCommands::List { status, limit, cwd } => {
            let root = workspace_root(cwd)?;
            let mut tasks = load_tasks(&root)?;
            if let Some(status) = status {
                let status = validate_task_status(&status)?;
                tasks.retain(|task| task.status == status);
            }
            if let Some(limit) = limit {
                tasks.truncate(limit);
            }
            Ok(format_task_list(&tasks))
        }
        TaskCommands::Show { task_id, cwd } => {
            let root = workspace_root(cwd)?;
            Ok(format_task_detail(&get_task(&root, &task_id)?))
        }
        TaskCommands::Add {
            content,
            priority,
            description,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let task = add_task(&root, content, priority, description)?;
            Ok(format!(
                "Added task `{}` at `{}`.\n{}",
                task.id,
                normalize_path(&root.join(".hellox").join("todos.json")),
                format_task_detail(&task),
            ))
        }
        TaskCommands::Update {
            task_id,
            content,
            priority,
            clear_priority,
            description,
            clear_description,
            status,
            output,
            clear_output,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let patch = TaskPatch {
                content,
                status,
                priority: merge_optional_field(priority, clear_priority),
                description: merge_optional_field(description, clear_description),
                output: merge_optional_field(output, clear_output),
            };
            if patch.is_empty() {
                return Err(anyhow!("tasks update requires at least one field change"));
            }
            let task = update_task(&root, &task_id, patch)?;
            Ok(format!(
                "Updated task `{}`.\n{}",
                task.id,
                format_task_detail(&task)
            ))
        }
        TaskCommands::Output { task_id, cwd } => {
            let root = workspace_root(cwd)?;
            Ok(format_task_output(&get_task(&root, &task_id)?))
        }
        TaskCommands::Stop {
            task_id,
            reason,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let task = stop_task(&root, &task_id, reason)?;
            Ok(format!(
                "Stopped task `{}`.\n{}",
                task.id,
                format_task_detail(&task)
            ))
        }
        TaskCommands::Start { task_id, cwd } => {
            let root = workspace_root(cwd)?;
            let task = transition_task(&root, &task_id, TaskTransition::Start)?;
            Ok(format!("Marked task `{}` as `{}`.", task.id, task.status))
        }
        TaskCommands::Done { task_id, cwd } => {
            let root = workspace_root(cwd)?;
            let task = transition_task(&root, &task_id, TaskTransition::Complete)?;
            Ok(format!("Marked task `{}` as `{}`.", task.id, task.status))
        }
        TaskCommands::Cancel { task_id, cwd } => {
            let root = workspace_root(cwd)?;
            let task = transition_task(&root, &task_id, TaskTransition::Cancel)?;
            Ok(format!("Marked task `{}` as `{}`.", task.id, task.status))
        }
        TaskCommands::Remove { task_id, cwd } => {
            let root = workspace_root(cwd)?;
            let task = remove_task(&root, &task_id)?;
            Ok(format!("Removed task `{}`.", task.id))
        }
        TaskCommands::Clear {
            completed,
            all,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let removed = clear_tasks(&root, completed, all)?;
            Ok(format!("Removed {} task(s).", removed.len()))
        }
    }
}

fn format_task_detail(task: &crate::tasks::TaskItem) -> String {
    format!(
        "task_id: {}\nstatus: {}\npriority: {}\ncontent: {}\ndescription: {}\noutput: {}",
        task.id,
        task.status,
        task.priority.as_deref().unwrap_or("(none)"),
        task.content,
        task.description.as_deref().unwrap_or("(none)"),
        task.output.as_deref().unwrap_or("(none)")
    )
}

fn format_task_output(task: &crate::tasks::TaskItem) -> String {
    format!(
        "task_id: {}\nstatus: {}\noutput: {}",
        task.id,
        task.status,
        task.output.as_deref().unwrap_or("(none)")
    )
}

fn merge_optional_field(value: Option<String>, clear: bool) -> Option<Option<String>> {
    if clear {
        Some(None)
    } else {
        value.map(Some)
    }
}

fn workspace_root(value: Option<PathBuf>) -> Result<PathBuf> {
    Ok(match value {
        Some(path) => path,
        None => env::current_dir()?,
    })
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::task_command_text;
    use crate::cli_task_types::TaskCommands;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-task-command-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn task_update_and_output_commands_roundtrip() {
        let root = temp_dir();

        task_command_text(TaskCommands::Add {
            content: "Review local workflow surface".to_string(),
            priority: Some("high".to_string()),
            description: Some("Focus on task UI parity".to_string()),
            cwd: Some(root.clone()),
        })
        .expect("add task");

        let updated = task_command_text(TaskCommands::Update {
            task_id: "task-1".to_string(),
            content: None,
            priority: None,
            clear_priority: false,
            description: None,
            clear_description: true,
            status: Some("in_progress".to_string()),
            output: Some("Started implementation".to_string()),
            clear_output: false,
            cwd: Some(root.clone()),
        })
        .expect("update task");
        assert!(updated.contains("status: in_progress"));
        assert!(updated.contains("output: Started implementation"));
        assert!(updated.contains("description: (none)"));

        let output = task_command_text(TaskCommands::Output {
            task_id: "task-1".to_string(),
            cwd: Some(root),
        })
        .expect("task output");
        assert!(output.contains("Started implementation"));
    }

    #[test]
    fn task_stop_records_reason() {
        let root = temp_dir();

        task_command_text(TaskCommands::Add {
            content: "Ship docs".to_string(),
            priority: None,
            description: None,
            cwd: Some(root.clone()),
        })
        .expect("add task");

        let stopped = task_command_text(TaskCommands::Stop {
            task_id: "task-1".to_string(),
            reason: Some("Blocked by host validation".to_string()),
            cwd: Some(root),
        })
        .expect("stop task");
        assert!(stopped.contains("status: cancelled"));
        assert!(stopped.contains("Blocked by host validation"));
    }
}
