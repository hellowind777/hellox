use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{HelloxConfig, PermissionMode};

use super::commands::{InstallCommand, ReplCommand, UpgradeCommand};
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-install-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session() -> AgentSession {
    let root = temp_dir();
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

fn metadata() -> ReplMetadata {
    let root = temp_dir();
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: root.join(".hellox").join("config.toml"),
        memory_root: root.join(".hellox").join("memory"),
        plugins_root: root.join(".hellox").join("plugins"),
        sessions_root: root.join(".hellox").join("sessions"),
        shares_root: root.join(".hellox").join("shares"),
    }
}

#[test]
fn parse_install_and_upgrade_commands() {
    assert_eq!(
        super::commands::parse_command("/install apply dist/hellox.exe out/hellox.exe --force"),
        Some(ReplCommand::Install(InstallCommand::Apply {
            source: Some(String::from("dist/hellox.exe")),
            target: Some(String::from("out/hellox.exe")),
            force: true,
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/upgrade apply dist/hellox.exe out/hellox.exe --backup --force"
        ),
        Some(ReplCommand::Upgrade(UpgradeCommand::Apply {
            source: Some(String::from("dist/hellox.exe")),
            target: Some(String::from("out/hellox.exe")),
            backup: true,
            force: true,
        }))
    );
}

#[test]
fn handle_install_status_stays_in_repl() {
    let mut session = session();

    let action = handle_repl_input("/install", &mut session, &metadata()).expect("install status");
    assert_eq!(action, ReplAction::Continue);
}

#[test]
fn handle_upgrade_apply_with_missing_source_returns_continue() {
    let mut session = session();

    let action =
        handle_repl_input("/upgrade apply", &mut session, &metadata()).expect("upgrade usage");
    assert_eq!(action, ReplAction::Continue);
}
