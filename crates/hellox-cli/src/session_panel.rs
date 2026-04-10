use std::path::Path;

use anyhow::Result;
use hellox_agent::StoredSessionSnapshot;
use hellox_tui::{render_panel, render_table, KeyValueRow, PanelSection, Table};

use crate::sessions::{list_sessions, load_session};
use crate::style_command_support::normalize_path;

#[path = "session_panel_selector.rs"]
mod selector;

use selector::{render_session_lens, render_session_selector};

pub(crate) fn render_session_panel(root: &Path, session_id: Option<&str>) -> Result<String> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    match session_id {
        Some(session_id) => render_session_detail_panel(root, session_id),
        None => render_session_list_panel(root),
    }
}

fn render_session_list_panel(root: &Path) -> Result<String> {
    let sessions = list_sessions(root)?;
    let metadata = vec![
        KeyValueRow::new("sessions_root", normalize_path(root)),
        KeyValueRow::new("sessions", sessions.len().to_string()),
        KeyValueRow::new(
            "latest_updated_at",
            sessions
                .first()
                .map(|session| session.updated_at.to_string())
                .unwrap_or_else(|| "(none)".to_string()),
        ),
    ];
    let sections = vec![
        PanelSection::new("Session selector", render_session_selector(&sessions)),
        PanelSection::new("Action palette", session_list_cli_palette()),
        PanelSection::new("REPL palette", session_list_repl_palette()),
    ];

    Ok(render_panel("Session panel", &metadata, &sections))
}

fn render_session_detail_panel(root: &Path, session_id: &str) -> Result<String> {
    let snapshot = load_session(root, session_id)?;
    let metadata = vec![
        KeyValueRow::new("sessions_root", normalize_path(root)),
        KeyValueRow::new("session_id", snapshot.session_id.clone()),
        KeyValueRow::new("model", snapshot.model.clone()),
        KeyValueRow::new(
            "permission_mode",
            snapshot
                .permission_mode
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "(from current config)".to_string()),
        ),
        KeyValueRow::new(
            "output_style",
            snapshot.output_style_name.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new(
            "persona",
            snapshot
                .persona
                .as_ref()
                .map(|persona| persona.name.as_str())
                .unwrap_or("(none)"),
        ),
        KeyValueRow::new("prompt_fragments", render_prompt_fragments(&snapshot)),
        KeyValueRow::new("plan_mode", yes_no(snapshot.planning.active)),
        KeyValueRow::new("plan_steps", snapshot.planning.plan.len().to_string()),
        KeyValueRow::new(
            "working_directory",
            snapshot.working_directory.replace('\\', "/"),
        ),
        KeyValueRow::new("shell", snapshot.shell_name.clone()),
        KeyValueRow::new("messages", snapshot.messages.len().to_string()),
        KeyValueRow::new("requests", snapshot.total_requests().to_string()),
        KeyValueRow::new("input_tokens", snapshot.total_input_tokens().to_string()),
        KeyValueRow::new("output_tokens", snapshot.total_output_tokens().to_string()),
    ];
    let sections = vec![
        PanelSection::new("Session lens", render_session_lens(&snapshot)),
        PanelSection::new("Timeline", timeline_lines(&snapshot)),
        PanelSection::new("Usage by model", render_table(&usage_table(&snapshot))),
        PanelSection::new("Action palette", session_detail_cli_palette(session_id)),
        PanelSection::new("REPL palette", session_detail_repl_palette(session_id)),
    ];

    Ok(render_panel(
        &format!("Session panel: {session_id}"),
        &metadata,
        &sections,
    ))
}

fn usage_table(snapshot: &StoredSessionSnapshot) -> Table {
    let rows = snapshot
        .usage_by_model
        .iter()
        .map(|(model, totals)| {
            vec![
                model.clone(),
                totals.requests.to_string(),
                totals.input_tokens.to_string(),
                totals.output_tokens.to_string(),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "model".to_string(),
            "requests".to_string(),
            "input_tokens".to_string(),
            "output_tokens".to_string(),
        ],
        rows,
    )
}

fn timeline_lines(snapshot: &StoredSessionSnapshot) -> Vec<String> {
    vec![
        format!("created_at: {}", snapshot.created_at),
        format!("updated_at: {}", snapshot.updated_at),
        format!(
            "config_path: {}",
            snapshot.config_path.as_deref().unwrap_or("(none)")
        ),
    ]
}

fn session_list_cli_palette() -> Vec<String> {
    vec![
        "- open panel: `hellox session panel <session-id>`".to_string(),
        "- show raw: `hellox session show <session-id>`".to_string(),
        "- share: `hellox session share <session-id> --output <path>`".to_string(),
        "- compact: `hellox session compact <session-id>`".to_string(),
    ]
}

fn session_list_repl_palette() -> Vec<String> {
    vec![
        "- open panel: `/session panel [session-id]`".to_string(),
        "- show raw: `/session show <session-id>`".to_string(),
        "- share: `/session share <session-id> [path]`".to_string(),
        "- resume: `/resume <session-id>`".to_string(),
    ]
}

fn session_detail_cli_palette(session_id: &str) -> Vec<String> {
    vec![
        "- back to list: `hellox session panel`".to_string(),
        format!("- show raw: `hellox session show {session_id}`"),
        format!("- share: `hellox session share {session_id} --output <path>`"),
        format!("- compact: `hellox session compact {session_id}`"),
    ]
}

fn session_detail_repl_palette(session_id: &str) -> Vec<String> {
    vec![
        "- back to list: `/session panel`".to_string(),
        format!("- show raw: `/session show {session_id}`"),
        format!("- share: `/session share {session_id} [path]`"),
        format!("- resume: `/resume {session_id}`"),
    ]
}

fn render_prompt_fragments(snapshot: &StoredSessionSnapshot) -> String {
    if snapshot.prompt_fragments.is_empty() {
        "(none)".to_string()
    } else {
        snapshot
            .prompt_fragments
            .iter()
            .map(|fragment| fragment.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "YES".to_string()
    } else {
        "NO".to_string()
    }
}
