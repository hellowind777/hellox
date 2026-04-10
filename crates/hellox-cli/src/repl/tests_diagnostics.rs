use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{HelloxConfig, PermissionMode};

use super::commands::ReplCommand;
use super::format::help_text;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-diagnostics-{suffix}"));
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
        config_path: PathBuf::from("C:/Users/test/.hellox/config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

#[test]
fn parse_diagnostic_commands() {
    assert_eq!(
        super::commands::parse_command("/doctor"),
        Some(ReplCommand::Doctor)
    );
    assert_eq!(
        super::commands::parse_command("/usage"),
        Some(ReplCommand::Usage)
    );
    assert_eq!(
        super::commands::parse_command("/stats"),
        Some(ReplCommand::Stats)
    );
    assert_eq!(
        super::commands::parse_command("/cost"),
        Some(ReplCommand::Cost)
    );
}

#[test]
fn help_text_lists_diagnostic_commands() {
    let text = help_text();
    assert!(text.contains("/doctor"));
    assert!(text.contains("/usage"));
    assert!(text.contains("/stats"));
    assert!(text.contains("/cost"));
}

#[test]
fn handle_diagnostic_commands_stay_in_repl() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    assert_eq!(
        handle_repl_input("/doctor", &mut session, &metadata).expect("doctor"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/usage", &mut session, &metadata).expect("usage"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/stats", &mut session, &metadata).expect("stats"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/cost", &mut session, &metadata).expect("cost"),
        ReplAction::Continue
    );
}
