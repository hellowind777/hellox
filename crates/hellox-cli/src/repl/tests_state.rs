use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{save_config, HelloxConfig, PermissionMode};
use serde_json::json;

use super::commands::{ModelCommand, ReplCommand, SessionCommand};
use super::format::help_text;
use super::{ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-state-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session_in(root: PathBuf) -> AgentSession {
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

fn metadata_in(root: &PathBuf) -> ReplMetadata {
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
    let raw = serde_json::to_string_pretty(&json!({
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
            { "role": "user", "content": "hello" }
        ]
    }))
    .expect("serialize session");
    fs::write(root.join(format!("{session_id}.json")), raw).expect("write session");
}

#[test]
fn parse_state_panel_commands() {
    assert_eq!(
        super::commands::parse_command("/model panel sonnet"),
        Some(ReplCommand::Model(ModelCommand::Panel {
            profile_name: Some(String::from("sonnet"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/session panel persisted"),
        Some(ReplCommand::Session(SessionCommand::Panel {
            session_id: Some(String::from("persisted"))
        }))
    );
}

#[test]
fn help_text_lists_state_panel_commands() {
    let text = help_text();
    assert!(text.contains("/model panel [name]"));
    assert!(text.contains("/session panel [id]"));
}

#[test]
fn session_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    write_session(&metadata.sessions_root, "bbb");
    write_session(&metadata.sessions_root, "aaa");
    let mut session = session_in(root);
    let driver = super::CliReplDriver::new();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            match driver
                .handle_repl_input_async("1", &mut session, &metadata)
                .await
                .expect("submit")
            {
                ReplAction::Submit(text) => assert_eq!(text, "1"),
                other => panic!("expected submit action, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("/session panel", &mut session, &metadata)
                    .await
                    .expect("open panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::SessionPanelList { session_ids }) => {
                    assert_eq!(session_ids, vec!["aaa".to_string(), "bbb".to_string()]);
                }
                other => panic!("expected session selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("select session"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}

#[test]
fn handle_model_panel_renders_dashboard_and_detail() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    save_config(Some(metadata.config_path.clone()), &HelloxConfig::default()).expect("save config");
    let mut session = session_in(root.clone());

    let list = super::core_actions::handle_model_command(
        ModelCommand::Panel { profile_name: None },
        &mut session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("render model list panel");
    assert!(list.contains("Model panel"));
    assert!(list.contains("hellox model panel sonnet"));
    assert!(list.contains("/model panel [profile-name]"));

    let detail = super::core_actions::handle_model_command(
        ModelCommand::Panel {
            profile_name: Some(String::from("sonnet")),
        },
        &mut session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("render model detail panel");
    assert!(detail.contains("Model panel: sonnet"));
    assert!(detail.contains("upstream_model"));
    assert!(detail.contains("/model default sonnet"));
}

#[test]
fn model_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    save_config(Some(metadata.config_path.clone()), &HelloxConfig::default()).expect("save config");
    let mut session = session_in(root);
    let driver = super::CliReplDriver::new();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("/model panel", &mut session, &metadata)
                    .await
                    .expect("open model panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::ModelPanelList { profile_names }) => {
                    assert!(profile_names.contains(&"sonnet".to_string()));
                }
                other => panic!("expected model selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("select profile"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}

#[test]
fn handle_session_panel_renders_dashboard_and_detail() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    write_session(&metadata.sessions_root, "persisted");
    let session = session_in(root);

    let list = super::core_actions::handle_session_command(
        SessionCommand::Panel { session_id: None },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("render session list panel");
    assert!(list.contains("Session panel"));
    assert!(list.contains("== Session selector =="));
    assert!(list.contains("hellox session panel persisted"));
    assert!(list.contains("/session panel [session-id]"));

    let detail = super::core_actions::handle_session_command(
        SessionCommand::Panel {
            session_id: Some(String::from("persisted")),
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("render session detail panel");
    assert!(detail.contains("Session panel: persisted"));
    assert!(detail.contains("== Session lens =="));
    assert!(detail.contains("> [1] persisted — opus"));
    assert!(detail.contains("Usage by model"));
    assert!(detail.contains("/resume persisted"));
}
