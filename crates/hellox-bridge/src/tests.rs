use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{StoredSessionMessage, StoredSessionSnapshot};
use hellox_config::{
    save_config, HelloxConfig, McpScope, PermissionMode, PluginEntryConfig, PluginSourceConfig,
};
use serde_json::json;

use crate::{
    format_bridge_session_detail, format_bridge_session_list, format_bridge_status,
    format_ide_status, inspect_bridge_status, inspect_ide_status, list_bridge_sessions,
    load_bridge_session, run_stdio_bridge, BridgeResponse, BridgeRuntimePaths,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-bridge-{suffix}"));
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
        planning: hellox_agent::PlanningState::default(),
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
fn inspects_status_and_latest_session() {
    let root = temp_dir();
    let paths = runtime_paths(&root);

    let status = inspect_bridge_status(&paths).expect("inspect status");
    assert_eq!(status.persisted_sessions, 2);
    assert_eq!(status.enabled_mcp_servers, 1);
    assert_eq!(status.enabled_plugins, 1);
    assert!(format_bridge_status(&status).contains("mode: local_bridge"));

    let ide_status = inspect_ide_status(&paths).expect("inspect ide status");
    assert_eq!(
        ide_status
            .latest_session
            .as_ref()
            .expect("latest session")
            .session_id,
        "newer"
    );
    assert!(format_ide_status(&ide_status).contains("latest_session: newer"));
}

#[test]
fn lists_and_loads_bridge_sessions() {
    let root = temp_dir();
    let paths = runtime_paths(&root);

    let sessions = list_bridge_sessions(&paths).expect("list sessions");
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session_id, "newer");
    assert!(format_bridge_session_list(&sessions).contains("permission_mode"));

    let detail = load_bridge_session(&paths, "newer").expect("load session");
    assert_eq!(detail.summary.message_count, 2);
    let rendered = format_bridge_session_detail(&detail);
    assert!(rendered.contains("tool_use: search"));
    assert!(rendered.contains("system_prompt_preview: You are a bridge session."));
}

#[test]
fn stdio_bridge_handles_status_get_and_shutdown() {
    let root = temp_dir();
    let paths = runtime_paths(&root);
    let input = [
        r#"{"id":"1","method":"status"}"#,
        r#"{"id":"2","method":"sessions/list"}"#,
        r#"{"id":"3","method":"sessions/get","params":{"session_id":"newer"}}"#,
        r#"{"id":"4","method":"shutdown"}"#,
    ]
    .join("\n");
    let mut output = Vec::new();

    run_stdio_bridge(Cursor::new(input.into_bytes()), &mut output, &paths).expect("run bridge");

    let lines = String::from_utf8(output)
        .expect("utf8 output")
        .lines()
        .map(|line| serde_json::from_str::<BridgeResponse>(line).expect("parse response"))
        .collect::<Vec<_>>();

    assert_eq!(lines.len(), 4);
    assert!(lines[0].ok);
    assert_eq!(lines[0].id.as_deref(), Some("1"));
    assert!(lines[1].ok);
    assert!(lines[2].ok);
    assert_eq!(lines[2].id.as_deref(), Some("3"));
    assert!(lines[3].ok);
    assert_eq!(lines[3].id.as_deref(), Some("4"));
}

#[test]
fn stdio_bridge_returns_structured_errors() {
    let root = temp_dir();
    let paths = runtime_paths(&root);
    let input = [
        r#"{"id":"1","method":"sessions/get","params":{}}"#,
        r#"{"id":"2","method":"unknown"}"#,
    ]
    .join("\n");
    let mut output = Vec::new();

    run_stdio_bridge(Cursor::new(input.into_bytes()), &mut output, &paths).expect("run bridge");

    let lines = String::from_utf8(output)
        .expect("utf8 output")
        .lines()
        .map(|line| serde_json::from_str::<BridgeResponse>(line).expect("parse response"))
        .collect::<Vec<_>>();

    assert_eq!(lines.len(), 2);
    assert!(!lines[0].ok);
    assert!(lines[0]
        .error
        .as_deref()
        .expect("error")
        .contains("missing `session_id`"));
    assert!(!lines[1].ok);
    assert!(lines[1]
        .error
        .as_deref()
        .expect("error")
        .contains("unsupported bridge method"));
}
