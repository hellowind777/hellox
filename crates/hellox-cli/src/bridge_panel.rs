use std::path::Path;

use anyhow::Result;
use hellox_bridge::{
    inspect_bridge_status, inspect_ide_status, list_bridge_sessions, load_bridge_session,
    BridgeRuntimePaths, BridgeSessionDetail, BridgeSessionSummary,
};
use hellox_tui::{
    render_panel, render_selector, render_table, KeyValueRow, PanelSection, SelectorEntry, Table,
};

use crate::style_command_support::normalize_path;

pub(crate) fn render_bridge_panel(
    paths: &BridgeRuntimePaths,
    session_id: Option<&str>,
) -> Result<String> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    match session_id {
        Some(session_id) => render_bridge_detail_panel(paths, session_id),
        None => render_bridge_list_panel(paths),
    }
}

pub(crate) fn bridge_panel_session_ids(paths: &BridgeRuntimePaths) -> Result<Vec<String>> {
    Ok(list_bridge_sessions(paths)?
        .into_iter()
        .map(|session| session.session_id)
        .collect())
}

pub(crate) fn render_ide_panel(paths: &BridgeRuntimePaths) -> Result<String> {
    let status = inspect_ide_status(paths)?;
    let metadata = vec![
        KeyValueRow::new("viewer", "ide_bridge"),
        KeyValueRow::new(
            "latest_session",
            status
                .latest_session
                .as_ref()
                .map(|session| session.session_id.as_str())
                .unwrap_or("(none)"),
        ),
        KeyValueRow::new(
            "persisted_sessions",
            status.bridge.persisted_sessions.to_string(),
        ),
        KeyValueRow::new(
            "enabled_mcp_servers",
            format!(
                "{}/{}",
                status.bridge.enabled_mcp_servers, status.bridge.configured_mcp_servers
            ),
        ),
        KeyValueRow::new(
            "enabled_plugins",
            format!(
                "{}/{}",
                status.bridge.enabled_plugins, status.bridge.installed_plugins
            ),
        ),
    ];
    let sections = vec![
        PanelSection::new("Bridge summary", bridge_summary_lines(&status.bridge)),
        PanelSection::new(
            "Latest session lens",
            ide_latest_session_lens(status.latest_session.as_ref()),
        ),
        PanelSection::new(
            "Action palette",
            ide_cli_palette(status.latest_session.as_ref()),
        ),
        PanelSection::new(
            "REPL palette",
            ide_repl_palette(status.latest_session.as_ref()),
        ),
    ];

    Ok(render_panel("IDE panel", &metadata, &sections))
}

fn render_bridge_list_panel(paths: &BridgeRuntimePaths) -> Result<String> {
    let status = inspect_bridge_status(paths)?;
    let sessions = list_bridge_sessions(paths)?;
    let metadata = vec![
        KeyValueRow::new("viewer", "local_bridge"),
        KeyValueRow::new("config_path", status.config_path.clone()),
        KeyValueRow::new("sessions_root", status.sessions_root.clone()),
        KeyValueRow::new("plugins_root", status.plugins_root.clone()),
        KeyValueRow::new("persisted_sessions", status.persisted_sessions.to_string()),
    ];
    let sections = vec![
        PanelSection::new("Bridge summary", bridge_summary_lines(&status)),
        PanelSection::new("Session selector", render_bridge_selector(&sessions)),
        PanelSection::new("Action palette", bridge_list_cli_palette()),
        PanelSection::new("REPL palette", bridge_list_repl_palette()),
    ];

    Ok(render_panel("Bridge panel", &metadata, &sections))
}

fn render_bridge_detail_panel(paths: &BridgeRuntimePaths, session_id: &str) -> Result<String> {
    let detail = load_bridge_session(paths, session_id)?;
    let metadata = vec![
        KeyValueRow::new("viewer", "local_bridge"),
        KeyValueRow::new(
            "sessions_root",
            normalize_path(Path::new(&paths.sessions_root)),
        ),
        KeyValueRow::new("session_id", detail.summary.session_id.clone()),
        KeyValueRow::new("model", detail.summary.model.clone()),
        KeyValueRow::new("permission_mode", detail.summary.permission_mode.clone()),
        KeyValueRow::new(
            "working_directory",
            detail.summary.working_directory.clone(),
        ),
        KeyValueRow::new("shell", detail.shell_name.clone()),
        KeyValueRow::new("messages", detail.summary.message_count.to_string()),
    ];
    let sections = vec![
        PanelSection::new("Bridge session lens", render_bridge_lens(&detail)),
        PanelSection::new(
            "Transcript",
            render_table(&bridge_transcript_table(&detail)),
        ),
        PanelSection::new("Action palette", bridge_detail_cli_palette(session_id)),
        PanelSection::new("REPL palette", bridge_detail_repl_palette(session_id)),
    ];

    Ok(render_panel(
        &format!("Bridge panel: {session_id}"),
        &metadata,
        &sections,
    ))
}

fn bridge_summary_lines(status: &hellox_bridge::BridgeStatus) -> Vec<String> {
    vec![
        "mode: local_bridge".to_string(),
        format!("configured_mcp_servers: {}", status.configured_mcp_servers),
        format!("enabled_mcp_servers: {}", status.enabled_mcp_servers),
        format!("installed_plugins: {}", status.installed_plugins),
        format!("enabled_plugins: {}", status.enabled_plugins),
    ]
}

fn render_bridge_selector(sessions: &[BridgeSessionSummary]) -> Vec<String> {
    let entries = sessions
        .iter()
        .map(|session| {
            SelectorEntry::new(
                session.session_id.clone(),
                vec![
                    format!("model: {}", session.model),
                    format!("permission_mode: {}", session.permission_mode),
                    format!("messages: {}", session.message_count),
                    format!("updated_at: {}", session.updated_at),
                    format!("cwd: {}", session.working_directory),
                    format!("focus: `hellox bridge panel {}`", session.session_id),
                ],
            )
            .with_badge(session.model.clone())
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

fn render_bridge_lens(detail: &BridgeSessionDetail) -> Vec<String> {
    render_selector(&[SelectorEntry::new(
        detail.summary.session_id.clone(),
        vec![
            format!("model: {}", detail.summary.model),
            format!("permission_mode: {}", detail.summary.permission_mode),
            format!("working_directory: {}", detail.summary.working_directory),
            format!("shell: {}", detail.shell_name),
            format!("created_at: {}", detail.created_at),
            format!("updated_at: {}", detail.updated_at),
            format!("system_prompt_preview: {}", detail.system_prompt_preview),
        ],
    )
    .with_badge(detail.summary.model.clone())
    .selected(true)])
}

fn bridge_transcript_table(detail: &BridgeSessionDetail) -> Table {
    let rows = detail
        .transcript
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            vec![
                (index + 1).to_string(),
                entry.role.clone(),
                entry.preview.clone(),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec!["#".to_string(), "role".to_string(), "preview".to_string()],
        rows,
    )
}

fn ide_latest_session_lens(session: Option<&BridgeSessionSummary>) -> Vec<String> {
    match session {
        Some(session) => render_selector(&[SelectorEntry::new(
            session.session_id.clone(),
            vec![
                format!("model: {}", session.model),
                format!("permission_mode: {}", session.permission_mode),
                format!("messages: {}", session.message_count),
                format!("updated_at: {}", session.updated_at),
                format!("cwd: {}", session.working_directory),
                format!("open: `hellox bridge panel {}`", session.session_id),
            ],
        )
        .with_badge(session.model.clone())
        .selected(true)]),
        None => vec!["No persisted bridge sessions found.".to_string()],
    }
}

fn bridge_list_cli_palette() -> Vec<String> {
    vec![
        "- open panel: `hellox bridge panel <session-id>`".to_string(),
        "- raw status: `hellox bridge status`".to_string(),
        "- raw sessions: `hellox bridge sessions`".to_string(),
        "- assistant viewer: `hellox assistant list`".to_string(),
        "- ide overview: `hellox ide panel`".to_string(),
    ]
}

fn bridge_list_repl_palette() -> Vec<String> {
    vec![
        "- open panel: `/bridge panel [session-id]`".to_string(),
        "- raw status: `/bridge`".to_string(),
        "- raw sessions: `/bridge sessions`".to_string(),
        "- numeric focus: render `/bridge panel`, then enter `1..n`".to_string(),
        "- assistant viewer: `/assistant`".to_string(),
        "- ide overview: `/ide panel`".to_string(),
    ]
}

fn bridge_detail_cli_palette(session_id: &str) -> Vec<String> {
    vec![
        "- back to list: `hellox bridge panel`".to_string(),
        format!("- raw detail: `hellox bridge show-session {session_id}`"),
        format!("- assistant viewer: `hellox assistant show {session_id}`"),
        format!("- session panel: `hellox session panel {session_id}`"),
    ]
}

fn bridge_detail_repl_palette(session_id: &str) -> Vec<String> {
    vec![
        "- back to list: `/bridge panel`".to_string(),
        format!("- raw detail: `/bridge show {session_id}`"),
        format!("- assistant viewer: `/assistant show {session_id}`"),
        format!("- session panel: `/session panel {session_id}`"),
    ]
}

fn ide_cli_palette(session: Option<&BridgeSessionSummary>) -> Vec<String> {
    let mut palette = vec![
        "- bridge overview: `hellox bridge panel`".to_string(),
        "- raw status: `hellox ide status`".to_string(),
        "- assistant viewer: `hellox assistant list`".to_string(),
    ];
    if let Some(session) = session {
        palette.push(format!(
            "- inspect latest bridge session: `hellox bridge panel {}`",
            session.session_id
        ));
        palette.push(format!(
            "- inspect latest assistant session: `hellox assistant show {}`",
            session.session_id
        ));
        palette.push(format!(
            "- open latest session panel: `hellox session panel {}`",
            session.session_id
        ));
    }
    palette
}

fn ide_repl_palette(session: Option<&BridgeSessionSummary>) -> Vec<String> {
    let mut palette = vec![
        "- bridge overview: `/bridge panel`".to_string(),
        "- raw status: `/ide`".to_string(),
        "- assistant viewer: `/assistant`".to_string(),
    ];
    if let Some(session) = session {
        palette.push(format!(
            "- inspect latest bridge session: `/bridge panel {}`",
            session.session_id
        ));
        palette.push(format!(
            "- inspect latest assistant session: `/assistant show {}`",
            session.session_id
        ));
        palette.push(format!(
            "- open latest session panel: `/session panel {}`",
            session.session_id
        ));
    }
    palette
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_agent::{PlanningState, StoredSessionMessage, StoredSessionSnapshot};
    use hellox_config::{
        save_config, HelloxConfig, McpScope, PermissionMode, PluginEntryConfig, PluginSourceConfig,
    };
    use serde_json::json;

    use super::*;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-cli-bridge-panel-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn write_snapshot(root: &Path, session_id: &str, updated_at: u64) {
        let snapshot = StoredSessionSnapshot {
            session_id: session_id.to_string(),
            model: "opus".to_string(),
            permission_mode: Some(PermissionMode::AcceptEdits),
            output_style_name: None,
            output_style: None,
            persona: None,
            prompt_fragments: Vec::new(),
            config_path: None,
            planning: PlanningState::default(),
            working_directory: "D:\\workspace".to_string(),
            shell_name: "powershell".to_string(),
            system_prompt: "You are a bridge session.".to_string(),
            created_at: updated_at.saturating_sub(10),
            updated_at,
            agent_runtime: None,
            usage_by_model: Default::default(),
            messages: vec![
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "user",
                    "content": "hello bridge"
                }))
                .expect("user message"),
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "tool-1",
                            "name": "search",
                            "input": { "query": "bridge" }
                        }
                    ]
                }))
                .expect("assistant message"),
            ],
        };
        let raw = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");
        fs::create_dir_all(root).expect("create session root");
        fs::write(root.join(format!("{session_id}.json")), raw).expect("write snapshot");
    }

    fn write_config(path: &Path) {
        let mut config = HelloxConfig::default();
        config.mcp.servers.insert(
            "filesystem".to_string(),
            hellox_config::McpServerConfig {
                enabled: true,
                description: Some("Workspace filesystem".to_string()),
                scope: McpScope::Project,
                oauth: None,
                transport: hellox_config::McpTransportConfig::Stdio {
                    command: "npx".to_string(),
                    args: vec!["@modelcontextprotocol/server-filesystem".to_string()],
                    env: Default::default(),
                    cwd: None,
                },
            },
        );
        config.plugins.installed.insert(
            "filesystem".to_string(),
            PluginEntryConfig {
                enabled: true,
                install_path: Some("D:/plugins/filesystem".to_string()),
                source: PluginSourceConfig::Builtin {
                    name: "filesystem".to_string(),
                },
                version: Some("0.1.0".to_string()),
                description: Some("Filesystem plugin".to_string()),
            },
        );
        save_config(Some(path.to_path_buf()), &config).expect("save config");
    }

    fn runtime_paths(root: &Path) -> BridgeRuntimePaths {
        let config_path = root.join(".hellox").join("config.toml");
        write_config(&config_path);
        let sessions_root = root.join(".hellox").join("sessions");
        write_snapshot(&sessions_root, "older", 10);
        write_snapshot(&sessions_root, "newer", 20);
        BridgeRuntimePaths::new(
            config_path,
            sessions_root,
            root.join(".hellox").join("plugins"),
        )
    }

    #[test]
    fn bridge_panel_renders_selector_and_detail() {
        let root = temp_dir();
        let paths = runtime_paths(&root);

        let selector = render_bridge_panel(&paths, None).expect("render bridge selector");
        assert!(selector.contains("Bridge panel"));
        assert!(selector.contains("== Session selector =="));
        assert!(selector.contains("hellox bridge panel newer"));
        assert!(selector.contains("/bridge panel [session-id]"));

        let detail = render_bridge_panel(&paths, Some("newer")).expect("render bridge detail");
        assert!(detail.contains("Bridge panel: newer"));
        assert!(detail.contains("== Bridge session lens =="));
        assert!(detail.contains("tool_use: search"));
        assert!(detail.contains("hellox bridge show-session newer"));
        assert!(detail.contains("/assistant show newer"));
    }

    #[test]
    fn ide_panel_renders_latest_session_lens() {
        let root = temp_dir();
        let paths = runtime_paths(&root);

        let panel = render_ide_panel(&paths).expect("render ide panel");
        assert!(panel.contains("IDE panel"));
        assert!(panel.contains("latest_session"));
        assert!(panel.contains("== Latest session lens =="));
        assert!(panel.contains("hellox bridge panel newer"));
        assert!(panel.contains("/assistant show newer"));
    }
}
