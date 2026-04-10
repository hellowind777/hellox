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
    let root = env::temp_dir().join(format!("hellox-cli-repl-ext-{suffix}"));
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
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

#[test]
fn parse_extension_commands() {
    assert_eq!(
        super::commands::parse_command("/skills review"),
        Some(ReplCommand::Skills {
            name: Some(String::from("review"))
        })
    );
    assert_eq!(
        super::commands::parse_command("/hooks pre_tool"),
        Some(ReplCommand::Hooks {
            name: Some(String::from("pre_tool"))
        })
    );
}

#[test]
fn help_text_lists_extension_commands() {
    let text = help_text();
    assert!(text.contains("/skills [name]"));
    assert!(text.contains("/hooks [name]"));
}

#[test]
fn handle_skills_and_hooks_commands_render_project_entries() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let skills_root = root.join(".hellox").join("skills");
    let hooks_root = root.join(".hellox").join("hooks");
    fs::create_dir_all(&skills_root).expect("create skills root");
    fs::create_dir_all(&hooks_root).expect("create hooks root");
    fs::write(
        skills_root.join("review.md"),
        r#"---
name: review
description: Review a patch.
allowedTools: [Read, Grep]
---
Review skill body."#,
    )
    .expect("write skill");
    fs::write(
        hooks_root.join("pre_tool.ps1"),
        "Write-Host 'before tool'\n",
    )
    .expect("write hook");

    assert_eq!(
        handle_repl_input("/skills", &mut session, &metadata).expect("skills"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/hooks pre_tool", &mut session, &metadata).expect("hooks"),
        ReplAction::Continue
    );
}
