use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{load_or_default, HelloxConfig, PermissionMode};

use super::commands::{AssistantCommand, RemoteEnvCommand, ReplCommand, TeleportCommand};
use super::format::help_text;
use super::remote_actions::handle_assistant_command;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-remote-{suffix}"));
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
        Some("active-session".to_string()),
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
            { "role": "user", "content": "hello remote" }
        ]
    }))
    .expect("serialize session");
    fs::write(root.join(format!("{session_id}.json")), raw).expect("write session");
}

#[test]
fn parse_remote_commands() {
    assert_eq!(
        super::commands::parse_command("/remote-env"),
        Some(ReplCommand::RemoteEnv(RemoteEnvCommand::List))
    );
    assert_eq!(
        super::commands::parse_command("/remote-env panel dev"),
        Some(ReplCommand::RemoteEnv(RemoteEnvCommand::Panel {
            environment_name: Some(String::from("dev")),
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/remote-env add dev https://remote.example.test REMOTE_TOKEN"
        ),
        Some(ReplCommand::RemoteEnv(RemoteEnvCommand::Add {
            environment_name: Some(String::from("dev")),
            url: Some(String::from("https://remote.example.test")),
            token_env: Some(String::from("REMOTE_TOKEN")),
            account_id: None,
            device_id: None,
        }))
    );
    assert_eq!(
        super::commands::parse_command("/teleport panel dev session-123"),
        Some(ReplCommand::Teleport(TeleportCommand::Panel {
            environment_name: Some(String::from("dev")),
            session_id: Some(String::from("session-123")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/teleport plan dev session-123"),
        Some(ReplCommand::Teleport(TeleportCommand::Plan {
            environment_name: Some(String::from("dev")),
            session_id: Some(String::from("session-123")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/teleport connect dev session-123"),
        Some(ReplCommand::Teleport(TeleportCommand::Connect {
            environment_name: Some(String::from("dev")),
            session_id: Some(String::from("session-123")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/assistant show session-123"),
        Some(ReplCommand::Assistant(AssistantCommand::Show {
            session_id: Some(String::from("session-123")),
            environment_name: None,
        }))
    );
}

#[test]
fn help_text_lists_remote_commands() {
    let text = help_text();
    assert!(text.contains("/remote-env"));
    assert!(text.contains("/remote-env panel [name]"));
    assert!(text.contains("/teleport panel"));
    assert!(text.contains("/teleport plan"));
    assert!(text.contains("/teleport connect"));
    assert!(text.contains("/assistant"));
}

#[test]
fn handle_remote_commands_stay_in_repl() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    write_session(&metadata.sessions_root, "remote-session");

    assert_eq!(
        handle_repl_input(
            "/remote-env add dev https://remote.example.test REMOTE_TOKEN",
            &mut session,
            &metadata,
        )
        .expect("remote-env add"),
        ReplAction::Continue
    );
    let config = load_or_default(Some(metadata.config_path.clone())).expect("load config");
    assert!(config.remote.environments.contains_key("dev"));

    assert_eq!(
        handle_repl_input("/remote-env panel", &mut session, &metadata).expect("remote-env panel"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/teleport panel dev remote-session",
            &mut session,
            &metadata
        )
        .expect("teleport panel"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/teleport plan dev remote-session", &mut session, &metadata)
            .expect("teleport plan"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/assistant show remote-session", &mut session, &metadata)
            .expect("assistant show"),
        ReplAction::Continue
    );
}

#[test]
fn assistant_command_renders_local_viewer_panels() {
    let root = temp_dir();
    let metadata = metadata(&root);
    write_session(&metadata.sessions_root, "remote-session");

    let list = handle_assistant_command(
        AssistantCommand::List {
            environment_name: None,
        },
        &metadata,
    )
    .expect("assistant list");
    assert!(list.contains("Assistant viewer panel"));
    assert!(list.contains("remote-session"));
    assert!(list.contains("hellox assistant show remote-session"));

    let detail = handle_assistant_command(
        AssistantCommand::Show {
            session_id: Some(String::from("remote-session")),
            environment_name: None,
        },
        &metadata,
    )
    .expect("assistant detail");
    assert!(detail.contains("Assistant viewer: remote-session"));
    assert!(detail.contains("Transcript preview"));
    assert!(detail.contains("system_prompt_preview: system"));
}

#[test]
fn remote_env_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let driver = super::CliReplDriver::new();

    assert_eq!(
        handle_repl_input(
            "/remote-env add dev https://remote.example.test REMOTE_TOKEN",
            &mut session,
            &metadata,
        )
        .expect("remote-env add dev"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/remote-env add qa https://qa.example.test QA_TOKEN",
            &mut session,
            &metadata,
        )
        .expect("remote-env add qa"),
        ReplAction::Continue
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("/remote-env panel", &mut session, &metadata)
                    .await
                    .expect("open remote-env panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::RemoteEnvPanelList { environment_names }) => {
                    assert_eq!(environment_names, vec!["dev".to_string(), "qa".to_string()]);
                }
                other => panic!("expected remote-env selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("select remote env"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}
