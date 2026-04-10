use hellox_memory::{relative_age_text, MemoryEntry};
use hellox_tui::{render_selector, SelectorEntry};

pub(super) fn render_memory_selector(
    entries: &[MemoryEntry],
    limit: usize,
    archived: bool,
) -> Vec<String> {
    let entries = entries
        .iter()
        .take(limit)
        .map(|entry| build_memory_entry(entry, archived))
        .collect::<Vec<_>>();
    render_selector(&entries)
}

pub(super) fn render_memory_lens(
    entry: Option<&MemoryEntry>,
    memory_id: &str,
    markdown: &str,
    archived: bool,
) -> Vec<String> {
    let scope_badge = entry.map(|entry| entry.scope.as_str().to_ascii_uppercase());
    let archived_flag = if archived { " --archived" } else { "" };
    let mut lines = vec![
        format!("scope: {}", entry_scope(entry)),
        format!("age: {}", entry_age(entry)),
        format!("updated_at: {}", entry_timestamp(entry)),
        format!("path: {}", entry_path(entry)),
        format!("markdown_lines: {}", markdown.lines().count()),
        format!("markdown_chars: {}", markdown.chars().count()),
        format!("show: `hellox memory show {memory_id}{archived_flag}`"),
        format!("search: `hellox memory search \"{memory_id}\" --limit 10{archived_flag}`"),
    ];
    if let Some(title) = first_heading(markdown) {
        lines.insert(4, format!("heading: {}", preview_text(title, 80)));
    }

    let entry = match scope_badge {
        Some(scope_badge) => SelectorEntry::new(memory_id.to_string(), lines)
            .with_badge(scope_badge)
            .selected(true),
        None => SelectorEntry::new(memory_id.to_string(), lines).selected(true),
    };
    render_selector(&[entry])
}

fn build_memory_entry(entry: &MemoryEntry, archived: bool) -> SelectorEntry {
    let panel_command = if archived {
        format!("hellox memory panel --archived {}", entry.memory_id)
    } else {
        format!("hellox memory panel {}", entry.memory_id)
    };
    let show_command = if archived {
        format!("hellox memory show {} --archived", entry.memory_id)
    } else {
        format!("hellox memory show {}", entry.memory_id)
    };
    let lines = vec![
        format!("age: {}", relative_age_text(entry.updated_at)),
        format!("updated_at: {}", entry.updated_at),
        format!("path: {}", preview_text(&entry.path, 72)),
        format!("open: `{panel_command}`"),
        format!("show: `{show_command}`"),
    ];

    SelectorEntry::new(entry.memory_id.clone(), lines)
        .with_badge(entry.scope.as_str().to_ascii_uppercase())
}

fn entry_scope(entry: Option<&MemoryEntry>) -> &'static str {
    entry
        .map(|entry| entry.scope.as_str())
        .unwrap_or("(unknown)")
}

fn entry_age(entry: Option<&MemoryEntry>) -> String {
    entry
        .map(|entry| relative_age_text(entry.updated_at))
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn entry_timestamp(entry: Option<&MemoryEntry>) -> String {
    entry
        .map(|entry| entry.updated_at.to_string())
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn entry_path(entry: Option<&MemoryEntry>) -> String {
    entry
        .map(|entry| preview_text(&entry.path, 96))
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn first_heading(markdown: &str) -> Option<&str> {
    markdown
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('#') && line.chars().any(|ch| ch.is_alphanumeric()))
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
