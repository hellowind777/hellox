use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::PermissionMode;
use hellox_repl::{parse_command, ReplCommand, WorkflowCommand};

use crate::repl::handle_workflow_command_for_test;

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-workflow-authoring-{suffix}"));
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

fn write_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(".hellox").join("workflows").join(relative);
    fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
    fs::write(path, raw).expect("write workflow");
}

#[test]
fn repl_parses_duplicate_and_move_commands() {
    assert_eq!(
        parse_command("/workflow duplicate-step release-review 2 --to 3 --name ship copy"),
        Some(ReplCommand::Workflow(WorkflowCommand::DuplicateStep {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
            step_number: Some(2),
            to_step_number: Some(3),
            name: Some(String::from("ship copy")),
        }))
    );
    assert_eq!(
        parse_command("/workflow move-step release-review 3 --to 1"),
        Some(ReplCommand::Workflow(WorkflowCommand::MoveStep {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
            step_number: Some(3),
            to_step_number: Some(1),
        }))
    );
    assert_eq!(
        parse_command(
            "/workflow duplicate-step --script-path scripts/custom-release.json 2 --to 3"
        ),
        Some(ReplCommand::Workflow(WorkflowCommand::DuplicateStep {
            workflow_name: None,
            script_path: Some(String::from("scripts/custom-release.json")),
            step_number: Some(2),
            to_step_number: Some(3),
            name: None,
        }))
    );
    assert_eq!(
        parse_command("/workflow move-step --script-path scripts/custom-release.json 3 --to 1"),
        Some(ReplCommand::Workflow(WorkflowCommand::MoveStep {
            workflow_name: None,
            script_path: Some(String::from("scripts/custom-release.json")),
            step_number: Some(3),
            to_step_number: Some(1),
        }))
    );
}

#[tokio::test]
async fn repl_workflow_authoring_supports_duplicate_and_move() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
    );

    let mut session = session_in(root);
    let duplicated = handle_workflow_command_for_test(
        WorkflowCommand::DuplicateStep {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
            step_number: Some(2),
            to_step_number: Some(3),
            name: Some(String::from("ship copy")),
        },
        &mut session,
    )
    .await
    .expect("duplicate step from repl");
    assert!(duplicated.contains("Duplicated workflow step 2 into step 3"));
    assert!(duplicated.contains("ship copy"));

    let moved = handle_workflow_command_for_test(
        WorkflowCommand::MoveStep {
            workflow_name: Some(String::from("release-review")),
            script_path: None,
            step_number: Some(3),
            to_step_number: Some(1),
        },
        &mut session,
    )
    .await
    .expect("move step from repl");
    assert!(moved.contains("Moved workflow step 3"));
    assert!(moved.contains("steps: 3"));
}
