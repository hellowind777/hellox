use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::cli_types::{Cli, Commands};
use crate::cli_workflow_types::WorkflowCommands;
use crate::workflow_commands::workflow_command_text;
use crate::workflow_test_support::acquire_workflow_test_guard;

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-workflow-command-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn write_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(".hellox").join("workflows").join(relative);
    fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
    fs::write(path, raw).expect("write workflow");
}

fn write_explicit_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("explicit workflow dir"))
        .expect("create explicit workflow dir");
    fs::write(path, raw).expect("write explicit workflow");
}

fn write_config(root: &Path, base_url: &str) -> PathBuf {
    let config = format!(
        "[gateway]\nlisten = \"{}\"\n\n[session]\npersist = false\nmodel = \"mock-model\"\n",
        base_url
    );
    let path = root.join(".hellox").join("config.toml");
    fs::create_dir_all(path.parent().expect("config dir")).expect("create config dir");
    fs::write(&path, config).expect("write config");
    path
}

async fn spawn_mock_gateway(response_text: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock gateway");
    let address = listener.local_addr().expect("local addr");
    let response_text = response_text.to_string();
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.expect("accept connection");
            let mut buffer = vec![0_u8; 4096];
            let _ = stream.read(&mut buffer).await;
            let body = serde_json::json!({
                "id": "workflow-command-response",
                "type": "message",
                "role": "assistant",
                "model": "mock-model",
                "content": [{ "type": "text", "text": response_text }],
                "stop_reason": "end_turn",
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 5
                }
            })
            .to_string();
            let payload = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(payload.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    format!("http://{}", address)
}

#[test]
fn parses_workflow_duplicate_and_move_commands() {
    let dashboard = Cli::try_parse_from(["hellox", "workflow", "dashboard", "release-review"])
        .expect("parse workflow dashboard");
    let dashboard_by_path = Cli::try_parse_from([
        "hellox",
        "workflow",
        "dashboard",
        "--script-path",
        "scripts/custom-release.json",
    ])
    .expect("parse workflow dashboard by script path");
    let duplicate = Cli::try_parse_from([
        "hellox",
        "workflow",
        "duplicate-step",
        "--workflow",
        "release-review",
        "2",
        "--to",
        "3",
        "--name",
        "ship copy",
    ])
    .expect("parse workflow duplicate-step");
    let overview_by_path = Cli::try_parse_from([
        "hellox",
        "workflow",
        "overview",
        "--script-path",
        "scripts/custom-release.json",
    ])
    .expect("parse workflow overview by script path");
    let moved = Cli::try_parse_from([
        "hellox",
        "workflow",
        "move-step",
        "--workflow",
        "release-review",
        "3",
        "--to",
        "1",
    ])
    .expect("parse workflow move-step");

    match dashboard.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Dashboard {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow dashboard command: {other:?}"),
    }

    match dashboard_by_path.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Dashboard {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, None);
            assert_eq!(
                script_path,
                Some(PathBuf::from("scripts/custom-release.json"))
            );
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow dashboard-by-path command: {other:?}"),
    }

    match overview_by_path.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Overview {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, None);
            assert_eq!(
                script_path,
                Some(PathBuf::from("scripts/custom-release.json"))
            );
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow overview-by-path command: {other:?}"),
    }

    match duplicate.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::DuplicateStep {
                    workflow_name,
                    step_number,
                    to_step_number,
                    name,
                    ..
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(step_number, 2);
            assert_eq!(to_step_number, Some(3));
            assert_eq!(name, Some(String::from("ship copy")));
        }
        other => panic!("unexpected workflow duplicate-step command: {other:?}"),
    }

    match moved.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::MoveStep {
                    workflow_name,
                    step_number,
                    to_step_number,
                    ..
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(step_number, 3);
            assert_eq!(to_step_number, 1);
        }
        other => panic!("unexpected workflow move-step command: {other:?}"),
    }
}

#[tokio::test]
async fn workflow_authoring_commands_support_duplicate_and_move() {
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

    let duplicated = workflow_command_text(WorkflowCommands::DuplicateStep {
        workflow_name: Some("release-review".to_string()),
        step_number: 2,
        script_path: None,
        to_step_number: Some(3),
        name: Some("ship copy".to_string()),
        cwd: Some(root.clone()),
    })
    .await
    .expect("duplicate workflow step");
    assert!(duplicated.contains("Duplicated workflow step 2 into step 3"));
    assert!(duplicated.contains("ship copy"));

    let moved = workflow_command_text(WorkflowCommands::MoveStep {
        workflow_name: Some("release-review".to_string()),
        step_number: 3,
        script_path: None,
        to_step_number: 1,
        cwd: Some(root.clone()),
    })
    .await
    .expect("move workflow step");
    assert!(moved.contains("Moved workflow step 3"));
    assert!(moved.contains("steps: 3"));
}

#[tokio::test]
async fn workflow_run_command_executes_named_script() {
    let _guard = acquire_workflow_test_guard().await;
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
    );
    let base_url = spawn_mock_gateway("workflow command done").await;
    let config_path = write_config(&root, &base_url);

    let text = workflow_command_text(WorkflowCommands::Run {
        workflow_name: Some("release-review".to_string()),
        script_path: None,
        shared_context: Some("ship carefully".to_string()),
        continue_on_error: false,
        config: Some(config_path),
        cwd: Some(root.clone()),
    })
    .await
    .expect("run workflow command");

    assert!(text.contains("\"workflow_source\": \".hellox/workflows/release-review.json\""));
    assert!(text.contains("workflow command done"));
}

#[tokio::test]
async fn workflow_runs_and_last_run_support_explicit_script_path() {
    let _guard = acquire_workflow_test_guard().await;
    let root = temp_dir();
    write_explicit_workflow(
        &root,
        "scripts/custom-release.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
    );
    let base_url = spawn_mock_gateway("workflow command done").await;
    let config_path = write_config(&root, &base_url);
    let absolute_script_path = root
        .join("scripts")
        .join("custom-release.json")
        .display()
        .to_string()
        .replace('\\', "/");

    let run = workflow_command_text(WorkflowCommands::Run {
        workflow_name: None,
        script_path: Some(PathBuf::from("scripts/custom-release.json")),
        shared_context: Some("ship carefully".to_string()),
        continue_on_error: false,
        config: Some(config_path),
        cwd: Some(root.clone()),
    })
    .await
    .expect("run explicit workflow command");
    let run_id = serde_json::from_str::<serde_json::Value>(&run)
        .expect("parse explicit workflow command output")
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("explicit workflow run id")
        .to_string();

    let runs = workflow_command_text(WorkflowCommands::Runs {
        workflow_name: None,
        script_path: Some(PathBuf::from("scripts/custom-release.json")),
        limit: 20,
        cwd: Some(root.clone()),
    })
    .await
    .expect("list explicit workflow runs");
    assert!(runs.contains(&run_id));
    assert!(runs.contains(&format!(
        "hellox workflow last-run --script-path {absolute_script_path}"
    )));

    let latest = workflow_command_text(WorkflowCommands::LastRun {
        workflow_name: None,
        script_path: Some(PathBuf::from("scripts/custom-release.json")),
        step: Some(1),
        cwd: Some(root),
    })
    .await
    .expect("show latest explicit workflow run");
    assert!(latest.contains(&run_id));
    assert!(latest.contains(&format!(
        "hellox workflow runs --script-path {absolute_script_path}"
    )));
    assert!(latest.contains(&format!(
        "/workflow last-run --script-path {absolute_script_path}"
    )));
}

#[tokio::test]
async fn workflow_dashboard_command_renders_initial_view() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "name": "review", "prompt": "review release notes" }] }"#,
    );

    let text = workflow_command_text(WorkflowCommands::Dashboard {
        workflow_name: Some("release-review".to_string()),
        script_path: None,
        cwd: Some(root),
    })
    .await
    .expect("render workflow dashboard");

    assert!(text.contains("Workflow overview: release-review"));
    assert!(text.contains("== Dashboard =="));
}

#[tokio::test]
async fn workflow_dashboard_command_supports_explicit_script_path() {
    let root = temp_dir();
    write_explicit_workflow(
        &root,
        "scripts/custom-release.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
    );

    let text = workflow_command_text(WorkflowCommands::Dashboard {
        workflow_name: None,
        script_path: Some(PathBuf::from("scripts/custom-release.json")),
        cwd: Some(root),
    })
    .await
    .expect("render workflow dashboard by script path");

    assert!(text.contains("Workflow overview: scripts/custom-release"));
    assert!(text.contains("focus: `/workflow panel --script-path"));
    assert!(text.contains("scripts/custom-release.json"));
}

#[tokio::test]
async fn workflow_overview_command_supports_explicit_script_path() {
    let root = temp_dir();
    write_explicit_workflow(
        &root,
        "scripts/custom-release.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" }
  ]
}"#,
    );

    let text = workflow_command_text(WorkflowCommands::Overview {
        workflow_name: None,
        script_path: Some(PathBuf::from("scripts/custom-release.json")),
        cwd: Some(root),
    })
    .await
    .expect("render workflow overview by script path");

    assert!(text.contains("Workflow overview: scripts/custom-release"));
    assert!(text.contains("/workflow panel --script-path"));
}
