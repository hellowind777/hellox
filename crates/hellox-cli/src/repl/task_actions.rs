use anyhow::Result;
use hellox_agent::AgentSession;

use crate::task_panel::render_task_panel;
use crate::tasks::{
    add_task, clear_tasks, format_task_list, get_task, load_tasks, remove_task, stop_task,
    transition_task, update_task, TaskPatch, TaskTransition,
};

use super::commands::TaskCommand;

pub(super) fn handle_task_command(command: TaskCommand, session: &AgentSession) -> Result<String> {
    match command {
        TaskCommand::List => Ok(match load_tasks(session.working_directory()) {
            Ok(tasks) => format_task_list(&tasks),
            Err(error) => format!("Unable to inspect tasks: {error}"),
        }),
        TaskCommand::Panel { task_id } => Ok(match render_task_panel(
            session.working_directory(),
            task_id.as_deref(),
        ) {
            Ok(panel) => panel,
            Err(error) => format!("Unable to render task panel: {error}"),
        }),
        TaskCommand::Add { content: None } => Ok("Usage: /tasks add <content>".to_string()),
        TaskCommand::Add {
            content: Some(content),
        } => {
            let task = add_task(session.working_directory(), content, None, None)?;
            Ok(format!("Added task `{}`.", task.id))
        }
        TaskCommand::Show { task_id: None } => Ok("Usage: /tasks show <task-id>".to_string()),
        TaskCommand::Show {
            task_id: Some(task_id),
        } => {
            let task = get_task(session.working_directory(), &task_id)?;
            Ok(format_task_detail(&task))
        }
        TaskCommand::Update { task_id: None, .. } => {
            Ok("Usage: /tasks update <task-id> [--content <text>] [--priority <value>] [--description <text>] [--status <value>] [--output <text>] [--clear-priority] [--clear-description] [--clear-output]".to_string())
        }
        TaskCommand::Update {
            task_id: Some(task_id),
            content,
            priority,
            clear_priority,
            description,
            clear_description,
            status,
            output,
            clear_output,
        } => {
            let patch = TaskPatch {
                content,
                status,
                priority: merge_optional_field(priority, clear_priority),
                description: merge_optional_field(description, clear_description),
                output: merge_optional_field(output, clear_output),
            };
            if patch.is_empty() {
                return Ok("Usage: /tasks update <task-id> [--content <text>] [--priority <value>] [--description <text>] [--status <value>] [--output <text>] [--clear-priority] [--clear-description] [--clear-output]".to_string());
            }
            let task = update_task(session.working_directory(), &task_id, patch)?;
            Ok(format!("Updated task `{}`.\n{}", task.id, format_task_detail(&task)))
        }
        TaskCommand::Output { task_id: None } => Ok("Usage: /tasks output <task-id>".to_string()),
        TaskCommand::Output {
            task_id: Some(task_id),
        } => {
            let task = get_task(session.working_directory(), &task_id)?;
            Ok(format!(
                "task_id: {}\nstatus: {}\noutput: {}",
                task.id,
                task.status,
                task.output.as_deref().unwrap_or("(none)")
            ))
        }
        TaskCommand::Stop { task_id: None, .. } => {
            Ok("Usage: /tasks stop <task-id> [reason]".to_string())
        }
        TaskCommand::Stop {
            task_id: Some(task_id),
            reason,
        } => {
            let task = stop_task(session.working_directory(), &task_id, reason)?;
            Ok(format!("Stopped task `{}`.\n{}", task.id, format_task_detail(&task)))
        }
        TaskCommand::Start { task_id: None } => Ok("Usage: /tasks start <task-id>".to_string()),
        TaskCommand::Start {
            task_id: Some(task_id),
        } => {
            let task =
                transition_task(session.working_directory(), &task_id, TaskTransition::Start)?;
            Ok(format!("Marked task `{}` as `{}`.", task.id, task.status))
        }
        TaskCommand::Done { task_id: None } => Ok("Usage: /tasks done <task-id>".to_string()),
        TaskCommand::Done {
            task_id: Some(task_id),
        } => {
            let task = transition_task(
                session.working_directory(),
                &task_id,
                TaskTransition::Complete,
            )?;
            Ok(format!("Marked task `{}` as `{}`.", task.id, task.status))
        }
        TaskCommand::Cancel { task_id: None } => Ok("Usage: /tasks cancel <task-id>".to_string()),
        TaskCommand::Cancel {
            task_id: Some(task_id),
        } => {
            let task = transition_task(
                session.working_directory(),
                &task_id,
                TaskTransition::Cancel,
            )?;
            Ok(format!("Marked task `{}` as `{}`.", task.id, task.status))
        }
        TaskCommand::Remove { task_id: None } => Ok("Usage: /tasks remove <task-id>".to_string()),
        TaskCommand::Remove {
            task_id: Some(task_id),
        } => {
            let task = remove_task(session.working_directory(), &task_id)?;
            Ok(format!("Removed task `{}`.", task.id))
        }
        TaskCommand::Clear { target: None } => {
            Ok("Usage: /tasks clear <completed|all>".to_string())
        }
        TaskCommand::Clear {
            target: Some(target),
        } => {
            let removed = match target.to_ascii_lowercase().as_str() {
                "completed" => clear_tasks(session.working_directory(), true, false)?,
                "all" => clear_tasks(session.working_directory(), false, true)?,
                _ => return Ok("Usage: /tasks clear <completed|all>".to_string()),
            };
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

fn merge_optional_field(value: Option<String>, clear: bool) -> Option<Option<String>> {
    if clear {
        Some(None)
    } else {
        value.map(Some)
    }
}
