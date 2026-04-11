use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::load_or_default;
use hellox_config::HelloxConfig;
use hellox_config::PermissionMode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::commands::WorkflowCommand;
use super::format::help_text_for_workdir;
use super::workflow_actions::{handle_workflow_command, resolve_dynamic_workflow_invocation};
use super::{ReplAction, ReplMetadata};
use crate::workflow_overview::WorkflowOverviewSelectionItem;

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-workflow-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session_in(root: PathBuf) -> AgentSession {
    let config_path = root.join(".hellox").join("config.toml");
    let gateway = load_or_default(Some(config_path.clone()))
        .map(|config| GatewayClient::from_config(&config, None))
        .unwrap_or_else(|_| GatewayClient::new("http://127.0.0.1:7821"));
    AgentSession::create(
        gateway,
        default_tool_registry(),
        config_path,
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

fn metadata_in(root: &Path) -> ReplMetadata {
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: root.join(".hellox").join("config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join(".hellox").join("plugins"),
        sessions_root: root.join(".hellox").join("sessions"),
        shares_root: root.join("shares"),
    }
}

fn write_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(".hellox").join("workflows").join(relative);
    fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
    fs::write(path, raw).expect("write workflow");
}

fn write_config(root: &Path, base_url: &str) {
    let config = format!(
        "[gateway]\nlisten = \"{}\"\n\n[session]\npersist = false\nmodel = \"mock-model\"\n",
        base_url
    );
    let path = root.join(".hellox").join("config.toml");
    fs::create_dir_all(path.parent().expect("config dir")).expect("create config dir");
    fs::write(path, config).expect("write config");
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
                "id": "repl-workflow-response",
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
fn help_text_lists_project_workflow_commands() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "prompt": "review release" }] }"#,
    );

    let text = help_text_for_workdir(&root);
    assert!(text.contains("/workflow dashboard [name]"));
    assert!(text.contains("/workflow overview [name]"));
    assert!(text.contains("/workflow panel [name] [n]"));
    assert!(text.contains("/workflow runs [name]"));
    assert!(text.contains("/workflow validate [name]"));
    assert!(text.contains("/workflow show-run <id> [n]"));
    assert!(text.contains("/workflow last-run [name] [n]"));
    assert!(text.contains("/workflow init <name>"));
    assert!(text.contains("/workflow add-step <name> --prompt <text>"));
    assert!(text.contains("/workflow run <name> [shared_context]"));
    assert!(text.contains("/release-review [shared_context]"));
}

#[test]
fn resolves_dynamic_workflow_command_by_script_name() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "prompt": "review release" }] }"#,
    );

    let resolved = resolve_dynamic_workflow_invocation("/release-review ship carefully", &root)
        .expect("resolve dynamic workflow");
    assert_eq!(
        resolved,
        Some((
            String::from("release-review"),
            Some(String::from("ship carefully"))
        ))
    );
}

#[test]
fn handle_workflow_validate_and_init_commands() {
    let root = temp_dir();
    write_workflow(&root, "broken.json", "{ not-json");
    let mut session = session_in(root.clone());

    let validation = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime")
        .block_on(handle_workflow_command(
            WorkflowCommand::Validate {
                workflow_name: None,
            },
            &mut session,
        ))
        .expect("validate workflow from repl");
    assert!(validation.contains("broken"));
    assert!(validation.contains("invalid"));

    let initialized = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime")
        .block_on(handle_workflow_command(
            WorkflowCommand::Init {
                workflow_name: Some(String::from("release-review")),
            },
            &mut session,
        ))
        .expect("init workflow from repl");
    assert!(initialized.contains("Initialized workflow `release-review`"));
    assert!(root
        .join(".hellox")
        .join("workflows")
        .join("release-review.json")
        .exists());
}

#[test]
fn parse_workflow_authoring_commands() {
    assert_eq!(
        super::commands::parse_command("/workflow dashboard release-review"),
        Some(super::commands::ReplCommand::Workflow(
            WorkflowCommand::Dashboard {
                workflow_name: Some(String::from("release-review")),
            }
        ))
    );
    assert_eq!(
        super::commands::parse_command("/workflow panel release-review 2"),
        Some(super::commands::ReplCommand::Workflow(
            WorkflowCommand::Panel {
                workflow_name: Some(String::from("release-review")),
                step_number: Some(2),
            }
        ))
    );
    assert_eq!(
        super::commands::parse_command("/workflow overview release-review"),
        Some(super::commands::ReplCommand::Workflow(
            WorkflowCommand::Overview {
                workflow_name: Some(String::from("release-review")),
            }
        ))
    );
    assert_eq!(
        super::commands::parse_command("/workflow runs release-review"),
        Some(super::commands::ReplCommand::Workflow(
            WorkflowCommand::Runs {
                workflow_name: Some(String::from("release-review")),
            }
        ))
    );
    assert_eq!(
        super::commands::parse_command("/workflow show-run run-123 2"),
        Some(super::commands::ReplCommand::Workflow(
            WorkflowCommand::ShowRun {
                run_id: Some(String::from("run-123")),
                step_number: Some(2),
            }
        ))
    );
    assert_eq!(
        super::commands::parse_command("/workflow last-run release-review 3"),
        Some(super::commands::ReplCommand::Workflow(
            WorkflowCommand::LastRun {
                workflow_name: Some(String::from("release-review")),
                step_number: Some(3),
            }
        ))
    );
    assert_eq!(
        super::commands::parse_command(
            "/workflow add-step release-review --prompt review release notes --name review --background"
        ),
        Some(super::commands::ReplCommand::Workflow(WorkflowCommand::AddStep {
            workflow_name: Some(String::from("release-review")),
            name: Some(String::from("review")),
            prompt: Some(String::from("review release notes")),
            index: None,
            when: None,
            model: None,
            backend: None,
            step_cwd: None,
            run_in_background: true,
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/workflow update-step release-review 2 --clear-name --prompt summarize findings --foreground"
        ),
        Some(super::commands::ReplCommand::Workflow(WorkflowCommand::UpdateStep {
            workflow_name: Some(String::from("release-review")),
            step_number: Some(2),
            name: None,
            clear_name: true,
            prompt: Some(String::from("summarize findings")),
            when: None,
            clear_when: false,
            model: None,
            clear_model: false,
            backend: None,
            clear_backend: false,
            step_cwd: None,
            clear_step_cwd: false,
            run_in_background: Some(false),
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/workflow set-shared-context release-review ship carefully"
        ),
        Some(super::commands::ReplCommand::Workflow(
            WorkflowCommand::SetSharedContext {
                workflow_name: Some(String::from("release-review")),
                value: Some(String::from("ship carefully")),
            }
        ))
    );
}

#[test]
fn workflow_dashboard_mode_navigates_and_closes() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "/workflow dashboard release-review",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open workflow dashboard"),
                ReplAction::Continue
            );
            assert!(driver.workflow_dashboard_state().is_some());

            assert_eq!(
                driver
                    .handle_repl_input_async("panel 2", &mut session, &metadata)
                    .await
                    .expect("focus workflow panel from dashboard"),
                ReplAction::Continue
            );
            let state = driver
                .workflow_dashboard_state()
                .expect("workflow dashboard state");
            match state.current() {
                hellox_tui::WorkflowDashboardView::PanelFocus {
                    workflow_name,
                    step_number,
                } => {
                    assert_eq!(workflow_name, "release-review");
                    assert_eq!(*step_number, Some(2));
                }
                other => panic!("expected workflow panel focus, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("close", &mut session, &metadata)
                    .await
                    .expect("close workflow dashboard"),
                ReplAction::Continue
            );
            assert!(driver.workflow_dashboard_state().is_none());
        });
}

#[test]
fn workflow_dashboard_mode_supports_context_updates() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "/workflow dashboard release-review",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open workflow dashboard"),
                ReplAction::Continue
            );

            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "set-shared-context ship carefully",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("set workflow shared_context"),
                ReplAction::Continue
            );
            let state = driver
                .workflow_dashboard_state()
                .expect("workflow dashboard state");
            match state.current() {
                hellox_tui::WorkflowDashboardView::OverviewFocus { workflow_name } => {
                    assert_eq!(workflow_name, "release-review");
                }
                other => panic!("expected workflow overview focus, got {other:?}"),
            }

            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load updated workflow");
            assert_eq!(
                detail.summary.shared_context,
                Some(String::from("ship carefully"))
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("enable-continue-on-error", &mut session, &metadata)
                    .await
                    .expect("enable continue_on_error"),
                ReplAction::Continue
            );

            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load updated workflow");
            assert!(detail.summary.continue_on_error);
        });
}

#[test]
fn workflow_dashboard_mode_supports_step_authoring() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "/workflow dashboard release-review",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open workflow dashboard"),
                ReplAction::Continue
            );

            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "add-step --prompt summarize findings --name summarize --background",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("add workflow step from dashboard"),
                ReplAction::Continue
            );

            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "update-step --clear-name --prompt ship release --foreground",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("update workflow step from dashboard"),
                ReplAction::Continue
            );
        });

    let raw = fs::read_to_string(
        root.join(".hellox")
            .join("workflows")
            .join("release-review.json"),
    )
    .expect("read workflow json");
    let value = serde_json::from_str::<serde_json::Value>(&raw).expect("parse workflow json");
    let steps = value
        .get("steps")
        .and_then(serde_json::Value::as_array)
        .expect("workflow steps");
    assert_eq!(steps.len(), 2);
    assert!(steps[1].get("name").is_none());
    assert_eq!(
        steps[1].get("prompt").and_then(serde_json::Value::as_str),
        Some("ship release")
    );
    assert_eq!(
        steps[1]
            .get("run_in_background")
            .and_then(serde_json::Value::as_bool),
        None
    );

    let state = driver
        .workflow_dashboard_state()
        .expect("workflow dashboard state");
    match state.current() {
        hellox_tui::WorkflowDashboardView::PanelFocus {
            workflow_name,
            step_number,
        } => {
            assert_eq!(workflow_name, "release-review");
            assert_eq!(*step_number, Some(2));
        }
        other => panic!("expected workflow panel focus after authoring, got {other:?}"),
    }
}

#[tokio::test]
async fn workflow_dashboard_mode_runs_active_workflow() {
    let root = temp_dir();
    let base_url = spawn_mock_gateway("workflow dashboard done").await;
    write_config(&root, &base_url);
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
    );

    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    assert_eq!(
        driver
            .handle_repl_input_async(
                "/workflow dashboard release-review",
                &mut session,
                &metadata
            )
            .await
            .expect("open workflow dashboard"),
        ReplAction::Continue
    );

    assert_eq!(
        driver
            .handle_repl_input_async("run ship carefully", &mut session, &metadata)
            .await
            .expect("run workflow from dashboard"),
        ReplAction::Continue
    );

    let run_ids = fs::read_dir(root.join(".hellox").join("workflow-runs"))
        .expect("read workflow runs dir")
        .map(|entry| {
            entry
                .expect("workflow run entry")
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .expect("workflow run id")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(run_ids.len(), 1);

    let state = driver
        .workflow_dashboard_state()
        .expect("workflow dashboard state");
    match state.current() {
        hellox_tui::WorkflowDashboardView::RunInspect {
            run_id,
            step_number,
        } => {
            assert_eq!(run_id, &run_ids[0]);
            assert_eq!(*step_number, None);
        }
        other => panic!("expected workflow run inspect view, got {other:?}"),
    }
}

#[tokio::test]
async fn handle_workflow_run_command_executes_local_script() {
    let root = temp_dir();
    let base_url = spawn_mock_gateway("workflow repl done").await;
    write_config(&root, &base_url);
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
    );

    let mut session = session_in(root.clone());
    let text = handle_workflow_command(
        WorkflowCommand::Run {
            workflow_name: Some(String::from("release-review")),
            shared_context: Some(String::from("ship carefully")),
        },
        &mut session,
    )
    .await
    .expect("run workflow from repl");

    let run_id = serde_json::from_str::<serde_json::Value>(&text)
        .expect("parse workflow repl run output")
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("workflow run id")
        .to_string();

    assert!(text.contains("\"workflow_source\": \".hellox/workflows/release-review.json\""));
    assert!(text.contains("workflow repl done"));
    assert!(root
        .join(".hellox")
        .join("workflow-runs")
        .join(format!("{run_id}.json"))
        .exists());

    let runs = handle_workflow_command(
        WorkflowCommand::Runs {
            workflow_name: Some(String::from("release-review")),
        },
        &mut session,
    )
    .await
    .expect("list workflow runs from repl");
    assert!(runs.contains(&run_id));

    let detail = handle_workflow_command(
        WorkflowCommand::ShowRun {
            run_id: Some(run_id.clone()),
            step_number: Some(1),
        },
        &mut session,
    )
    .await
    .expect("show workflow run from repl");
    assert!(detail.contains("Workflow run inspect panel:"));
    assert!(detail.contains("release-review"));
    assert!(detail.contains("== Visual execution map =="));
    assert!(detail.contains("== REPL palette =="));

    let latest = handle_workflow_command(
        WorkflowCommand::LastRun {
            workflow_name: Some(String::from("release-review")),
            step_number: Some(1),
        },
        &mut session,
    )
    .await
    .expect("show latest workflow run from repl");
    assert!(latest.contains(&run_id));
    assert!(latest.contains("== CLI palette =="));

    let overview = handle_workflow_command(
        WorkflowCommand::Overview {
            workflow_name: Some(String::from("release-review")),
        },
        &mut session,
    )
    .await
    .expect("show workflow overview from repl");
    assert!(overview.contains("Workflow overview: release-review"));
    assert!(overview.contains("== Latest run snapshot =="));
    assert!(overview.contains(&run_id));

    let panel = handle_workflow_command(
        WorkflowCommand::Panel {
            workflow_name: Some(String::from("release-review")),
            step_number: Some(1),
        },
        &mut session,
    )
    .await
    .expect("show workflow panel from repl");
    assert!(panel.contains("Workflow authoring panel: release-review"));
    assert!(panel.contains("== Step selector =="));
    assert!(panel.contains("== Action palette =="));
    assert!(panel.contains("/workflow update-step release-review 1 --prompt <text>"));
}

#[tokio::test]
async fn handle_workflow_authoring_commands_edit_local_script() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" }
  ]
}"#,
    );

    let mut session = session_in(root.clone());
    let added = handle_workflow_command(
        WorkflowCommand::AddStep {
            workflow_name: Some(String::from("release-review")),
            name: Some(String::from("summarize")),
            prompt: Some(String::from("summarize findings")),
            index: Some(2),
            when: Some(String::from(r#"{"previous_status":"completed"}"#)),
            model: Some(String::from("mock-model")),
            backend: None,
            step_cwd: Some(String::from("docs")),
            run_in_background: true,
        },
        &mut session,
    )
    .await
    .expect("add step from repl");
    assert!(added.contains("Added workflow step 2"));
    assert!(added.contains("background=true"));

    let updated = handle_workflow_command(
        WorkflowCommand::UpdateStep {
            workflow_name: Some(String::from("release-review")),
            step_number: Some(2),
            name: None,
            clear_name: true,
            prompt: Some(String::from("ship release")),
            when: None,
            clear_when: true,
            model: None,
            clear_model: true,
            backend: Some(String::from("detached_process")),
            clear_backend: false,
            step_cwd: None,
            clear_step_cwd: true,
            run_in_background: Some(false),
        },
        &mut session,
    )
    .await
    .expect("update step from repl");
    assert!(updated.contains("Updated workflow step 2"));
    assert!(updated.contains("backend=detached_process"));

    let context = handle_workflow_command(
        WorkflowCommand::SetSharedContext {
            workflow_name: Some(String::from("release-review")),
            value: Some(String::from("ship carefully")),
        },
        &mut session,
    )
    .await
    .expect("set shared context from repl");
    assert!(context.contains("shared_context: ship carefully"));

    let enabled = handle_workflow_command(
        WorkflowCommand::EnableContinueOnError {
            workflow_name: Some(String::from("release-review")),
        },
        &mut session,
    )
    .await
    .expect("enable continue_on_error from repl");
    assert!(enabled.contains("continue_on_error: true"));

    let removed = handle_workflow_command(
        WorkflowCommand::RemoveStep {
            workflow_name: Some(String::from("release-review")),
            step_number: Some(1),
        },
        &mut session,
    )
    .await
    .expect("remove step from repl");
    assert!(removed.contains("Removed workflow step 1"));

    let cleared = handle_workflow_command(
        WorkflowCommand::ClearSharedContext {
            workflow_name: Some(String::from("release-review")),
        },
        &mut session,
    )
    .await
    .expect("clear shared context from repl");
    assert!(cleared.contains("shared_context: (none)"));
}

#[test]
fn workflow_numeric_input_without_selector_context_submits_prompt() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root);
    let driver = super::CliReplDriver::new();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("submit"),
                ReplAction::Submit(String::from("1"))
            );
        });
}

#[test]
fn workflow_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    write_workflow(
        &root,
        "alpha.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review alpha" }
  ]
}"#,
    );
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("/workflow panel", &mut session, &metadata)
                    .await
                    .expect("open workflow panel selector"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowPanelList { workflow_names }) => {
                    assert_eq!(
                        workflow_names,
                        vec!["alpha".to_string(), "release-review".to_string()]
                    );
                }
                other => panic!("expected workflow panel selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("2", &mut session, &metadata)
                    .await
                    .expect("select workflow"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowPanelItems {
                    workflow_name,
                    step_count,
                    items,
                }) => {
                    assert_eq!(workflow_name, "release-review");
                    assert_eq!(step_count, 2);
                    assert_eq!(
                        items,
                        vec![
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(1),
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(2),
                        ]
                    );
                }
                other => panic!("expected workflow step selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("2", &mut session, &metadata)
                    .await
                    .expect("select workflow step"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowPanelItems {
                    workflow_name,
                    step_count,
                    items,
                }) => {
                    assert_eq!(workflow_name, "release-review");
                    assert_eq!(step_count, 2);
                    assert_eq!(
                        items,
                        vec![
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(1),
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(2),
                        ]
                    );
                }
                other => panic!("expected persistent workflow step selector, got {other:?}"),
            }
        });
}

#[tokio::test]
async fn workflow_runs_selector_allows_numeric_selection() {
    let root = temp_dir();
    let base_url = spawn_mock_gateway("workflow repl done").await;
    write_config(&root, &base_url);
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
    );

    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    let run_text = handle_workflow_command(
        WorkflowCommand::Run {
            workflow_name: Some(String::from("release-review")),
            shared_context: Some(String::from("ship carefully")),
        },
        &mut session,
    )
    .await
    .expect("run workflow from repl");
    let run_id = serde_json::from_str::<serde_json::Value>(&run_text)
        .expect("parse workflow run output")
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("workflow run id")
        .to_string();

    assert_eq!(
        driver
            .handle_repl_input_async("/workflow runs release-review", &mut session, &metadata)
            .await
            .expect("open workflow runs panel"),
        ReplAction::Continue
    );

    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowRunList { run_ids }) => {
            assert_eq!(run_ids, vec![run_id.clone()]);
        }
        other => panic!("expected workflow run selector context, got {other:?}"),
    }

    assert_eq!(
        driver
            .handle_repl_input_async("1", &mut session, &metadata)
            .await
            .expect("select workflow run"),
        ReplAction::Continue
    );
    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowRunSteps {
            run_id: selected_run_id,
            step_count,
        }) => {
            assert_eq!(selected_run_id, run_id);
            assert_eq!(step_count, 1);
        }
        other => panic!("expected workflow run step selector after opening run, got {other:?}"),
    }
}

#[tokio::test]
async fn workflow_panel_focus_allows_numeric_recent_run_selection() {
    let root = temp_dir();
    let base_url = spawn_mock_gateway("workflow repl done").await;
    write_config(&root, &base_url);
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
    );

    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    let run_text = handle_workflow_command(
        WorkflowCommand::Run {
            workflow_name: Some(String::from("release-review")),
            shared_context: Some(String::from("ship carefully")),
        },
        &mut session,
    )
    .await
    .expect("run workflow from repl");
    let run_id = serde_json::from_str::<serde_json::Value>(&run_text)
        .expect("parse workflow run output")
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("workflow run id")
        .to_string();

    assert_eq!(
        driver
            .handle_repl_input_async("/workflow panel release-review", &mut session, &metadata)
            .await
            .expect("open workflow panel"),
        ReplAction::Continue
    );

    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowPanelItems {
            workflow_name,
            step_count,
            items,
        }) => {
            assert_eq!(workflow_name, "release-review");
            assert_eq!(step_count, 1);
            assert_eq!(
                items,
                vec![
                    crate::workflow_panel::WorkflowPanelSelectionItem::Step(1),
                    crate::workflow_panel::WorkflowPanelSelectionItem::Run(run_id.clone()),
                ]
            );
        }
        other => panic!("expected workflow panel selection items, got {other:?}"),
    }

    assert_eq!(
        driver
            .handle_repl_input_async("2", &mut session, &metadata)
            .await
            .expect("select recent run from panel"),
        ReplAction::Continue
    );
    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowRunSteps {
            run_id: selected_run_id,
            step_count,
        }) => {
            assert_eq!(selected_run_id, run_id);
            assert_eq!(step_count, 1);
        }
        other => panic!("expected workflow run selector after opening recent run, got {other:?}"),
    }
}

#[test]
fn workflow_overview_focus_allows_numeric_step_selection() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "/workflow overview release-review",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open workflow overview focus"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowOverviewFocusItems {
                    workflow_name,
                    items,
                }) => {
                    assert_eq!(workflow_name, "release-review");
                    assert_eq!(
                        items,
                        vec![
                            crate::workflow_overview::WorkflowOverviewFocusSelectionItem::Step(1),
                            crate::workflow_overview::WorkflowOverviewFocusSelectionItem::Step(2),
                        ]
                    );
                }
                other => panic!("expected workflow overview step selector, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("2", &mut session, &metadata)
                    .await
                    .expect("focus workflow step from overview"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowPanelItems {
                    workflow_name,
                    step_count,
                    items,
                }) => {
                    assert_eq!(workflow_name, "release-review");
                    assert_eq!(step_count, 2);
                    assert_eq!(
                        items,
                        vec![
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(1),
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(2),
                        ]
                    );
                }
                other => panic!("expected workflow panel step selector, got {other:?}"),
            }
        });
}

#[tokio::test]
async fn workflow_overview_focus_allows_numeric_recent_run_selection() {
    let root = temp_dir();
    let base_url = spawn_mock_gateway("workflow repl done").await;
    write_config(&root, &base_url);
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
    );

    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    let run_text = handle_workflow_command(
        WorkflowCommand::Run {
            workflow_name: Some(String::from("release-review")),
            shared_context: Some(String::from("ship carefully")),
        },
        &mut session,
    )
    .await
    .expect("run workflow from repl");
    let run_id = serde_json::from_str::<serde_json::Value>(&run_text)
        .expect("parse workflow run output")
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("workflow run id")
        .to_string();

    assert_eq!(
        driver
            .handle_repl_input_async("/workflow overview release-review", &mut session, &metadata)
            .await
            .expect("open workflow overview focus"),
        ReplAction::Continue
    );

    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowOverviewFocusItems {
            workflow_name,
            items,
        }) => {
            assert_eq!(workflow_name, "release-review");
            assert_eq!(
                items,
                vec![
                    crate::workflow_overview::WorkflowOverviewFocusSelectionItem::Step(1),
                    crate::workflow_overview::WorkflowOverviewFocusSelectionItem::Run(
                        run_id.clone()
                    ),
                ]
            );
        }
        other => panic!("expected workflow overview selection items, got {other:?}"),
    }

    assert_eq!(
        driver
            .handle_repl_input_async("2", &mut session, &metadata)
            .await
            .expect("select recent run from overview"),
        ReplAction::Continue
    );
    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowRunSteps {
            run_id: selected_run_id,
            step_count,
        }) => {
            assert_eq!(selected_run_id, run_id);
            assert_eq!(step_count, 1);
        }
        other => panic!("expected workflow run selector after overview recent run, got {other:?}"),
    }
}

#[tokio::test]
async fn workflow_show_run_allows_numeric_step_selection() {
    let root = temp_dir();
    let base_url = spawn_mock_gateway("workflow repl done").await;
    write_config(&root, &base_url);
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "continue_on_error": true,
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" },
    { "name": "ship", "prompt": "ship release" }
  ]
}"#,
    );

    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    let run_text = handle_workflow_command(
        WorkflowCommand::Run {
            workflow_name: Some(String::from("release-review")),
            shared_context: Some(String::from("ship carefully")),
        },
        &mut session,
    )
    .await
    .expect("run workflow from repl");
    let run_id = serde_json::from_str::<serde_json::Value>(&run_text)
        .expect("parse workflow run output")
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("workflow run id")
        .to_string();

    assert_eq!(
        driver
            .handle_repl_input_async(
                &format!("/workflow show-run {run_id}"),
                &mut session,
                &metadata
            )
            .await
            .expect("open workflow show-run"),
        ReplAction::Continue
    );

    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowRunSteps {
            run_id: selected_run_id,
            step_count,
        }) => {
            assert_eq!(selected_run_id, run_id);
            assert_eq!(step_count, 2);
        }
        other => panic!("expected workflow run step selector, got {other:?}"),
    }

    assert_eq!(
        driver
            .handle_repl_input_async("1", &mut session, &metadata)
            .await
            .expect("focus recorded run step"),
        ReplAction::Continue
    );

    match driver.selector_context() {
        Some(super::SelectorContext::WorkflowRunSteps {
            run_id: selected_run_id,
            step_count,
        }) => {
            assert_eq!(selected_run_id, run_id);
            assert_eq!(step_count, 2);
        }
        other => panic!("expected persistent workflow run step selector, got {other:?}"),
    }
}

#[test]
fn workflow_overview_selector_uses_global_numbering_order() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    write_workflow(
        &root,
        "alpha.json",
        r#"{ "steps": [{ "prompt": "alpha" }] }"#,
    );
    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "prompt": "release" }] }"#,
    );
    fs::create_dir_all(root.join(".hellox").join("workflow-runs")).expect("create runs dir");
    fs::write(
        root.join(".hellox")
            .join("workflow-runs")
            .join("run-200.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "run_id": "run-200",
            "status": "failed",
            "workflow_name": null,
            "workflow_source": "scripts/custom-release.json",
            "requested_script_path": "scripts/custom-release.json",
            "started_at": 3,
            "finished_at": 4,
            "shared_context": null,
            "continue_on_error": null,
            "summary": {
                "total_steps": 0,
                "completed_steps": 0,
                "failed_steps": 1,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [],
            "error": "boom",
            "result_text": "boom"
        }))
        .expect("serialize run"),
    )
    .expect("write run");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("/workflow overview", &mut session, &metadata)
                    .await
                    .expect("open workflow overview"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowOverviewList { items }) => {
                    assert_eq!(
                        items,
                        vec![
                            WorkflowOverviewSelectionItem::Workflow(String::from("alpha")),
                            WorkflowOverviewSelectionItem::Workflow(String::from("release-review")),
                            WorkflowOverviewSelectionItem::Run(String::from("run-200")),
                        ]
                    );
                }
                other => panic!("expected workflow overview selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("3", &mut session, &metadata)
                    .await
                    .expect("select custom run from overview"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}
