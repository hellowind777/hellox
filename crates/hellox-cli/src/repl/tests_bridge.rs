use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{HelloxConfig, PermissionMode};

use super::commands::{BridgeCommand, IdeCommand, ReplCommand};
use super::format::help_text;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-bridge-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session(root: PathBuf) -> AgentSession {
    AgentSession::create(
        GatewayClient::new("http://127.0.0.1:7821"),
        default_tool_registry(),
        root.join(".hellox").join("config.toml"),
        root,
        "powershell",
        AgentOptions::default(),
        PermissionMode::BypassPermissions,
        None,
        None,
        false,
        None,
    )
}

fn metadata(root: &PathBuf) -> ReplMetadata {
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: root.join(".hellox").join("config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join(".hellox").join("plugins"),
        sessions_root: root.join(".hellox").join("sessions"),
        shares_root: root.join("shares"),
    }
}

fn write_session(root: &PathBuf, session_id: &str) {
    fs::create_dir_all(root).expect("create sessions root");
    let raw = serde_json::to_string_pretty(&serde_json::json!({
        "session_id": session_id,
        "model": "opus",
        "permission_mode": "accept_edits",
        "output_style_name": null,
        "working_directory": "D:\\workspace",
        "shell_name": "powershell",
        "system_prompt": "system",
        "created_at": 1,
        "updated_at": 2,
        "messages": [
            { "role": "user", "content": "hello bridge" }
        ]
    }))
    .expect("serialize session");
    fs::write(root.join(format!("{session_id}.json")), raw).expect("write session");
}

#[test]
fn parse_bridge_and_ide_commands() {
    assert_eq!(
        super::commands::parse_command("/bridge"),
        Some(ReplCommand::Bridge(BridgeCommand::Status))
    );
    assert_eq!(
        super::commands::parse_command("/bridge sessions"),
        Some(ReplCommand::Bridge(BridgeCommand::Sessions))
    );
    assert_eq!(
        super::commands::parse_command("/bridge show abc"),
        Some(ReplCommand::Bridge(BridgeCommand::Show {
            session_id: Some(String::from("abc"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/ide"),
        Some(ReplCommand::Ide(IdeCommand::Status))
    );
}

#[test]
fn help_text_lists_bridge_commands() {
    let text = help_text();
    assert!(text.contains("/bridge"));
    assert!(text.contains("/bridge sessions"));
    assert!(text.contains("/ide"));
}

#[test]
fn handle_bridge_and_ide_commands_stay_in_repl() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    write_session(&metadata.sessions_root, "bridge-session");

    assert_eq!(
        handle_repl_input("/bridge", &mut session, &metadata).expect("bridge status"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/bridge sessions", &mut session, &metadata).expect("bridge sessions"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/bridge show bridge-session", &mut session, &metadata)
            .expect("bridge show"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/ide", &mut session, &metadata).expect("ide status"),
        ReplAction::Continue
    );
}
