use std::path::Path;

use hellox_bridge::{BridgeSessionDetail, BridgeSessionSummary};
use hellox_server::{ServerSessionDetail, ServerSessionSummary};
use hellox_tui::{render_panel, render_table, KeyValueRow, PanelSection, Table};

pub(crate) fn render_local_assistant_list_panel(
    sessions_root: &Path,
    sessions: &[BridgeSessionSummary],
) -> String {
    let metadata = vec![
        KeyValueRow::new("viewer", "local_bridge"),
        KeyValueRow::new("sessions_root", normalize_path(sessions_root)),
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
        PanelSection::new(
            "Sessions",
            render_table(&build_local_session_table(sessions)),
        ),
        PanelSection::new("Action palette", local_list_cli_palette()),
        PanelSection::new("REPL palette", local_list_repl_palette()),
    ];

    render_panel("Assistant viewer panel", &metadata, &sections)
}

pub(crate) fn render_local_assistant_detail_panel(
    sessions_root: &Path,
    detail: &BridgeSessionDetail,
) -> String {
    let metadata = vec![
        KeyValueRow::new("viewer", "local_bridge"),
        KeyValueRow::new("sessions_root", normalize_path(sessions_root)),
        KeyValueRow::new("session_id", detail.summary.session_id.clone()),
        KeyValueRow::new("model", detail.summary.model.clone()),
        KeyValueRow::new("permission_mode", detail.summary.permission_mode.clone()),
        KeyValueRow::new(
            "working_directory",
            detail.summary.working_directory.clone(),
        ),
        KeyValueRow::new("shell", detail.shell_name.clone()),
        KeyValueRow::new("messages", detail.summary.message_count.to_string()),
        KeyValueRow::new("updated_at", detail.updated_at.to_string()),
    ];
    let sections = vec![
        PanelSection::new("Session lens", local_detail_lines(detail)),
        PanelSection::new("Transcript preview", local_transcript_lines(detail)),
        PanelSection::new(
            "Action palette",
            local_detail_cli_palette(&detail.summary.session_id),
        ),
        PanelSection::new(
            "REPL palette",
            local_detail_repl_palette(&detail.summary.session_id),
        ),
    ];

    render_panel(
        &format!("Assistant viewer: {}", detail.summary.session_id),
        &metadata,
        &sections,
    )
}

pub(crate) fn render_remote_assistant_list_panel(
    environment_name: &str,
    sessions: &[ServerSessionSummary],
) -> String {
    let metadata = vec![
        KeyValueRow::new("viewer", "remote_server"),
        KeyValueRow::new("environment", environment_name),
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
        PanelSection::new(
            "Sessions",
            render_table(&build_remote_session_table(sessions)),
        ),
        PanelSection::new("Action palette", remote_list_cli_palette(environment_name)),
        PanelSection::new("REPL palette", remote_list_repl_palette(environment_name)),
    ];

    render_panel(
        &format!("Assistant viewer panel: {environment_name}"),
        &metadata,
        &sections,
    )
}

pub(crate) fn render_remote_assistant_detail_panel(
    environment_name: &str,
    detail: &ServerSessionDetail,
) -> String {
    let metadata = vec![
        KeyValueRow::new("viewer", "remote_server"),
        KeyValueRow::new("environment", environment_name),
        KeyValueRow::new("session_id", detail.summary.session_id.clone()),
        KeyValueRow::new("model", detail.summary.model.clone()),
        KeyValueRow::new("source", detail.summary.source.clone()),
        KeyValueRow::new(
            "working_directory",
            detail.summary.working_directory.clone(),
        ),
        KeyValueRow::new("owner_account_id", detail.summary.owner_account_id.clone()),
        KeyValueRow::new(
            "owner_device_id",
            detail
                .summary
                .owner_device_id
                .as_deref()
                .unwrap_or("(none)"),
        ),
        KeyValueRow::new(
            "permission_mode",
            detail.permission_mode.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new("messages", detail.message_count.to_string()),
        KeyValueRow::new("persisted", yes_no(detail.summary.persisted)),
    ];
    let sections = vec![
        PanelSection::new("Session lens", remote_detail_lines(detail)),
        PanelSection::new(
            "Action palette",
            remote_detail_cli_palette(environment_name, &detail.summary.session_id),
        ),
        PanelSection::new(
            "REPL palette",
            remote_detail_repl_palette(environment_name, &detail.summary.session_id),
        ),
    ];

    render_panel(
        &format!(
            "Assistant viewer: {} @ {}",
            detail.summary.session_id, environment_name
        ),
        &metadata,
        &sections,
    )
}

fn build_local_session_table(sessions: &[BridgeSessionSummary]) -> Table {
    let rows = sessions
        .iter()
        .enumerate()
        .map(|(index, session)| {
            vec![
                (index + 1).to_string(),
                session.session_id.clone(),
                session.model.clone(),
                session.permission_mode.clone(),
                session.message_count.to_string(),
                session.updated_at.to_string(),
                preview_text(&session.working_directory, 28),
                format!("hellox assistant show {}", session.session_id),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "#".to_string(),
            "session".to_string(),
            "model".to_string(),
            "permissions".to_string(),
            "messages".to_string(),
            "updated_at".to_string(),
            "cwd".to_string(),
            "open".to_string(),
        ],
        rows,
    )
}

fn build_remote_session_table(sessions: &[ServerSessionSummary]) -> Table {
    let rows = sessions
        .iter()
        .enumerate()
        .map(|(index, session)| {
            vec![
                (index + 1).to_string(),
                session.session_id.clone(),
                session.model.clone(),
                preview_text(&session.working_directory, 24),
                session.source.clone(),
                session.owner_account_id.clone(),
                session.updated_at.to_string(),
                yes_no(session.persisted),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "#".to_string(),
            "session".to_string(),
            "model".to_string(),
            "cwd".to_string(),
            "source".to_string(),
            "owner".to_string(),
            "updated_at".to_string(),
            "persisted".to_string(),
        ],
        rows,
    )
}

fn local_detail_lines(detail: &BridgeSessionDetail) -> Vec<String> {
    vec![
        format!("created_at: {}", detail.created_at),
        format!("updated_at: {}", detail.updated_at),
        format!("system_prompt_preview: {}", detail.system_prompt_preview),
    ]
}

fn local_transcript_lines(detail: &BridgeSessionDetail) -> Vec<String> {
    if detail.transcript.is_empty() {
        vec!["(empty)".to_string()]
    } else {
        detail
            .transcript
            .iter()
            .map(|entry| format!("- {}: {}", entry.role, entry.preview))
            .collect()
    }
}

fn remote_detail_lines(detail: &ServerSessionDetail) -> Vec<String> {
    vec![
        format!(
            "owner_device_name: {}",
            detail.owner_device_name.as_deref().unwrap_or("(none)")
        ),
        format!(
            "shell_name: {}",
            detail.shell_name.as_deref().unwrap_or("(none)")
        ),
        format!(
            "system_prompt: {}",
            detail.system_prompt.as_deref().unwrap_or("(none)")
        ),
        format!("created_at: {}", detail.summary.created_at),
        format!("updated_at: {}", detail.summary.updated_at),
    ]
}

fn local_list_cli_palette() -> Vec<String> {
    vec![
        "- inspect session: `hellox assistant show <session-id>`".to_string(),
        "- bridge detail: `hellox bridge show <session-id>`".to_string(),
        "- session panel: `hellox session panel <session-id>`".to_string(),
    ]
}

fn local_list_repl_palette() -> Vec<String> {
    vec![
        "- inspect session: `/assistant show <session-id>`".to_string(),
        "- bridge detail: `/bridge show <session-id>`".to_string(),
        "- resume session: `/resume <session-id>`".to_string(),
    ]
}

fn local_detail_cli_palette(session_id: &str) -> Vec<String> {
    vec![
        "- back to list: `hellox assistant list`".to_string(),
        format!("- bridge detail: `hellox bridge show {session_id}`"),
        format!("- session panel: `hellox session panel {session_id}`"),
    ]
}

fn local_detail_repl_palette(session_id: &str) -> Vec<String> {
    vec![
        "- back to list: `/assistant`".to_string(),
        format!("- bridge detail: `/bridge show {session_id}`"),
        format!("- resume session: `/resume {session_id}`"),
    ]
}

fn remote_list_cli_palette(environment_name: &str) -> Vec<String> {
    vec![
        format!(
            "- inspect session: `hellox assistant show <session-id> --environment {environment_name}`"
        ),
        format!(
            "- direct-connect plan: `hellox teleport plan {environment_name} --session-id <session-id>`"
        ),
        format!(
            "- direct-connect launch: `hellox teleport connect {environment_name} --session-id <session-id>`"
        ),
    ]
}

fn remote_list_repl_palette(environment_name: &str) -> Vec<String> {
    vec![
        format!("- inspect session: `/assistant show <session-id> {environment_name}`"),
        format!("- direct-connect plan: `/teleport plan {environment_name} <session-id>`"),
        format!("- direct-connect launch: `/teleport connect {environment_name} <session-id>`"),
    ]
}

fn remote_detail_cli_palette(environment_name: &str, session_id: &str) -> Vec<String> {
    vec![
        format!("- back to list: `hellox assistant list --environment {environment_name}`"),
        format!(
            "- direct-connect plan: `hellox teleport plan {environment_name} --session-id {session_id}`"
        ),
        format!(
            "- direct-connect launch: `hellox teleport connect {environment_name} --session-id {session_id}`"
        ),
    ]
}

fn remote_detail_repl_palette(environment_name: &str, session_id: &str) -> Vec<String> {
    vec![
        format!("- back to list: `/assistant list {environment_name}`"),
        format!("- direct-connect plan: `/teleport plan {environment_name} {session_id}`"),
        format!("- direct-connect launch: `/teleport connect {environment_name} {session_id}`"),
    ]
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn preview_text(value: &str, max_chars: usize) -> String {
    let value = value.trim();
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut preview = value
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>();
        preview.push('…');
        preview
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "YES".to_string()
    } else {
        "NO".to_string()
    }
}
