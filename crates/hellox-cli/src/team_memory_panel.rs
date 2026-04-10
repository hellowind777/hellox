use hellox_sync::{team_memory_snapshot_path, TeamMemoryEntry, TeamMemorySnapshot};
use hellox_tui::{render_panel, render_selector, KeyValueRow, PanelSection, SelectorEntry};

pub(crate) fn render_local_team_memory_panel(snapshot: &TeamMemorySnapshot) -> String {
    let metadata = vec![
        KeyValueRow::new("repo_id", snapshot.repo_id.clone()),
        KeyValueRow::new("entries", snapshot.entries.len().to_string()),
        KeyValueRow::new("exported_at", snapshot.exported_at.to_string()),
        KeyValueRow::new(
            "snapshot_path",
            normalize_path(&team_memory_snapshot_path(&snapshot.repo_id)),
        ),
    ];

    let sections = vec![
        PanelSection::new("Entry selector", render_entry_selector(snapshot)),
        PanelSection::new("Focused entry lens", render_focused_entry_lens(snapshot)),
        PanelSection::new("Action palette", local_action_palette(&snapshot.repo_id)),
    ];

    render_panel("Team memory panel", &metadata, &sections)
}

pub(crate) fn render_server_team_memory_panel(
    account_id: &str,
    snapshot: &TeamMemorySnapshot,
) -> String {
    let metadata = vec![
        KeyValueRow::new("account_id", account_id.to_string()),
        KeyValueRow::new("repo_id", snapshot.repo_id.clone()),
        KeyValueRow::new("entries", snapshot.entries.len().to_string()),
        KeyValueRow::new("exported_at", snapshot.exported_at.to_string()),
    ];

    let sections = vec![
        PanelSection::new("Entry selector", render_entry_selector(snapshot)),
        PanelSection::new("Focused entry lens", render_focused_entry_lens(snapshot)),
        PanelSection::new(
            "Action palette",
            server_action_palette(account_id, &snapshot.repo_id),
        ),
    ];

    render_panel("Server team memory panel", &metadata, &sections)
}

fn render_entry_selector(snapshot: &TeamMemorySnapshot) -> Vec<String> {
    let entries = sorted_entries(snapshot)
        .into_iter()
        .map(|(key, entry)| {
            SelectorEntry::new(
                key.to_string(),
                vec![
                    format!("updated_at: {}", entry.updated_at),
                    format!("content: {}", preview_text(&entry.content, 96)),
                    format!("chars: {}", entry.content.chars().count()),
                ],
            )
            .with_badge(entry.updated_at.to_string())
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

fn render_focused_entry_lens(snapshot: &TeamMemorySnapshot) -> Vec<String> {
    let Some((key, entry)) = latest_entry(snapshot) else {
        return vec!["(no team memory entries)".to_string()];
    };

    let lines = vec![
        format!("updated_at: {}", entry.updated_at),
        format!("content: {}", preview_text(&entry.content, 160)),
        format!("chars: {}", entry.content.chars().count()),
        format!(
            "update: `hellox sync team-memory-put {} {} \"<content>\"`",
            snapshot.repo_id, key
        ),
        format!(
            "remove: `hellox sync team-memory-remove {} {}`",
            snapshot.repo_id, key
        ),
    ];

    render_selector(&[SelectorEntry::new(key.to_string(), lines)
        .with_badge("LATEST")
        .selected(true)])
}

fn local_action_palette(repo_id: &str) -> Vec<String> {
    vec![
        format!("- show raw: `hellox sync team-memory-show {repo_id}`"),
        format!("- export: `hellox sync team-memory-export {repo_id} <path>`"),
        format!("- put entry: `hellox sync team-memory-put {repo_id} <key> <content>`"),
        format!("- remove entry: `hellox sync team-memory-remove {repo_id} <key>`"),
    ]
}

fn server_action_palette(account_id: &str, repo_id: &str) -> Vec<String> {
    vec![
        format!("- show raw: `hellox server team-memory-show {account_id} {repo_id}`"),
        format!("- inspect settings: `hellox server settings-show {account_id}`"),
        format!("- compare local sync: `hellox sync team-memory-show {repo_id}`"),
    ]
}

fn sorted_entries(snapshot: &TeamMemorySnapshot) -> Vec<(&str, &TeamMemoryEntry)> {
    let mut entries = snapshot
        .entries
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect::<Vec<_>>();
    entries.sort_by(|(left_key, left_value), (right_key, right_value)| {
        right_value
            .updated_at
            .cmp(&left_value.updated_at)
            .then_with(|| left_key.cmp(right_key))
    });
    entries
}

fn latest_entry(snapshot: &TeamMemorySnapshot) -> Option<(&str, &TeamMemoryEntry)> {
    sorted_entries(snapshot).into_iter().next()
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

fn normalize_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hellox_sync::{TeamMemoryEntry, TeamMemorySnapshot};

    use super::{render_local_team_memory_panel, render_server_team_memory_panel};

    fn snapshot() -> TeamMemorySnapshot {
        TeamMemorySnapshot {
            repo_id: String::from("repo-1"),
            exported_at: 42,
            entries: BTreeMap::from([
                (
                    String::from("architecture"),
                    TeamMemoryEntry {
                        content: String::from("Keep the product local-first and seam remote."),
                        updated_at: 20,
                    },
                ),
                (
                    String::from("workflow"),
                    TeamMemoryEntry {
                        content: String::from("Upgrade workflow and plan panels to selector+lens."),
                        updated_at: 30,
                    },
                ),
            ]),
        }
    }

    #[test]
    fn local_panel_renders_selector_lens_and_actions() {
        let text = render_local_team_memory_panel(&snapshot());
        assert!(text.contains("Team memory panel"));
        assert!(text.contains("== Entry selector =="));
        assert!(text.contains("== Focused entry lens =="));
        assert!(text.contains("hellox sync team-memory-show repo-1"));
        assert!(text.contains("> [1] workflow — LATEST"));
    }

    #[test]
    fn server_panel_renders_account_metadata_and_actions() {
        let text = render_server_team_memory_panel("account-1", &snapshot());
        assert!(text.contains("Server team memory panel"));
        assert!(text.contains("account_id"));
        assert!(text.contains("hellox server team-memory-show account-1 repo-1"));
        assert!(text.contains("hellox server settings-show account-1"));
    }
}
