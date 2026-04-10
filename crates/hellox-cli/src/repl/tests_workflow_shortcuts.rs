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
                    selected_step: 1,
                })
            );
            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load removed workflow");
            assert_eq!(detail.steps.len(), 2);
            assert_eq!(detail.steps[0].name.as_deref(), Some("review"));
            assert_eq!(detail.steps[1].name.as_deref(), Some("ship"));

            match driver.selector_context() {
                Some(super::SelectorContext::WorkflowPanelSteps {
                    workflow_name,
                    step_count,
                }) => {
                    assert_eq!(workflow_name, "release-review");
                    assert_eq!(step_count, 2);
                }
                other => panic!("expected workflow panel selector context, got {other:?}"),
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
                    selected_step: 1,
                })
            );
            let detail = crate::workflows::load_named_workflow_detail(&root, "release-review")
                .expect("load unchanged workflow");
            assert_eq!(detail.steps.len(), 1);
            assert_eq!(detail.steps[0].name.as_deref(), Some("review"));
        });
}
