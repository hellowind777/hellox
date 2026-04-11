use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{
    default_tool_registry, AgentOptions, AgentSession, GatewayClient, StoredSession,
    StoredSessionMessage, StoredSessionSnapshot,
};
use hellox_config::{save_config, HelloxConfig, PermissionMode};
use serde_json::json;

use super::commands::{
    ModelCommand, OutputStyleCommand, ReplCommand, SessionCommand, WorkflowCommand,
};
use super::format::{config_text, help_text};
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session() -> AgentSession {
    session_in(temp_dir())
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

fn sessions_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-sessions-{suffix}"));
    fs::create_dir_all(&root).expect("create sessions root");
    root
}

fn metadata() -> ReplMetadata {
    let root = sessions_root();
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: PathBuf::from("C:/Users/test/.hellox/config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join("plugins"),
        sessions_root: root.clone(),
        shares_root: root.join("shares"),
    }
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

fn write_output_style(root: &PathBuf, style_name: &str, prompt: &str) {
    let styles_root = root.join(".hellox").join("output-styles");
    fs::create_dir_all(&styles_root).expect("create output styles root");
    fs::write(styles_root.join(format!("{style_name}.md")), prompt).expect("write output style");
}

fn write_session(root: &PathBuf, session_id: &str) {
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

fn restorable_session_with_tool_turn() -> AgentSession {
    let root = temp_dir();
    let stored = StoredSession {
        session_id: String::from("rewind-me"),
        path: root.join("rewind-me.json"),
        snapshot: StoredSessionSnapshot {
            session_id: String::from("rewind-me"),
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
            messages: vec![
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "user",
                    "content": "first prompt"
                }))
                .expect("first prompt"),
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "assistant",
                    "content": "first answer"
                }))
                .expect("first answer"),
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "user",
                    "content": "second prompt"
                }))
                .expect("second prompt"),
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "tool-1",
                            "name": "read_file",
                            "input": { "path": "src/main.rs" }
                        }
                    ]
                }))
                .expect("tool use"),
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "tool-1",
                            "content": "file contents",
                            "is_error": false
                        }
                    ]
                }))
                .expect("tool result"),
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": "assistant",
                    "content": "second answer"
                }))
                .expect("second answer"),
            ],
        },
    };

    AgentSession::restore(
        GatewayClient::new("http://127.0.0.1:7821"),
        default_tool_registry(),
        AgentOptions::default(),
        PermissionMode::Default,
        None,
        None,
        stored,
    )
}

#[test]
fn parse_known_and_unknown_commands() {
    assert_eq!(
        super::commands::parse_command("/help"),
        Some(ReplCommand::Help)
    );
    assert_eq!(
        super::commands::parse_command("/model gpt-5"),
        Some(ReplCommand::Model(ModelCommand::Use {
            value: Some(String::from("gpt-5"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/model list"),
        Some(ReplCommand::Model(ModelCommand::List))
    );
    assert_eq!(
        super::commands::parse_command("/model show sonnet"),
        Some(ReplCommand::Model(ModelCommand::Show {
            profile_name: Some(String::from("sonnet"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/model panel sonnet"),
        Some(ReplCommand::Model(ModelCommand::Panel {
            profile_name: Some(String::from("sonnet"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/model default sonnet"),
        Some(ReplCommand::Model(ModelCommand::Default {
            profile_name: Some(String::from("sonnet"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/permissions accept-edits"),
        Some(ReplCommand::Permissions {
            value: Some(String::from("accept-edits"))
        })
    );
    assert_eq!(
        super::commands::parse_command("/resume abc"),
        Some(ReplCommand::Resume {
            session_id: Some(String::from("abc"))
        })
    );
    assert_eq!(
        super::commands::parse_command("/share transcript.md"),
        Some(ReplCommand::Share {
            path: Some(String::from("transcript.md"))
        })
    );
    assert_eq!(
        super::commands::parse_command("/compact keep only active work"),
        Some(ReplCommand::Compact {
            instructions: Some(String::from("keep only active work"))
        })
    );
    assert_eq!(
        super::commands::parse_command("/rewind"),
        Some(ReplCommand::Rewind)
    );
    assert_eq!(
        super::commands::parse_command("/output-style"),
        Some(ReplCommand::OutputStyle(OutputStyleCommand::Current))
    );
    assert_eq!(
        super::commands::parse_command("/output-style show reviewer"),
        Some(ReplCommand::OutputStyle(OutputStyleCommand::Show {
            style_name: Some(String::from("reviewer"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/output-style use reviewer"),
        Some(ReplCommand::OutputStyle(OutputStyleCommand::Use {
            style_name: Some(String::from("reviewer"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/output-style clear"),
        Some(ReplCommand::OutputStyle(OutputStyleCommand::Clear))
    );
    assert_eq!(
        super::commands::parse_command("/session list"),
        Some(ReplCommand::Session(SessionCommand::List))
    );
    assert_eq!(
        super::commands::parse_command("/session panel abc"),
        Some(ReplCommand::Session(SessionCommand::Panel {
            session_id: Some(String::from("abc"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/session show abc"),
        Some(ReplCommand::Session(SessionCommand::Show {
            session_id: Some(String::from("abc"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/session share abc notes.md"),
        Some(ReplCommand::Session(SessionCommand::Share {
            session_id: Some(String::from("abc")),
            path: Some(String::from("notes.md"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/workflow"),
        Some(ReplCommand::Workflow(WorkflowCommand::List))
    );
    assert_eq!(
        super::commands::parse_command("/workflow dashboard release-review"),
        Some(ReplCommand::Workflow(WorkflowCommand::Dashboard {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/workflow dashboard --script-path scripts/custom-release.json"
        ),
        Some(ReplCommand::Workflow(WorkflowCommand::Dashboard {
            workflow_name: None,
            script_path: Some(String::from("scripts/custom-release.json")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/workflow validate release-review"),
        Some(ReplCommand::Workflow(WorkflowCommand::Validate {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
        }))
    );
    assert_eq!(
        super::commands::parse_command("/workflow show release-review"),
        Some(ReplCommand::Workflow(WorkflowCommand::Show {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
        }))
    );
    assert_eq!(
        super::commands::parse_command("/workflow init release-review"),
        Some(ReplCommand::Workflow(WorkflowCommand::Init {
            workflow_name: Some(String::from("release-review"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/workflow release-review ship carefully"),
        Some(ReplCommand::Workflow(WorkflowCommand::Run {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
            shared_context: Some(String::from("ship carefully"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/unknown"),
        Some(ReplCommand::Unknown(String::from("unknown")))
    );
    assert_eq!(super::commands::parse_command("plain prompt"), None);
}

#[test]
fn help_text_lists_core_commands() {
    let text = help_text();
    assert!(text.contains("/help"));
    assert!(text.contains("/output-style"));
    assert!(text.contains("/install"));
    assert!(text.contains("/upgrade"));
    assert!(text.contains("/workflow dashboard [name]"));
    assert!(text.contains("/workflow dashboard --script-path <path>"));
    assert!(text.contains("/output-style panel [name]"));
    assert!(text.contains("/model list"));
    assert!(text.contains("/model panel [name]"));
    assert!(text.contains("/model default <name>"));
    assert!(text.contains("/permissions [mode]"));
    assert!(text.contains("/share [path]"));
    assert!(text.contains("/workflow"));
    assert!(text.contains("/workflow validate"));
    assert!(text.contains("/workflow init"));
    assert!(text.contains("/compact [instructions]"));
    assert!(text.contains("/rewind"));
    assert!(text.contains("/session panel [id]"));
    assert!(text.contains("/session share <id> [path]"));
    assert!(text.contains("/session list"));
}

#[test]
fn config_text_includes_path_and_serialized_config() {
    let text = config_text(&metadata());
    assert!(text.contains("config_path: C:/Users/test/.hellox/config.toml"));
    assert!(text.contains("[gateway]"));
}

#[test]
fn handle_model_command_updates_session() {
    let mut session = session();
    let action =
        handle_repl_input("/model openai_opus", &mut session, &metadata()).expect("handle model");
    assert!(matches!(action, ReplAction::Continue));
    assert_eq!(session.model(), "openai_opus");
}

#[test]
fn handle_model_list_and_default_commands_stay_in_repl() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());

    let list_action =
        handle_repl_input("/model list", &mut session, &metadata).expect("handle model list");
    assert_eq!(list_action, ReplAction::Continue);

    let default_action = handle_repl_input("/model default sonnet", &mut session, &metadata)
        .expect("handle model default");
    assert_eq!(default_action, ReplAction::Continue);
}

#[test]
fn handle_output_style_commands_update_and_show_session_style() {
    let root = temp_dir();
    write_output_style(
        &root,
        "reviewer",
        "Prioritize bugs, risks, and missing tests.",
    );
    let mut config = HelloxConfig::default();
    config.output_style.default = Some("reviewer".to_string());
    save_config(Some(root.join(".hellox").join("config.toml")), &config).expect("save config");
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());

    let summary = super::style_actions::handle_output_style_command(
        OutputStyleCommand::Current,
        &mut session,
        &metadata,
    )
    .expect("summarize styles");
    assert!(summary.contains("default_output_style: reviewer"));
    assert!(summary.contains("reviewer"));

    let detail = super::style_actions::handle_output_style_command(
        OutputStyleCommand::Show { style_name: None },
        &mut session,
        &metadata,
    )
    .expect("show default style");
    assert!(detail.contains("style: reviewer"));
    assert!(detail.contains("default: true"));

    let action = handle_repl_input("/output-style use reviewer", &mut session, &metadata)
        .expect("use style");
    assert_eq!(action, ReplAction::Continue);
    assert_eq!(session.output_style_name(), Some("reviewer"));

    let cleared =
        handle_repl_input("/output-style clear", &mut session, &metadata).expect("clear style");
    assert_eq!(cleared, ReplAction::Continue);
    assert_eq!(session.output_style_name(), None);
}

#[test]
fn handle_permissions_command_updates_session() {
    let mut session = session();
    let action = handle_repl_input("/permissions accept-edits", &mut session, &metadata())
        .expect("handle permissions");
    assert!(matches!(action, ReplAction::Continue));
    assert_eq!(session.permission_mode(), &PermissionMode::AcceptEdits);
}

#[test]
fn handle_permissions_without_argument_stays_in_repl() {
    let mut session = session();
    let action =
        handle_repl_input("/permissions", &mut session, &metadata()).expect("show permissions");
    assert_eq!(action, ReplAction::Continue);
}

#[test]
fn handle_non_command_returns_submit_action() {
    let mut session = session();
    let action =
        handle_repl_input("implement the feature", &mut session, &metadata()).expect("submit");
    match action {
        ReplAction::Submit(prompt) => assert_eq!(prompt, "implement the feature"),
        other => panic!("expected submit action, got {other:?}"),
    }
}

#[test]
fn handle_clear_command_resets_messages() {
    let mut session = session();
    let action = handle_repl_input("/clear", &mut session, &metadata()).expect("clear");
    assert_eq!(action, ReplAction::Continue);
    assert_eq!(session.message_count(), 0);
}

#[test]
fn handle_rewind_command_removes_latest_turn() {
    let mut session = restorable_session_with_tool_turn();
    let action = handle_repl_input("/rewind", &mut session, &metadata()).expect("rewind");
    assert_eq!(action, ReplAction::Continue);
    assert_eq!(session.message_count(), 2);
}

#[test]
fn handle_share_command_writes_markdown_transcript() {
    let root = temp_dir();
    let mut session = session_in(root.clone());
    let metadata = ReplMetadata {
        config: HelloxConfig::default(),
        config_path: PathBuf::from("C:/Users/test/.hellox/config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    };

    let action =
        handle_repl_input("/share notes/session.md", &mut session, &metadata).expect("share");
    assert_eq!(action, ReplAction::Continue);

    let output = root.join("notes").join("session.md");
    let markdown = fs::read_to_string(output).expect("read shared transcript");
    assert!(markdown.contains("# hellox transcript"));
    assert!(markdown.contains("- permission_mode: bypass_permissions"));
}

#[test]
fn handle_compact_command_replaces_history_with_summary() {
    let mut session = restorable_session_with_tool_turn();
    let action = handle_repl_input(
        "/compact keep active implementation context",
        &mut session,
        &metadata(),
    )
    .expect("compact");
    assert_eq!(action, ReplAction::Continue);
    assert_eq!(session.message_count(), 1);
    let summary = match &session.messages()[0].content {
        hellox_gateway_api::MessageContent::Text(text) => text,
        other => panic!("expected compacted summary text, got {other:?}"),
    };
    assert!(summary.contains("Conversation summary generated by hellox /compact."));
    assert!(summary.contains("Compaction instructions: keep active implementation context"));
}

#[test]
fn handle_session_share_command_writes_persisted_transcript() {
    let root = temp_dir();
    let mut session = session_in(root.clone());
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");
    write_session(&sessions_root, "persisted");
    let metadata = ReplMetadata {
        config: HelloxConfig::default(),
        config_path: PathBuf::from("C:/Users/test/.hellox/config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join("plugins"),
        sessions_root,
        shares_root: root.join("shares"),
    };

    let action = handle_repl_input(
        "/session share persisted exports/persisted.md",
        &mut session,
        &metadata,
    )
    .expect("session share");
    assert_eq!(action, ReplAction::Continue);

    let output = root.join("exports").join("persisted.md");
    let markdown = fs::read_to_string(output).expect("read persisted transcript");
    assert!(markdown.contains("- session_id: persisted"));
    assert!(markdown.contains("- permission_mode: accept_edits"));
}

#[test]
fn handle_resume_command_returns_resume_action_for_existing_session() {
    let mut session = session();
    let metadata = metadata();
    write_session(&metadata.sessions_root, "resume-me");

    let action = handle_repl_input("/resume resume-me", &mut session, &metadata).expect("resume");
    assert_eq!(action, ReplAction::Resume(String::from("resume-me")));
}

#[test]
fn handle_resume_without_argument_stays_in_repl() {
    let mut session = session();
    let metadata = metadata();
    let action = handle_repl_input("/resume", &mut session, &metadata).expect("resume help");
    assert_eq!(action, ReplAction::Continue);
}

#[test]
fn handle_session_list_stays_in_repl() {
    let mut session = session();
    let metadata = metadata();
    write_session(&metadata.sessions_root, "list-me");

    let action = handle_repl_input("/session list", &mut session, &metadata).expect("session list");
    assert_eq!(action, ReplAction::Continue);
}

#[test]
fn handle_session_show_stays_in_repl() {
    let mut session = session();
    let metadata = metadata();
    write_session(&metadata.sessions_root, "show-me");

    let action =
        handle_repl_input("/session show show-me", &mut session, &metadata).expect("session show");
    assert_eq!(action, ReplAction::Continue);
}
