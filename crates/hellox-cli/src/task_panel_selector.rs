use hellox_tui::{render_selector, status_badge, SelectorEntry};

use crate::tasks::TaskItem;

pub(super) fn render_task_selector(tasks: &[TaskItem]) -> Vec<String> {
    let entries = tasks.iter().map(build_task_entry).collect::<Vec<_>>();
    render_selector(&entries)
}

pub(super) fn render_task_lens(task: &TaskItem) -> Vec<String> {
    let lines = vec![
        format!("content: {}", preview_text(&task.content, 96)),
        format!("priority: {}", task.priority.as_deref().unwrap_or("(none)")),
        format!(
            "description: {}",
            preview_optional(task.description.as_deref(), 96)
        ),
        format!("output: {}", preview_optional(task.output.as_deref(), 96)),
        format!("content_chars: {}", task.content.chars().count()),
        format!("open: `hellox tasks panel {}`", task.id),
        format!("repl: `/tasks panel {}`", task.id),
    ];

    render_selector(&[SelectorEntry::new(task.id.clone(), lines)
        .with_badge(status_badge(&task.status))
        .selected(true)])
}

fn build_task_entry(task: &TaskItem) -> SelectorEntry {
    let lines = vec![
        format!("content: {}", preview_text(&task.content, 72)),
        format!("priority: {}", task.priority.as_deref().unwrap_or("(none)")),
        format!(
            "description: {}",
            preview_optional(task.description.as_deref(), 72)
        ),
        format!("output: {}", preview_optional(task.output.as_deref(), 72)),
        format!("open: `hellox tasks panel {}`", task.id),
        format!("repl: `/tasks panel {}`", task.id),
    ];

    SelectorEntry::new(task.id.clone(), lines).with_badge(status_badge(&task.status))
}

fn preview_optional(value: Option<&str>, max_chars: usize) -> String {
    value
        .map(|value| preview_text(value, max_chars))
        .unwrap_or_else(|| "(none)".to_string())
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
