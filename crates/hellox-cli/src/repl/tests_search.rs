use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{
    default_tool_registry, AgentOptions, AgentSession, GatewayClient, StoredSession,
    StoredSessionSnapshot,
};
use hellox_config::{HelloxConfig, PermissionMode};
use serde_json::json;

use crate::search::DEFAULT_SEARCH_LIMIT;

use super::commands::ReplCommand;
use super::format::{help_text, search_text};
use super::ReplMetadata;

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-search-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn metadata(root: &Path) -> ReplMetadata {
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: PathBuf::from("C:/Users/test/.hellox/config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

fn restored_session(root: &Path, session_id: &str, messages: &[(&str, &str)]) -> AgentSession {
    AgentSession::restore(
        GatewayClient::new("http://127.0.0.1:7821"),
        default_tool_registry(),
        AgentOptions::default(),
        PermissionMode::BypassPermissions,
        None,
        None,
        StoredSession {
            session_id: session_id.to_string(),
            path: root.join(format!("{session_id}.json")),
            snapshot: StoredSessionSnapshot {
                session_id: session_id.to_string(),
                model: String::from("opus"),
                permission_mode: Some(PermissionMode::BypassPermissions),
                output_style_name: None,
                output_style: None,
                persona: None,
                prompt_fragments: Vec::new(),
                config_path: None,
                planning: hellox_agent::PlanningState::default(),
                working_directory: root.display().to_string(),
                shell_name: String::from("powershell"),
                system_prompt: String::from("system"),
                created_at: 1,
                updated_at: 2,
                agent_runtime: None,
                usage_by_model: Default::default(),
                messages: messages
                    .iter()
                    .map(|(role, content)| {
                        serde_json::from_value(json!({
                            "role": role,
                            "content": content,
                        }))
                        .expect("message")
                    })
                    .collect(),
            },
        },
    )
}

fn write_session(root: &Path, session_id: &str, content: &str) {
    let snapshot = StoredSessionSnapshot {
        session_id: session_id.to_string(),
        model: String::from("opus"),
        permission_mode: Some(PermissionMode::AcceptEdits),
        output_style_name: None,
        output_style: None,
        persona: None,
        prompt_fragments: Vec::new(),
        config_path: None,
        planning: hellox_agent::PlanningState::default(),
        working_directory: String::from("D:\\workspace"),
        shell_name: String::from("powershell"),
        system_prompt: String::from("system"),
        created_at: 1,
        updated_at: 2,
        agent_runtime: None,
        usage_by_model: Default::default(),
        messages: vec![serde_json::from_value(json!({
            "role": "user",
            "content": content,
        }))
        .expect("message")],
    };

    fs::create_dir_all(root).expect("create sessions root");
    fs::write(
        root.join(format!("{session_id}.json")),
        serde_json::to_string_pretty(&snapshot).expect("serialize session"),
    )
    .expect("write session");
}

fn write_memory(root: &Path, memory_id: &str, content: &str) {
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create memory root");
    fs::write(
        sessions_root.join(format!("{memory_id}.md")),
        format!("# hellox memory\n\n{content}"),
    )
    .expect("write memory");
}

#[test]
fn parse_search_commands() {
    assert_eq!(
        super::commands::parse_command("/search accepted architecture"),
        Some(ReplCommand::Search {
            query: Some(String::from("accepted architecture"))
        })
    );
    assert_eq!(
        super::commands::parse_command("/search"),
        Some(ReplCommand::Search { query: None })
    );
}

#[test]
fn help_text_lists_search_command() {
    let text = help_text();
    assert!(text.contains("/search <query>"));
}

#[test]
fn search_text_combines_transcript_sessions_and_memory() {
    let root = temp_dir();
    let metadata = metadata(&root);
    let session = restored_session(
        &root.join("active"),
        "active-transcript",
        &[("assistant", "transcript architecture note")],
    );
    write_session(
        &metadata.sessions_root,
        "persisted-session",
        "persisted architecture note",
    );
    write_memory(
        &metadata.memory_root,
        "session-persisted-session",
        "captured architecture note",
    );

    let rendered = search_text(&session, &metadata, "architecture", DEFAULT_SEARCH_LIMIT);

    assert!(rendered.contains("source\tsource_id\tlocation\tpreview"));
    assert!(rendered.contains("transcript\tactive-transcript"));
    assert!(rendered.contains("session\tpersisted-session"));
    assert!(rendered.contains("memory\tsession-persisted-session"));
}
