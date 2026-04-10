use std::path::Path;

use anyhow::Result;
use hellox_tui::{render_panel, status_badge, KeyValueRow, PanelSection};

use crate::tasks::{get_task, load_tasks, todo_file_path, TaskItem};

#[path = "task_panel_selector.rs"]
mod selector;

use selector::{render_task_lens, render_task_selector};

pub(crate) fn render_task_panel(root: &Path, task_id: Option<&str>) -> Result<String> {
    let task_id = task_id.map(str::trim).filter(|value| !value.is_empty());
    match task_id {
        Some(task_id) => render_task_detail_panel(root, task_id),
        None => render_task_list_panel(root),
    }
}

fn render_task_list_panel(root: &Path) -> Result<String> {
    let tasks = load_tasks(root)?;
    let task_path = todo_file_path(root);

    let metadata = vec![
        KeyValueRow::new("path", normalize_path(&task_path)),
        KeyValueRow::new("tasks", tasks.len().to_string()),
        KeyValueRow::new("pending", count_status(&tasks, "pending").to_string()),
        KeyValueRow::new(
            "in_progress",
            count_status(&tasks, "in_progress").to_string(),
        ),
        KeyValueRow::new("completed", count_status(&tasks, "completed").to_string()),
        KeyValueRow::new("cancelled", count_status(&tasks, "cancelled").to_string()),
    ];

    let sections = vec![
        PanelSection::new("Task selector", render_task_selector(&tasks)),
        PanelSection::new("Action palette", task_list_cli_palette()),
        PanelSection::new("REPL palette", task_list_repl_palette()),
    ];

    Ok(render_panel("Tasks panel", &metadata, &sections))
}

fn render_task_detail_panel(root: &Path, task_id: &str) -> Result<String> {
    let task = get_task(root, task_id)?;
    let task_path = todo_file_path(root);

    let metadata = vec![
        KeyValueRow::new("path", normalize_path(&task_path)),
        KeyValueRow::new("task_id", task.id.clone()),
        KeyValueRow::new("status", status_badge(&task.status)),
        KeyValueRow::new("priority", task.priority.as_deref().unwrap_or("(none)")),
        KeyValueRow::new("content", preview_text(&task.content, 96)),
    ];

    let sections = vec![
        PanelSection::new("Task lens", render_task_lens(&task)),
        PanelSection::new(
            "Description",
            optional_multiline_section(task.description.as_deref()),
        ),
        PanelSection::new("Output", optional_multiline_section(task.output.as_deref())),
        PanelSection::new("Action palette", task_detail_cli_palette(&task)),
        PanelSection::new("REPL palette", task_detail_repl_palette(&task)),
    ];

    Ok(render_panel(
        &format!("Task detail panel: {}", task.id),
        &metadata,
        &sections,
    ))
}

fn task_list_cli_palette() -> Vec<String> {
    vec![
        "- add: `hellox tasks add \"<text>\"`".to_string(),
        "- open detail panel: `hellox tasks panel <task-id>`".to_string(),
        "- list (raw): `hellox tasks list`".to_string(),
        "- mark in progress: `hellox tasks start <task-id>`".to_string(),
        "- mark complete: `hellox tasks done <task-id>`".to_string(),
        "- cancel with reason: `hellox tasks stop <task-id> --reason \"<text>\"`".to_string(),
        "- update fields: `hellox tasks update <task-id> --status <value> --output \"<text>\"`"
            .to_string(),
    ]
}

fn task_list_repl_palette() -> Vec<String> {
    vec![
        "- add: `/tasks add <text>`".to_string(),
        "- open detail panel: `/tasks panel [task-id]`".to_string(),
        "- mark in progress: `/tasks start <task-id>`".to_string(),
        "- mark complete: `/tasks done <task-id>`".to_string(),
        "- cancel with reason: `/tasks stop <task-id> <reason>`".to_string(),
        "- update fields: `/tasks update <task-id> --status <value> --output <text>`".to_string(),
    ]
}

fn task_detail_cli_palette(task: &TaskItem) -> Vec<String> {
    vec![
        format!("- back to list: `hellox tasks panel`"),
        format!("- start: `hellox tasks start {}`", task.id),
        format!("- done: `hellox tasks done {}`", task.id),
        format!(
            "- cancel with reason: `hellox tasks stop {} --reason \"<text>\"`",
            task.id
        ),
        format!(
            "- set output: `hellox tasks update {} --output \"<text>\"`",
            task.id
        ),
        format!(
            "- clear output: `hellox tasks update {} --clear-output`",
            task.id
        ),
        format!("- remove: `hellox tasks remove {}`", task.id),
    ]
}

fn task_detail_repl_palette(task: &TaskItem) -> Vec<String> {
    vec![
        "- back to list: `/tasks panel`".to_string(),
        format!("- start: `/tasks start {}`", task.id),
        format!("- done: `/tasks done {}`", task.id),
        format!("- cancel: `/tasks stop {} <reason>`", task.id),
        format!(
            "- update output: `/tasks update {} --output <text>`",
            task.id
        ),
        format!("- remove: `/tasks remove {}`", task.id),
    ]
}

fn optional_multiline_section(value: Option<&str>) -> Vec<String> {
    match value.map(str::trim) {
        Some(text) if !text.is_empty() => text.lines().map(ToString::to_string).collect(),
        _ => Vec::new(),
    }
}

fn count_status(tasks: &[TaskItem], status: &str) -> usize {
    tasks
        .iter()
        .filter(|task| task.status.eq_ignore_ascii_case(status))
        .count()
}

fn preview_text(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let head = compact
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        format!("{head}...")
    }
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
