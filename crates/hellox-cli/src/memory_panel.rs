use std::path::Path;

use anyhow::Result;
use hellox_memory::{
    list_archived_memories, list_memories, load_archived_memory, load_memory, relative_age_text,
};
use hellox_tui::{render_panel, KeyValueRow, PanelSection};

#[path = "memory_panel_selector.rs"]
mod selector;

use selector::{render_memory_lens, render_memory_selector};

pub(crate) fn render_memory_panel(
    root: &Path,
    archived: bool,
    memory_id: Option<&str>,
) -> Result<String> {
    let memory_id = memory_id.map(str::trim).filter(|value| !value.is_empty());
    match memory_id {
        Some(memory_id) => render_memory_detail_panel(root, memory_id, archived),
        None => render_memory_list_panel(root, archived),
    }
}

fn render_memory_list_panel(root: &Path, archived: bool) -> Result<String> {
    let entries = if archived {
        list_archived_memories(root)?
    } else {
        list_memories(root)?
    };
    let metadata = vec![
        KeyValueRow::new("root", normalize_path(root)),
        KeyValueRow::new("archived", archived.to_string()),
        KeyValueRow::new("memories", entries.len().to_string()),
    ];

    let sections = vec![
        PanelSection::new(
            "Memory selector",
            render_memory_selector(&entries, 20, archived),
        ),
        PanelSection::new("Action palette", memory_list_cli_palette()),
        PanelSection::new("REPL palette", memory_list_repl_palette()),
    ];

    Ok(render_panel("Memory panel", &metadata, &sections))
}

fn render_memory_detail_panel(root: &Path, memory_id: &str, archived: bool) -> Result<String> {
    let entries = if archived {
        list_archived_memories(root).unwrap_or_default()
    } else {
        list_memories(root).unwrap_or_default()
    };
    let entry = entries.iter().find(|entry| entry.memory_id == memory_id);
    let markdown = match if archived {
        load_archived_memory(root, memory_id)
    } else {
        load_memory(root, memory_id)
    } {
        Ok(markdown) => markdown,
        Err(error) => {
            return Ok(format!(
                "Unable to load memory `{memory_id}` under `{}`: {error}",
                normalize_path(root)
            ));
        }
    };

    let mut metadata = vec![
        KeyValueRow::new("root", normalize_path(root)),
        KeyValueRow::new("memory_id", memory_id.to_string()),
        KeyValueRow::new("archived", archived.to_string()),
    ];
    if let Some(entry) = entry {
        metadata.push(KeyValueRow::new("scope", entry.scope.as_str()));
        metadata.push(KeyValueRow::new("age", relative_age_text(entry.updated_at)));
        metadata.push(KeyValueRow::new("updated_at", entry.updated_at.to_string()));
        metadata.push(KeyValueRow::new("path", entry.path.clone()));
    }

    let sections = vec![
        PanelSection::new(
            "Memory lens",
            render_memory_lens(entry, memory_id, &markdown, archived),
        ),
        PanelSection::new(
            "Preview",
            markdown.lines().map(ToString::to_string).collect(),
        ),
        PanelSection::new(
            "Action palette",
            memory_detail_cli_palette(memory_id, archived),
        ),
        PanelSection::new(
            "REPL palette",
            memory_detail_repl_palette(memory_id, archived),
        ),
    ];

    Ok(render_panel(
        &format!("Memory inspect panel: {memory_id}"),
        &metadata,
        &sections,
    ))
}

fn memory_list_cli_palette() -> Vec<String> {
    vec![
        "- open one memory: `hellox memory panel <memory-id>`".to_string(),
        "- show archived list: `hellox memory panel --archived`".to_string(),
        "- show raw markdown: `hellox memory show <memory-id>`".to_string(),
        "- show archived markdown: `hellox memory show <memory-id> --archived`".to_string(),
        "- search: `hellox memory search \"<query>\" --limit 10`".to_string(),
        "- search archived: `hellox memory search \"<query>\" --limit 10 --archived`".to_string(),
        "- clusters: `hellox memory clusters --limit 200 --min-jaccard 0.18`".to_string(),
        "- clusters semantic: `hellox memory clusters --semantic --limit 200 --min-jaccard 0.18`"
            .to_string(),
        "- clusters archived: `hellox memory clusters --archived --limit 200 --min-jaccard 0.18`"
            .to_string(),
        "- prune preview: `hellox memory prune --scope all --older-than-days 30 --keep-latest 3`"
            .to_string(),
        "- prune apply: `hellox memory prune --scope all --older-than-days 30 --keep-latest 3 --apply`"
            .to_string(),
        "- archive preview: `hellox memory archive --scope all --older-than-days 30 --keep-latest 3`"
            .to_string(),
        "- archive apply: `hellox memory archive --scope all --older-than-days 30 --keep-latest 3 --apply`"
            .to_string(),
        "- decay preview: `hellox memory decay --scope all --older-than-days 180 --keep-latest 20 --max-summary-lines 24 --max-summary-chars 1600`"
            .to_string(),
        "- decay apply: `hellox memory decay --scope all --older-than-days 180 --keep-latest 20 --max-summary-lines 24 --max-summary-chars 1600 --apply`"
            .to_string(),
    ]
}

fn memory_list_repl_palette() -> Vec<String> {
    vec![
        "- open panel: `/memory panel [--archived] [memory-id]`".to_string(),
        "- list: `/memory list [--archived]`".to_string(),
        "- show: `/memory show [--archived] <memory-id>`".to_string(),
        "- search: `/memory search [--archived] <query>`".to_string(),
        "- clusters: `/memory clusters [--archived] [--limit <n>] [--semantic]`".to_string(),
        "- prune: `/memory prune --scope <all|session|project> --older-than-days <n> --keep-latest <n> [--apply]`"
            .to_string(),
        "- archive: `/memory archive --scope <all|session|project> --older-than-days <n> --keep-latest <n> [--apply]`"
            .to_string(),
        "- decay: `/memory decay --scope <all|session|project> --older-than-days <n> --keep-latest <n> --max-summary-lines <n> --max-summary-chars <n> [--apply]`"
            .to_string(),
        "- save: `/memory save [instructions]`".to_string(),
    ]
}

fn memory_detail_cli_palette(memory_id: &str, archived: bool) -> Vec<String> {
    let show_cmd = if archived {
        format!("hellox memory show {memory_id} --archived")
    } else {
        format!("hellox memory show {memory_id}")
    };
    let search_cmd = if archived {
        format!("hellox memory search \"{memory_id}\" --limit 10 --archived")
    } else {
        format!("hellox memory search \"{memory_id}\" --limit 10")
    };
    let back_cmd = if archived {
        "hellox memory panel --archived".to_string()
    } else {
        "hellox memory panel".to_string()
    };
    vec![
        format!("- back to list: `{back_cmd}`"),
        format!("- show raw markdown: `{show_cmd}`"),
        format!("- search around: `{search_cmd}`"),
    ]
}

fn memory_detail_repl_palette(memory_id: &str, archived: bool) -> Vec<String> {
    let show_cmd = if archived {
        format!("/memory show --archived {memory_id}")
    } else {
        format!("/memory show {memory_id}")
    };
    let search_cmd = if archived {
        format!("/memory search --archived {memory_id}")
    } else {
        format!("/memory search {memory_id}")
    };
    let back_cmd = if archived {
        "/memory panel --archived".to_string()
    } else {
        "/memory panel".to_string()
    };
    vec![
        format!("- back to list: `{back_cmd}`"),
        format!("- show: `{show_cmd}`"),
        format!("- search: `{search_cmd}`"),
    ]
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
