use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{HelloxConfig, PermissionMode};

use super::{ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-workflow-shortcuts-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session_in(root: PathBuf) -> AgentSession {
    let config_path = root.join(".hellox").join("config.toml");
    AgentSession::create(
        GatewayClient::new("http://127.0.0.1:7821"),
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

fn write_explicit_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("explicit workflow dir"))
        .expect("create explicit workflow dir");
    fs::write(path, raw).expect("write explicit workflow");
}

fn write_run(root: &Path, run_id: &str, workflow_name: &str) {
    let path = root
        .join(".hellox")
        .join("workflow-runs")
        .join(format!("{run_id}.json"));
    fs::create_dir_all(path.parent().expect("run dir")).expect("create run dir");
    fs::write(
        path,
        serde_json::to_string_pretty(&serde_json::json!({
            "run_id": run_id,
            "status": "completed",
            "workflow_name": workflow_name,
            "workflow_source": format!(".hellox/workflows/{workflow_name}.json"),
            "started_at": 1,
            "finished_at": 2,
            "summary": {
                "total_steps": 2,
                "completed_steps": 2,
                "failed_steps": 0,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [
                { "name": "review", "status": "completed", "result_text": "ok" },
                { "name": "ship", "status": "completed", "result_text": "done" }
            ],
            "result_text": "done"
        }))
        .expect("serialize run"),
    )
    .expect("write run");
}

#[test]
fn workflow_panel_shortcuts_submit_without_panel_context() {
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
                    .handle_repl_input_async("dup", &mut session, &metadata)
                    .await
                    .expect("submit plain text"),
                ReplAction::Submit(String::from("dup"))
            );
        });
}

#[test]
fn workflow_panel_shortcuts_keep_focus_and_refresh_script() {
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
                        "/workflow panel release-review 2",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open workflow panel"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("release-review"),
                    script_path: None,
                    selected_step: 2,
                })
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("dup", &mut session, &metadata)
                    .await
                    .expect("duplicate focused step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("release-review"),
                    script_path: None,
                    selected_step: 3,
                })
            );
            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load duplicated workflow");
            assert_eq!(detail.steps.len(), 3);
            assert_eq!(detail.steps[2].name.as_deref(), Some("ship copy"));

            assert_eq!(
                driver
                    .handle_repl_input_async("move 1", &mut session, &metadata)
                    .await
                    .expect("move focused step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("release-review"),
                    script_path: None,
                    selected_step: 1,
                })
            );
            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load moved workflow");
            assert_eq!(detail.steps[0].name.as_deref(), Some("ship copy"));

            assert_eq!(
                driver
                    .handle_repl_input_async("rm", &mut session, &metadata)
                    .await
                    .expect("remove focused step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("release-review"),
                    script_path: None,
                    selected_step: 1,
                })
            );
            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load removed workflow");
            assert_eq!(detail.steps.len(), 2);
            assert_eq!(detail.steps[0].name.as_deref(), Some("review"));
            assert_eq!(detail.steps[1].name.as_deref(), Some("ship"));

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
                other => panic!("expected workflow panel selector context, got {other:?}"),
            }
        });
}

#[test]
fn workflow_panel_shortcuts_support_field_edits() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();

    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release" }
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
                        "/workflow panel release-review 1",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open focused workflow panel"),
                ReplAction::Continue
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("name ship review", &mut session, &metadata)
                    .await
                    .expect("rename focused step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver
                    .handle_repl_input_async("backend detached_process", &mut session, &metadata)
                    .await
                    .expect("set backend"),
                ReplAction::Continue
            );
            assert_eq!(
                driver
                    .handle_repl_input_async("background", &mut session, &metadata)
                    .await
                    .expect("set background mode"),
                ReplAction::Continue
            );
            assert_eq!(
                driver
                    .handle_repl_input_async("clear-name", &mut session, &metadata)
                    .await
                    .expect("clear name"),
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
    let step = value
        .get("steps")
        .and_then(serde_json::Value::as_array)
        .and_then(|steps| steps.first())
        .expect("workflow step");
    assert!(step.get("name").is_none());
    assert_eq!(
        step.get("backend").and_then(serde_json::Value::as_str),
        Some("detached_process")
    );
    assert_eq!(
        step.get("run_in_background")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        driver.workflow_panel_focus(),
        Some(super::workflow_selectors::WorkflowPanelFocus {
            workflow_name: String::from("release-review"),
            script_path: None,
            selected_step: 1,
        })
    );
}

#[test]
fn workflow_step_navigation_shortcuts_keep_focus_in_panel_and_run_views() {
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
    write_run(&root, "run-123", "release-review");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "/workflow panel release-review 1",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open focused workflow panel"),
                ReplAction::Continue
            );
            assert_eq!(
                driver
                    .handle_repl_input_async("next", &mut session, &metadata)
                    .await
                    .expect("focus next workflow step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("release-review"),
                    script_path: None,
                    selected_step: 2,
                })
            );

            assert_eq!(
                driver
                    .handle_repl_input_async(
                        "/workflow show-run run-123 1",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open workflow run inspect"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_run_focus(),
                Some(super::workflow_selectors::WorkflowRunFocus {
                    run_id: String::from("run-123"),
                    selected_step: 1,
                })
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("last", &mut session, &metadata)
                    .await
                    .expect("focus last recorded step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_run_focus(),
                Some(super::workflow_selectors::WorkflowRunFocus {
                    run_id: String::from("run-123"),
                    selected_step: 2,
                })
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowRunSteps { run_id, step_count }) => {
                    assert_eq!(run_id, "run-123");
                    assert_eq!(step_count, 2);
                }
                other => panic!("expected workflow run selector context, got {other:?}"),
            }
        });
}

#[test]
fn workflow_panel_shortcut_usage_keeps_context() {
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
                        "/workflow panel release-review 1",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open focused workflow panel"),
                ReplAction::Continue
            );
            assert_eq!(
                driver
                    .handle_repl_input_async("move", &mut session, &metadata)
                    .await
                    .expect("show shortcut usage"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("release-review"),
                    script_path: None,
                    selected_step: 1,
                })
            );
            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load unchanged workflow");
            assert_eq!(detail.steps.len(), 1);
            assert_eq!(detail.steps[0].name.as_deref(), Some("review"));
        });
}

#[test]
fn workflow_panel_shortcuts_support_explicit_script_path_focus() {
    let root = temp_dir();
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());
    let driver = super::CliReplDriver::new();
    let absolute_script_path = root
        .join("scripts")
        .join("custom-release.json")
        .display()
        .to_string()
        .replace('\\', "/");

    write_explicit_workflow(
        &root,
        "scripts/custom-release.json",
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
                        "/workflow panel --script-path scripts/custom-release.json 2",
                        &mut session,
                        &metadata
                    )
                    .await
                    .expect("open explicit workflow panel"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("scripts/custom-release"),
                    script_path: Some(absolute_script_path.clone()),
                    selected_step: 2,
                })
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("dup", &mut session, &metadata)
                    .await
                    .expect("duplicate explicit focused step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("scripts/custom-release"),
                    script_path: Some(absolute_script_path.clone()),
                    selected_step: 3,
                })
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("move 1", &mut session, &metadata)
                    .await
                    .expect("move explicit focused step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("scripts/custom-release"),
                    script_path: Some(absolute_script_path.clone()),
                    selected_step: 1,
                })
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("rm", &mut session, &metadata)
                    .await
                    .expect("remove explicit focused step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("scripts/custom-release"),
                    script_path: Some(absolute_script_path.clone()),
                    selected_step: 1,
                })
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("next", &mut session, &metadata)
                    .await
                    .expect("focus next explicit workflow step"),
                ReplAction::Continue
            );
            assert_eq!(
                driver.workflow_panel_focus(),
                Some(super::workflow_selectors::WorkflowPanelFocus {
                    workflow_name: String::from("scripts/custom-release"),
                    script_path: Some(absolute_script_path.clone()),
                    selected_step: 2,
                })
            );

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowPanelPathItems {
                    script_path,
                    workflow_name,
                    step_count,
                    items,
                }) => {
                    assert_eq!(script_path, absolute_script_path);
                    assert_eq!(workflow_name, "scripts/custom-release");
                    assert_eq!(step_count, 2);
                    assert_eq!(
                        items,
                        vec![
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(1),
                            crate::workflow_panel::WorkflowPanelSelectionItem::Step(2),
                        ]
                    );
                }
                other => panic!("expected explicit workflow panel selector context, got {other:?}"),
            }
        });
}
