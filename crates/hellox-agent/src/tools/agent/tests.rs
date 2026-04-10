use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

use super::background::reset_background_state;
use super::coordination::{SendMessageTool, TeamRunTool};
use super::runtime::{AgentStatusTool, AgentTool, AgentWaitTool};
use super::supervision::{AgentListTool, AgentStopTool, TeamStopTool, TeamWaitTool};
use super::team::TeamStatusTool;
use super::team_registry::{TeamCreateTool, TeamDeleteTool, TeamUpdateTool};
use super::workflow::WorkflowTool;
use crate::permissions::PermissionPolicy;
use crate::planning::PlanningState;
use crate::tools::{LocalTool, ToolExecutionContext};
use crate::StoredSession;
use hellox_config::{session_file_path, PermissionMode};
use hellox_gateway_api::{
    extract_text, AnthropicCompatRequest, AnthropicCompatResponse, ContentBlock, Usage,
};

static AGENT_TEST_SEMAPHORE: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new() -> Self {
        let root = env::temp_dir().join(format!("hellox-agent-tool-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp root");
        Self { root }
    }

    fn context(&self) -> ToolExecutionContext {
        ToolExecutionContext {
            config_path: self.root.join(".hellox").join("config.toml"),
            planning_state: Arc::new(Mutex::new(PlanningState::default())),
            working_directory: self.root.clone(),
            permission_policy: PermissionPolicy::new(
                PermissionMode::BypassPermissions,
                self.root.clone(),
            ),
            approval_handler: None,
            question_handler: None,
            telemetry_sink: None,
        }
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn acquire_agent_test_guard() -> OwnedSemaphorePermit {
    AGENT_TEST_SEMAPHORE
        .clone()
        .acquire_owned()
        .await
        .expect("acquire agent test guard")
}

fn assert_command_sequence(commands: &[&str], expected: &[&[&str]], log: &str) {
    let mut offset = 0usize;
    for needles in expected {
        let relative = commands[offset..]
            .iter()
            .position(|command| needles.iter().all(|needle| command.contains(needle)))
            .unwrap_or_else(|| {
                panic!(
                    "missing command containing {:?} after offset {}:\n{}",
                    needles, offset, log
                )
            });
        offset += relative + 1;
    }
}

#[tokio::test]
async fn agent_tool_runs_nested_session_and_returns_result() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("subagent complete").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let result = AgentTool
        .call(json!({ "prompt": "Summarize the codebase." }), &context)
        .await
        .expect("run agent");

    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(text.contains("\"status\": \"completed\""), "{text}");
    assert!(text.contains("subagent complete"), "{text}");
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse result");
    let session_id = value
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .expect("session id");
    let _ = fs::remove_file(session_file_path(session_id));
}

#[tokio::test]
async fn background_agent_status_and_wait_complete() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("background done").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let started = AgentTool
        .call(
            json!({
                "prompt": "Do work in the background.",
                "session_id": format!("agent-{}", unique_suffix()),
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("start background agent");
    let started_text = match started.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let value: serde_json::Value = serde_json::from_str(&started_text).expect("parse start result");
    let session_id = value
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .expect("session id");

    let status = AgentStatusTool
        .call(json!({ "session_id": session_id }), &context)
        .await
        .expect("status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(status_text.contains(session_id), "{status_text}");

    let waited = AgentWaitTool
        .call(
            json!({
                "session_id": session_id,
                "timeout_ms": 5_000,
                "poll_interval_ms": 20
            }),
            &context,
        )
        .await
        .expect("wait");
    let waited_text = match waited.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        waited_text.contains("\"status\": \"completed\""),
        "{waited_text}"
    );
    assert!(waited_text.contains("background done"), "{waited_text}");
    let _ = fs::remove_file(session_file_path(session_id));
}

#[tokio::test]
async fn background_agent_runtime_survives_registry_reset() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway_with_delay("background persisted done", 200).await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let started = AgentTool
        .call(
            json!({
                "prompt": "Run and survive registry reset.",
                "session_id": format!("agent-{}", unique_suffix()),
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("start background agent");
    let started_text = match started.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let value: serde_json::Value = serde_json::from_str(&started_text).expect("parse start result");
    let session_id = value["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    reset_background_state().expect("simulate fresh process");

    let status = AgentStatusTool
        .call(json!({ "session_id": session_id }), &context)
        .await
        .expect("status after reset");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"status\": \"running\"")
            || status_text.contains("\"status\": \"completed\""),
        "{status_text}"
    );
    assert!(
        status_text.contains("\"background\": true"),
        "{status_text}"
    );

    let waited = AgentWaitTool
        .call(
            json!({
                "session_id": session_id,
                "timeout_ms": 5_000,
                "poll_interval_ms": 20
            }),
            &context,
        )
        .await
        .expect("wait after reset");
    let waited_text = match waited.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        waited_text.contains("\"status\": \"completed\""),
        "{waited_text}"
    );
    assert!(
        waited_text.contains("background persisted done"),
        "{waited_text}"
    );

    let _ = fs::remove_file(session_file_path(&session_id));
}

#[tokio::test]
async fn agent_supervision_lists_and_stops_background_agents() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway_with_delay("background pending", 750).await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let started = AgentTool
        .call(
            json!({
                "prompt": "Do work in the background.",
                "session_id": format!("agent-{}", unique_suffix()),
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("start background agent");
    let started_text = match started.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let started_value: serde_json::Value =
        serde_json::from_str(&started_text).expect("parse start result");
    let session_id = started_value["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let listed = AgentListTool
        .call(json!({}), &context)
        .await
        .expect("list agents");
    let listed_text = match listed.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(listed_text.contains(&session_id), "{listed_text}");
    assert!(
        listed_text.contains("\"running_agents\": 1"),
        "{listed_text}"
    );

    let stopped = AgentStopTool
        .call(
            json!({
                "session_id": session_id,
                "reason": "manual stop"
            }),
            &context,
        )
        .await
        .expect("stop background agent");
    let stopped_text = match stopped.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(stopped_text.contains("\"stopped\": true"), "{stopped_text}");
    assert!(
        stopped_text.contains("\"status\": \"cancelled\""),
        "{stopped_text}"
    );

    let waited = AgentWaitTool
        .call(
            json!({
                "session_id": session_id,
                "timeout_ms": 1_000,
                "poll_interval_ms": 20
            }),
            &context,
        )
        .await
        .expect("wait cancelled agent");
    let waited_text = match waited.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        waited_text.contains("\"status\": \"cancelled\""),
        "{waited_text}"
    );
    let _ = fs::remove_file(session_file_path(&session_id));
}

#[tokio::test]
async fn agent_list_discovers_persisted_sessions_for_reconnect() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("persisted session").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let result = AgentTool
        .call(
            json!({
                "prompt": "Complete and persist session.",
                "session_id": format!("agent-{}", unique_suffix())
            }),
            &context,
        )
        .await
        .expect("run agent");
    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let value: serde_json::Value = serde_json::from_str(&text).expect("parse result");
    let session_id = value["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    reset_background_state().expect("simulate fresh process");

    let listed = AgentListTool
        .call(json!({}), &context)
        .await
        .expect("agent list");
    let listed_text = match listed.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(listed_text.contains(&session_id), "{listed_text}");
    assert!(
        listed_text.contains("\"status\": \"completed\""),
        "{listed_text}"
    );
    assert!(
        listed_text.contains("\"completed_agents\": 1"),
        "{listed_text}"
    );
    assert!(
        listed_text.contains("\"backend\": \"in_process\""),
        "{listed_text}"
    );
    assert!(
        listed_text.contains("\"permission_mode\": \"bypass_permissions\""),
        "{listed_text}"
    );

    let _ = fs::remove_file(session_file_path(&session_id));
}

#[tokio::test]
async fn team_tools_create_message_and_delete_team() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("teammate done").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let created = TeamCreateTool
        .call(
            json!({
                "name": "reviewers",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Review the patch.",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse create team");
    let session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("member session id")
        .to_string();

    let status = TeamStatusTool
        .call(json!({ "name": "reviewers" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(status_text.contains("\"name\": \"alice\""), "{status_text}");
    assert!(status_text.contains("\"layout\""), "{status_text}");
    assert!(
        status_text.contains("\"strategy\": \"fanout\""),
        "{status_text}"
    );
    assert!(
        status_text.contains("\"layout_slot\": \"primary\""),
        "{status_text}"
    );

    AgentWaitTool
        .call(
            json!({
                "session_id": session_id,
                "timeout_ms": 5_000,
                "poll_interval_ms": 20
            }),
            &context,
        )
        .await
        .expect("wait teammate");

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "reviewers",
                "to": "alice",
                "content": "Continue the review."
            }),
            &context,
        )
        .await
        .expect("send message");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(reply_text.contains("teammate done"), "{reply_text}");

    let deleted = TeamDeleteTool
        .call(json!({ "name": "reviewers" }), &context)
        .await
        .expect("delete team");
    let deleted_text = match deleted.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(deleted_text.contains("orphaned_sessions"), "{deleted_text}");
    let _ = fs::remove_file(session_file_path(&session_id));
}

#[tokio::test]
async fn team_create_applies_horizontal_layout_slots() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("layout done").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let created = TeamCreateTool
        .call(
            json!({
                "name": "layout-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Inspect left pane",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Inspect right pane",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create layout team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        created_text.contains("\"strategy\": \"horizontal\""),
        "{created_text}"
    );
    assert!(
        created_text.contains("\"layout_slot\": \"primary\""),
        "{created_text}"
    );
    assert!(
        created_text.contains("\"layout_slot\": \"right\""),
        "{created_text}"
    );
    assert!(created_text.contains("\"pane_group\""), "{created_text}");

    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse layout create");
    for member in created_value["members"].as_array().expect("members") {
        if let Some(session_id) = member["agent"]["session_id"].as_str() {
            let _ = fs::remove_file(session_file_path(session_id));
        }
    }
}

#[tokio::test]
async fn team_update_resyncs_member_runtime_layout_slots() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("layout update done").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let created = TeamCreateTool
        .call(
            json!({
                "name": "layout-update-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Own the primary slot",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Own the right slot",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create layout update team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse layout update create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let updated = TeamUpdateTool
        .call(
            json!({
                "name": "layout-update-team",
                "layout": "vertical"
            }),
            &context,
        )
        .await
        .expect("update layout team");
    let updated_text = match updated.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let updated_value: serde_json::Value =
        serde_json::from_str(&updated_text).expect("parse updated layout team");
    let team = &updated_value["status"]["teams"][0];
    assert_eq!(team["layout"]["strategy"].as_str(), Some("vertical"));
    let members = team["members"].as_array().expect("team members");
    let bob = members
        .iter()
        .find(|member| member["name"].as_str() == Some("bob"))
        .expect("bob member");
    assert_eq!(bob["agent"]["layout_slot"].as_str(), Some("bottom"));

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
}

#[tokio::test]
async fn team_run_executes_members_in_parallel_and_aggregates() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("team coordination done").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let created = TeamCreateTool
        .call(
            json!({
                "name": "builders",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Inspect src/lib.rs",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Inspect src/main.rs",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse create team");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let run = TeamRunTool
        .call(
            json!({
                "team_name": "builders",
                "prompt": "Continue and return a concise result.",
                "coordinator_prompt": "Summarize the member outputs."
            }),
            &context,
        )
        .await
        .expect("team run");
    let run_text = match run.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        run_text.contains("\"status\": \"coordinated\""),
        "{run_text}"
    );
    assert!(run_text.contains("\"name\": \"alice\""), "{run_text}");
    assert!(run_text.contains("\"name\": \"bob\""), "{run_text}");
    assert!(run_text.contains("\"coordinator\""), "{run_text}");
    assert!(run_text.contains("team coordination done"), "{run_text}");

    let run_value: serde_json::Value = serde_json::from_str(&run_text).expect("parse team run");
    if let Some(coordinator_session_id) = run_value["coordinator"]["session_id"].as_str() {
        let _ = fs::remove_file(session_file_path(coordinator_session_id));
    }
    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
}

#[tokio::test]
async fn workflow_tool_runs_multiple_steps() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("workflow step done").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let result = WorkflowTool
        .call(
            json!({
                "steps": [
                    { "name": "analyze", "prompt": "Analyze the module." },
                    { "name": "summarize", "prompt": "Summarize the result." }
                ]
            }),
            &context,
        )
        .await
        .expect("workflow");
    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(text.contains("\"status\": \"completed\""), "{text}");
    assert!(text.contains("\"name\": \"analyze\""), "{text}");
    assert!(text.contains("workflow step done"), "{text}");

    let value: serde_json::Value = serde_json::from_str(&text).expect("parse workflow result");
    for step in value["steps"].as_array().expect("workflow steps") {
        if let Some(session_id) = step["result"]["session_id"].as_str() {
            let _ = fs::remove_file(session_file_path(session_id));
        }
    }
}

#[tokio::test]
async fn team_wait_and_stop_supervise_member_lifecycle() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway_with_delay("team background done", 200).await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let created = TeamCreateTool
        .call(
            json!({
                "name": "watchers",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Watch file A",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Watch file B",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse create team");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let waited = TeamWaitTool
        .call(
            json!({
                "name": "watchers",
                "timeout_ms": 5_000,
                "poll_interval_ms": 20
            }),
            &context,
        )
        .await
        .expect("wait team");
    let waited_text = match waited.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        waited_text.contains("\"completed_members\": 2"),
        "{waited_text}"
    );
    assert!(
        waited_text.contains("\"overall_status\": \"completed\""),
        "{waited_text}"
    );

    let second_team = TeamCreateTool
        .call(
            json!({
                "name": "stoppers",
                "members": [
                    {
                        "name": "charlie",
                        "prompt": "Watch file C",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create stop team");
    let second_text = match second_team.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let second_value: serde_json::Value =
        serde_json::from_str(&second_text).expect("parse stop team");
    let stop_session_id = second_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("stop session")
        .to_string();

    let stopped = TeamStopTool
        .call(
            json!({
                "name": "stoppers",
                "reason": "supervisor stop"
            }),
            &context,
        )
        .await
        .expect("stop team");
    let stopped_text = match stopped.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(stopped_text.contains("\"stopped\": true"), "{stopped_text}");
    assert!(
        stopped_text.contains("\"cancelled_members\": 1"),
        "{stopped_text}"
    );

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(session_file_path(&stop_session_id));
}

#[tokio::test]
async fn team_update_rotates_members_and_stops_removed_background_agents() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway_with_delay("team update done", 750).await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let created = TeamCreateTool
        .call(
            json!({
                "name": "rotating",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Watch file A",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse create team");
    let alice_session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("alice session id")
        .to_string();

    let updated = TeamUpdateTool
        .call(
            json!({
                "name": "rotating",
                "add_members": [
                    {
                        "name": "bob",
                        "prompt": "Watch file B",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ],
                "remove_members": ["alice"],
                "stop_removed": true,
                "reason": "rotate worker"
            }),
            &context,
        )
        .await
        .expect("update team");
    let updated_text = match updated.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(updated_text.contains("\"name\": \"bob\""), "{updated_text}");
    assert!(
        updated_text.contains("\"name\": \"alice\""),
        "{updated_text}"
    );
    assert!(updated_text.contains("\"stopped\": true"), "{updated_text}");
    assert!(
        updated_text.contains("\"status\": \"cancelled\""),
        "{updated_text}"
    );

    let updated_value: serde_json::Value =
        serde_json::from_str(&updated_text).expect("parse updated team");
    let bob_session_id = updated_value["added_members"][0]["agent"]["session_id"]
        .as_str()
        .expect("bob session id")
        .to_string();
    let team_members = updated_value["status"]["teams"][0]["members"]
        .as_array()
        .expect("team members");
    assert_eq!(team_members.len(), 1, "{updated_text}");
    assert_eq!(team_members[0]["name"].as_str(), Some("bob"));

    let _ = fs::remove_file(session_file_path(&alice_session_id));
    let _ = fs::remove_file(session_file_path(&bob_session_id));
}

#[tokio::test]
async fn team_update_syncs_permission_mode_to_existing_members() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("team permission sync").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let created = TeamCreateTool
        .call(
            json!({
                "name": "permissions",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Review permissions",
                        "run_in_background": false,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse create team");
    let session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("member session id")
        .to_string();

    let updated = TeamUpdateTool
        .call(
            json!({
                "name": "permissions",
                "permission_mode": "accept_edits"
            }),
            &context,
        )
        .await
        .expect("sync team permissions");
    let updated_text = match updated.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        updated_text.contains("\"permission_mode\": \"accept_edits\""),
        "{updated_text}"
    );

    let stored = StoredSession::load(&session_id).expect("load synced session");
    assert_eq!(
        stored.snapshot.permission_mode,
        Some(PermissionMode::AcceptEdits)
    );

    let _ = fs::remove_file(session_file_path(&session_id));
}

#[tokio::test]
async fn workflow_tool_expands_shared_and_previous_step_templates() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_echo_gateway().await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let result = WorkflowTool
        .call(
            json!({
                "shared_context": "repo=hellox",
                "steps": [
                    { "name": "first", "prompt": "alpha {{workflow.shared_context}}" },
                    {
                        "name": "second",
                        "prompt": "carry {{workflow.previous_result}} | {{steps.first.status}}"
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("workflow");
    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(text.contains("\"status\": \"completed\""), "{text}");

    let value: serde_json::Value = serde_json::from_str(&text).expect("parse workflow result");
    let second_result = value["steps"][1]["result"]["result"]
        .as_str()
        .expect("second step result");
    assert!(
        second_result.contains("carry alpha repo=hellox | completed"),
        "{text}"
    );

    for step in value["steps"].as_array().expect("workflow steps") {
        if let Some(session_id) = step["result"]["session_id"].as_str() {
            let _ = fs::remove_file(session_file_path(session_id));
        }
    }
}

#[tokio::test]
async fn workflow_tool_loads_steps_from_project_workflow_script() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_echo_gateway().await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let script_path = workspace
        .root
        .join(".hellox")
        .join("workflows")
        .join("release-review.json");
    fs::create_dir_all(script_path.parent().expect("script dir")).expect("create script dir");
    fs::write(
        &script_path,
        serde_json::to_string_pretty(&json!({
            "shared_context": "repo=hellox",
            "steps": [
                { "name": "first", "prompt": "alpha {{workflow.shared_context}}" },
                {
                    "name": "second",
                    "prompt": "carry {{workflow.previous_result}} | {{steps.first.status}}"
                }
            ]
        }))
        .expect("serialize workflow script"),
    )
    .expect("write workflow script");

    let result = WorkflowTool
        .call(json!({ "script": "release-review" }), &context)
        .await
        .expect("workflow from project script");
    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        text.contains("\"workflow_source\": \".hellox/workflows/release-review.json\""),
        "{text}"
    );

    let value: serde_json::Value = serde_json::from_str(&text).expect("parse workflow result");
    let second_result = value["steps"][1]["result"]["result"]
        .as_str()
        .expect("second step result");
    assert!(
        second_result.contains("carry alpha repo=hellox | completed"),
        "{text}"
    );

    for step in value["steps"].as_array().expect("workflow steps") {
        if let Some(session_id) = step["result"]["session_id"].as_str() {
            let _ = fs::remove_file(session_file_path(session_id));
        }
    }
}

#[tokio::test]
async fn workflow_tool_loads_script_path_and_applies_input_overrides() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_echo_gateway().await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let script_path = workspace.root.join("local-workflow.json");
    fs::write(
        &script_path,
        serde_json::to_string_pretty(&json!({
            "shared_context": "repo=base",
            "continue_on_error": false,
            "steps": [
                { "name": "first", "prompt": "alpha {{workflow.shared_context}}" }
            ]
        }))
        .expect("serialize workflow script"),
    )
    .expect("write workflow script");

    let result = WorkflowTool
        .call(
            json!({
                "script_path": "local-workflow.json",
                "shared_context": "repo=override"
            }),
            &context,
        )
        .await
        .expect("workflow from script_path");
    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        text.contains("\"workflow_source\": \"local-workflow.json\""),
        "{text}"
    );
    assert!(text.contains("alpha repo=override"), "{text}");

    let value: serde_json::Value = serde_json::from_str(&text).expect("parse workflow result");
    for step in value["steps"].as_array().expect("workflow steps") {
        if let Some(session_id) = step["result"]["session_id"].as_str() {
            let _ = fs::remove_file(session_file_path(session_id));
        }
    }
}

#[tokio::test]
async fn workflow_tool_skips_steps_when_conditions_do_not_match() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_echo_gateway().await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let result = WorkflowTool
        .call(
            json!({
                "steps": [
                    { "name": "first", "prompt": "alpha" },
                    {
                        "name": "second",
                        "when": { "previous_result_contains": "alpha" },
                        "prompt": "beta"
                    },
                    {
                        "name": "third",
                        "when": {
                            "all": [
                                { "step_status": { "name": "second", "status": "completed" } },
                                { "previous_result_contains": "missing" }
                            ]
                        },
                        "prompt": "gamma"
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("workflow with skipped step");
    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(text.contains("\"status\": \"completed\""), "{text}");
    assert!(text.contains("\"skipped_steps\": 1"), "{text}");
    assert!(text.contains("\"name\": \"third\""), "{text}");
    assert!(text.contains("\"status\": \"skipped\""), "{text}");

    let value: serde_json::Value = serde_json::from_str(&text).expect("parse workflow result");
    let steps = value["steps"].as_array().expect("workflow steps");
    assert_eq!(steps[2]["status"].as_str(), Some("skipped"));
    assert!(
        steps[2]["reason"]
            .as_str()
            .expect("skip reason")
            .contains("condition not met"),
        "{text}"
    );

    for step in steps {
        if let Some(session_id) = step["result"]["session_id"].as_str() {
            let _ = fs::remove_file(session_file_path(session_id));
        }
    }
}

#[tokio::test]
async fn workflow_tool_branches_on_step_status_and_not_conditions() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_echo_gateway().await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();

    let result = WorkflowTool
        .call(
            json!({
                "steps": [
                    { "name": "first", "prompt": "alpha" },
                    {
                        "name": "second",
                        "when": {
                            "any": [
                                { "step_result_contains": { "name": "first", "text": "alpha" } },
                                { "previous_status": "failed" }
                            ]
                        },
                        "prompt": "beta"
                    },
                    {
                        "name": "third",
                        "when": {
                            "not": {
                                "step_result_contains": { "name": "first", "text": "alpha" }
                            }
                        },
                        "prompt": "gamma"
                    },
                    {
                        "name": "fourth",
                        "when": { "step_status": { "name": "third", "status": "skipped" } },
                        "prompt": "after skipped"
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("workflow branching");
    let text = match result.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(text.contains("\"completed_steps\": 3"), "{text}");
    assert!(text.contains("\"skipped_steps\": 1"), "{text}");
    assert!(text.contains("after skipped"), "{text}");

    let value: serde_json::Value = serde_json::from_str(&text).expect("parse workflow result");
    let steps = value["steps"].as_array().expect("workflow steps");
    assert_eq!(steps[1]["status"].as_str(), Some("completed"));
    assert_eq!(steps[2]["status"].as_str(), Some("skipped"));
    assert_eq!(steps[3]["status"].as_str(), Some("completed"));

    for step in steps {
        if let Some(session_id) = step["result"]["session_id"].as_str() {
            let _ = fs::remove_file(session_file_path(session_id));
        }
    }
}

#[cfg(windows)]
#[tokio::test]
async fn team_follow_up_background_reuses_member_detached_backend() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused detached response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let env_guard = DetachedBackendEnvGuard::new(write_detached_script(
        &workspace.root,
        DetachedScriptMode::CompleteAfterDelay,
    ));

    let created = TeamCreateTool
        .call(
            json!({
                "name": "detached follow-up team",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Initial detached run.",
                        "backend": "detached_process",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create detached follow-up team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse detached team");
    let session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("member session id")
        .to_string();

    wait_for_agent_completion(&context, &session_id)
        .await
        .expect("wait initial detached member");

    let run = TeamRunTool
        .call(
            json!({
                "team_name": "detached follow-up team",
                "prompt": "Run in the background again.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("run detached follow-up team");
    let run_text = match run.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        run_text.contains("\"backend\": \"detached_process\""),
        "{run_text}"
    );
    assert!(run_text.contains("\"status\": \"running\""), "{run_text}");

    wait_for_agent_completion(&context, &session_id)
        .await
        .expect("wait detached team run");

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "detached follow-up team",
                "to": "alice",
                "content": "Resume in the background.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("send detached follow-up message");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        reply_text.contains("\"backend\": \"detached_process\""),
        "{reply_text}"
    );
    assert!(
        reply_text.contains("\"status\": \"running\""),
        "{reply_text}"
    );

    let waited_text = wait_for_agent_completion(&context, &session_id)
        .await
        .expect("wait detached follow-up message");
    assert!(
        waited_text.contains("\"backend\": \"detached_process\""),
        "{waited_text}"
    );
    assert!(
        waited_text.contains("detached backend done"),
        "{waited_text}"
    );

    let _ = fs::remove_file(session_file_path(&session_id));
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn agent_stop_tmux_member_clears_pane_target_in_status_and_team_registry() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-agent-stop-state.json");
    let tmux_log = workspace.root.join("tmux-agent-stop.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-agent-stop-team",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux agent stop team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux agent stop create");
    let session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let stopped = AgentStopTool
        .call(
            json!({
                "session_id": session_id,
                "reason": "stop tmux member"
            }),
            &context,
        )
        .await
        .expect("stop tmux member");
    let stopped_text = match stopped.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(stopped_text.contains("\"stopped\": true"), "{stopped_text}");
    assert!(
        stopped_text.contains("\"backend\": \"tmux_pane\""),
        "{stopped_text}"
    );
    assert!(
        stopped_text.contains("\"pane_target\": null"),
        "{stopped_text}"
    );

    let stopped_runtime = StoredSession::load(&session_id).expect("load stopped tmux member");
    assert!(stopped_runtime
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    let teams_raw = fs::read_to_string(workspace.root.join(".hellox").join("teams.json"))
        .expect("read team registry");
    let teams_value: serde_json::Value =
        serde_json::from_str(&teams_raw).expect("parse team registry");
    let team = teams_value
        .as_array()
        .expect("teams array")
        .iter()
        .find(|team| team["name"].as_str() == Some("tmux-agent-stop-team"))
        .expect("tmux agent stop team");
    assert!(team["members"][0]["pane_target"].is_null(), "{teams_raw}");

    let status = TeamStatusTool
        .call(json!({ "name": "tmux-agent-stop-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"pane_target\": null"),
        "{status_text}"
    );

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[&["new-session"], &["kill-pane", "-t %1"]],
        &log,
    );

    TeamDeleteTool
        .call(json!({ "name": "tmux-agent-stop-team" }), &context)
        .await
        .expect("delete tmux agent stop team");

    let _ = fs::remove_file(session_file_path(&session_id));
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn team_status_reconciles_external_tmux_host_drift_into_registry_and_runtime() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-status-reconcile-state.json");
    let tmux_log = workspace.root.join("tmux-status-reconcile.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-status-reconcile-team",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux status reconcile team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux status reconcile create");
    let session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let raw_state = fs::read_to_string(&tmux_state).expect("read fake tmux state");
    let mut state: serde_json::Value =
        serde_json::from_str(&raw_state).expect("parse fake tmux state");
    state["groups"]["hellox-tmux-status-reconcile-team"] = json!([]);
    state["panes"]["%1"] = serde_json::Value::Null;
    if let Some(panes) = state
        .get_mut("panes")
        .and_then(serde_json::Value::as_object_mut)
    {
        panes.remove("%1");
    }
    fs::write(
        &tmux_state,
        serde_json::to_string_pretty(&state).expect("serialize fake tmux state"),
    )
    .expect("write fake tmux state");

    let status = TeamStatusTool
        .call(json!({ "name": "tmux-status-reconcile-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"pane_target\": null"),
        "{status_text}"
    );
    assert!(
        status_text.contains("\"host_runtime_status\": \"empty\""),
        "{status_text}"
    );

    let teams_raw = fs::read_to_string(workspace.root.join(".hellox").join("teams.json"))
        .expect("read team registry");
    let teams_value: serde_json::Value =
        serde_json::from_str(&teams_raw).expect("parse team registry");
    let team = teams_value
        .as_array()
        .expect("teams array")
        .iter()
        .find(|team| team["name"].as_str() == Some("tmux-status-reconcile-team"))
        .expect("tmux status reconcile team");
    assert!(team["members"][0]["pane_target"].is_null(), "{teams_raw}");

    let stored = StoredSession::load(&session_id).expect("load reconciled session");
    assert!(stored
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    TeamDeleteTool
        .call(json!({ "name": "tmux-status-reconcile-team" }), &context)
        .await
        .expect("delete tmux status reconcile team");

    let _ = fs::remove_file(session_file_path(&session_id));
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn agent_status_reconciles_external_tmux_host_drift_for_team_member_runtime() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace
        .root
        .join("tmux-agent-status-reconcile-state.json");
    let tmux_log = workspace.root.join("tmux-agent-status-reconcile.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-agent-status-reconcile-team",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux agent status reconcile team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux agent status reconcile create");
    let session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let raw_state = fs::read_to_string(&tmux_state).expect("read fake tmux state");
    let mut state: serde_json::Value =
        serde_json::from_str(&raw_state).expect("parse fake tmux state");
    state["groups"]["hellox-tmux-agent-status-reconcile-team"] = json!([]);
    if let Some(panes) = state
        .get_mut("panes")
        .and_then(serde_json::Value::as_object_mut)
    {
        panes.remove("%1");
    }
    fs::write(
        &tmux_state,
        serde_json::to_string_pretty(&state).expect("serialize fake tmux state"),
    )
    .expect("write fake tmux state");

    let status = AgentStatusTool
        .call(json!({ "session_id": session_id.clone() }), &context)
        .await
        .expect("agent status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"pane_target\": null"),
        "{status_text}"
    );

    let teams_raw = fs::read_to_string(workspace.root.join(".hellox").join("teams.json"))
        .expect("read team registry");
    let teams_value: serde_json::Value =
        serde_json::from_str(&teams_raw).expect("parse team registry");
    let team = teams_value
        .as_array()
        .expect("teams array")
        .iter()
        .find(|team| team["name"].as_str() == Some("tmux-agent-status-reconcile-team"))
        .expect("tmux agent status reconcile team");
    assert!(team["members"][0]["pane_target"].is_null(), "{teams_raw}");

    let stored = StoredSession::load(&session_id).expect("load reconciled session");
    assert!(stored
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    TeamDeleteTool
        .call(
            json!({ "name": "tmux-agent-status-reconcile-team" }),
            &context,
        )
        .await
        .expect("delete tmux agent status reconcile team");

    let _ = fs::remove_file(session_file_path(&session_id));
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn agent_list_reconciles_external_tmux_host_drift_for_team_members() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-agent-list-reconcile-state.json");
    let tmux_log = workspace.root.join("tmux-agent-list-reconcile.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-agent-list-reconcile-team",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux agent list reconcile team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux agent list reconcile create");
    let session_id = created_value["members"][0]["agent"]["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let raw_state = fs::read_to_string(&tmux_state).expect("read fake tmux state");
    let mut state: serde_json::Value =
        serde_json::from_str(&raw_state).expect("parse fake tmux state");
    state["groups"]["hellox-tmux-agent-list-reconcile-team"] = json!([]);
    if let Some(panes) = state
        .get_mut("panes")
        .and_then(serde_json::Value::as_object_mut)
    {
        panes.remove("%1");
    }
    fs::write(
        &tmux_state,
        serde_json::to_string_pretty(&state).expect("serialize fake tmux state"),
    )
    .expect("write fake tmux state");

    let listed = AgentListTool
        .call(json!({}), &context)
        .await
        .expect("agent list");
    let listed_text = match listed.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(listed_text.contains(&session_id), "{listed_text}");
    assert!(
        listed_text.contains("\"pane_target\": null"),
        "{listed_text}"
    );

    let teams_raw = fs::read_to_string(workspace.root.join(".hellox").join("teams.json"))
        .expect("read team registry");
    let teams_value: serde_json::Value =
        serde_json::from_str(&teams_raw).expect("parse team registry");
    let team = teams_value
        .as_array()
        .expect("teams array")
        .iter()
        .find(|team| team["name"].as_str() == Some("tmux-agent-list-reconcile-team"))
        .expect("tmux agent list reconcile team");
    assert!(team["members"][0]["pane_target"].is_null(), "{teams_raw}");

    let stored = StoredSession::load(&session_id).expect("load reconciled session");
    assert!(stored
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    TeamDeleteTool
        .call(
            json!({ "name": "tmux-agent-list-reconcile-team" }),
            &context,
        )
        .await
        .expect("delete tmux agent list reconcile team");

    let _ = fs::remove_file(session_file_path(&session_id));
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn team_create_tmux_backend_uses_anchor_targets_for_horizontal_layout() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-state.json");
    let tmux_log = workspace.root.join("tmux.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-layout-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "charlie",
                        "prompt": "Third pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux layout team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux layout create");
    assert_eq!(
        created_value["layout_runtime_sync"]["status"].as_str(),
        Some("applied")
    );
    assert_eq!(
        created_value["layout_runtime_sync"]["preset"].as_str(),
        Some("even-horizontal")
    );
    let layout_runtime = &created_value["status"]["teams"][0]["layout"]["runtime"];
    assert_eq!(layout_runtime["status"].as_str(), Some("sync_capable"));
    assert_eq!(layout_runtime["preset"].as_str(), Some("even-horizontal"));
    assert_eq!(layout_runtime["tmux_members"].as_u64(), Some(3));
    assert_eq!(layout_runtime["live_tmux_members"].as_u64(), Some(3));
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[
            &["new-session"],
            &["select-layout", "even-horizontal"],
            &["split-window", "-t %1", "-h"],
            &["select-layout", "even-horizontal"],
            &["split-window", "-t %2", "-h"],
            &["select-layout", "even-horizontal"],
            &["select-layout", "even-horizontal"],
        ],
        &log,
    );

    TeamStopTool
        .call(
            json!({
                "name": "tmux-layout-team",
                "reason": "cleanup tmux layout team"
            }),
            &context,
        )
        .await
        .expect("stop tmux layout team");
    TeamDeleteTool
        .call(json!({ "name": "tmux-layout-team" }), &context)
        .await
        .expect("delete tmux layout team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn team_create_iterm_backend_uses_anchor_targets_for_horizontal_layout() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused iterm response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let iterm_script = write_fake_iterm_script(&workspace.root);
    let iterm_state = workspace.root.join("iterm-state.json");
    let iterm_log = workspace.root.join("iterm.log");
    let env_guard = ITermBackendEnvGuard::new(iterm_script, iterm_state.clone(), iterm_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "iterm-layout-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "charlie",
                        "prompt": "Third pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create iterm layout team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse iterm layout create");
    let runtime = &created_value["status"]["teams"][0]["layout"]["runtime"];
    assert_eq!(runtime["status"].as_str(), Some("live_panes"));
    assert_eq!(runtime["backend"].as_str(), Some("iterm_pane"));
    assert_eq!(runtime["iterm_members"].as_u64(), Some(3));
    assert_eq!(runtime["live_iterm_members"].as_u64(), Some(3));
    assert_eq!(runtime["pending_iterm_members"].as_u64(), Some(0));

    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let log = fs::read_to_string(&iterm_log).expect("read iterm log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_eq!(commands.len(), 3, "{log}");
    assert!(
        commands[0].contains("create group=hellox-iterm-layout-team"),
        "{log}"
    );
    assert!(commands[0].contains("id=session-1"), "{log}");
    assert!(commands[1].contains("split direction=horizontal"), "{log}");
    assert!(commands[1].contains("anchor=session-1"), "{log}");
    assert!(commands[1].contains("id=session-2"), "{log}");
    assert!(commands[2].contains("split direction=horizontal"), "{log}");
    assert!(commands[2].contains("anchor=session-2"), "{log}");
    assert!(commands[2].contains("id=session-3"), "{log}");

    TeamStopTool
        .call(
            json!({
                "name": "iterm-layout-team",
                "reason": "cleanup iterm layout team"
            }),
            &context,
        )
        .await
        .expect("stop iterm layout team");
    TeamDeleteTool
        .call(json!({ "name": "iterm-layout-team" }), &context)
        .await
        .expect("delete iterm layout team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(iterm_state);
    let _ = fs::remove_file(iterm_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn send_message_iterm_follow_up_reuses_anchor_and_refreshes_team_status() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused iterm response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let iterm_script = write_fake_iterm_script(&workspace.root);
    let iterm_state = workspace.root.join("iterm-followup-state.json");
    let iterm_log = workspace.root.join("iterm-followup.log");
    let env_guard = ITermBackendEnvGuard::new(iterm_script, iterm_state.clone(), iterm_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "iterm-followup-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "charlie",
                        "prompt": "Third pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create iterm follow-up team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse iterm follow-up create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TeamStopTool
        .call(
            json!({
                "name": "iterm-followup-team",
                "targets": ["charlie"],
                "reason": "restart charlie"
            }),
            &context,
        )
        .await
        .expect("stop charlie");

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "iterm-followup-team",
                "to": "charlie",
                "content": "Restart in background.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("restart charlie");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        reply_text.contains("\"backend\": \"iterm_pane\""),
        "{reply_text}"
    );
    assert!(
        reply_text.contains("\"pane_target\": \"session-4\""),
        "{reply_text}"
    );

    let status = TeamStatusTool
        .call(json!({ "name": "iterm-followup-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"pane_target\": \"session-4\""),
        "{status_text}"
    );
    assert!(
        status_text.contains("\"live_iterm_members\": 3"),
        "{status_text}"
    );

    let log = fs::read_to_string(&iterm_log).expect("read iterm log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_eq!(commands.len(), 5, "{log}");
    assert!(commands[3].contains("close target=session-3"), "{log}");
    assert!(commands[4].contains("split direction=horizontal"), "{log}");
    assert!(commands[4].contains("anchor=session-2"), "{log}");
    assert!(commands[4].contains("id=session-4"), "{log}");

    TeamStopTool
        .call(
            json!({
                "name": "iterm-followup-team",
                "reason": "cleanup iterm follow-up team"
            }),
            &context,
        )
        .await
        .expect("stop iterm follow-up team");
    TeamDeleteTool
        .call(json!({ "name": "iterm-followup-team" }), &context)
        .await
        .expect("delete iterm follow-up team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(iterm_state);
    let _ = fs::remove_file(iterm_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn send_message_tmux_follow_up_reuses_anchor_and_refreshes_team_status() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-followup-state.json");
    let tmux_log = workspace.root.join("tmux-followup.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-followup-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "charlie",
                        "prompt": "Third pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux follow-up team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux follow-up create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let stopped = TeamStopTool
        .call(
            json!({
                "name": "tmux-followup-team",
                "targets": ["charlie"],
                "reason": "restart charlie"
            }),
            &context,
        )
        .await
        .expect("stop charlie");
    let stopped_text = match stopped.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(stopped_text.contains("\"stopped\": true"), "{stopped_text}");
    let charlie_session_id = session_ids[2].clone();
    let charlie_runtime = StoredSession::load(&charlie_session_id).expect("load charlie runtime");
    assert!(charlie_runtime
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());
    let teams_raw = fs::read_to_string(workspace.root.join(".hellox").join("teams.json"))
        .expect("read team registry");
    let teams_value: serde_json::Value =
        serde_json::from_str(&teams_raw).expect("parse team registry");
    let team = teams_value
        .as_array()
        .expect("teams array")
        .iter()
        .find(|team| team["name"].as_str() == Some("tmux-followup-team"))
        .expect("tmux follow-up team");
    let charlie_registry = team["members"]
        .as_array()
        .expect("registry members")
        .iter()
        .find(|member| member["name"].as_str() == Some("charlie"))
        .expect("charlie registry member");
    assert!(charlie_registry["pane_target"].is_null(), "{teams_raw}");

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "tmux-followup-team",
                "to": "charlie",
                "content": "Restart in background.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("restart charlie");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        reply_text.contains("\"backend\": \"tmux_pane\""),
        "{reply_text}"
    );
    assert!(
        reply_text.contains("\"pane_target\": \"%4\""),
        "{reply_text}"
    );

    let status = TeamStatusTool
        .call(json!({ "name": "tmux-followup-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"pane_target\": \"%4\""),
        "{status_text}"
    );

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[
            &["split-window", "-t %2"],
            &["select-layout", "even-horizontal"],
            &["kill-pane", "-t %3"],
            &["split-window", "-t %2"],
            &["select-layout", "even-horizontal"],
        ],
        &log,
    );

    TeamStopTool
        .call(
            json!({
                "name": "tmux-followup-team",
                "reason": "cleanup tmux follow-up team"
            }),
            &context,
        )
        .await
        .expect("stop tmux follow-up team");
    TeamDeleteTool
        .call(json!({ "name": "tmux-followup-team" }), &context)
        .await
        .expect("delete tmux follow-up team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn team_update_tmux_layout_reapplies_layout_preset() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-layout-update-state.json");
    let tmux_log = workspace.root.join("tmux-layout-update.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-layout-update-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux layout update team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux layout update create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let updated = TeamUpdateTool
        .call(
            json!({
                "name": "tmux-layout-update-team",
                "layout": "vertical"
            }),
            &context,
        )
        .await
        .expect("update tmux layout team");
    let updated_text = match updated.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        updated_text.contains("\"status\": \"applied\""),
        "{updated_text}"
    );
    assert!(
        updated_text.contains("\"preset\": \"even-vertical\""),
        "{updated_text}"
    );
    let updated_value: serde_json::Value =
        serde_json::from_str(&updated_text).expect("parse tmux layout update");
    let members = updated_value["status"]["teams"][0]["members"]
        .as_array()
        .expect("team members");
    let bob = members
        .iter()
        .find(|member| member["name"].as_str() == Some("bob"))
        .expect("bob member");
    assert_eq!(bob["agent"]["layout_slot"].as_str(), Some("bottom"));

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[
            &["new-session"],
            &["select-layout", "even-horizontal"],
            &["split-window"],
            &["select-layout", "even-horizontal"],
            &["select-layout", "even-horizontal"],
            &["select-layout", "even-vertical"],
        ],
        &log,
    );

    TeamStopTool
        .call(
            json!({
                "name": "tmux-layout-update-team",
                "reason": "cleanup tmux layout update team"
            }),
            &context,
        )
        .await
        .expect("stop tmux layout update team");
    TeamDeleteTool
        .call(json!({ "name": "tmux-layout-update-team" }), &context)
        .await
        .expect("delete tmux layout update team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn team_update_force_layout_sync_reapplies_current_tmux_preset() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-force-layout-sync-state.json");
    let tmux_log = workspace.root.join("tmux-force-layout-sync.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-force-layout-sync-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux force layout sync team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux force layout sync create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let updated = TeamUpdateTool
        .call(
            json!({
                "name": "tmux-force-layout-sync-team",
                "force_layout_sync": true
            }),
            &context,
        )
        .await
        .expect("force layout sync");
    let updated_text = match updated.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let updated_value: serde_json::Value =
        serde_json::from_str(&updated_text).expect("parse force layout sync");
    assert_eq!(updated_value["force_layout_sync"].as_bool(), Some(true));
    assert_eq!(
        updated_value["layout_runtime_sync"]["status"].as_str(),
        Some("applied")
    );
    assert_eq!(
        updated_value["layout_runtime_sync"]["preset"].as_str(),
        Some("even-horizontal")
    );
    let runtime = &updated_value["status"]["teams"][0]["layout"]["runtime"];
    assert_eq!(runtime["status"].as_str(), Some("sync_capable"));
    assert_eq!(runtime["pending_tmux_members"].as_u64(), Some(0));
    assert_eq!(
        runtime["live_tmux_member_names"]
            .as_array()
            .map(|items| items.len()),
        Some(2)
    );

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[
            &["new-session"],
            &["select-layout", "even-horizontal"],
            &["split-window"],
            &["select-layout", "even-horizontal"],
            &["select-layout", "even-horizontal"],
            &["select-layout", "even-horizontal"],
        ],
        &log,
    );

    TeamStopTool
        .call(
            json!({
                "name": "tmux-force-layout-sync-team",
                "reason": "cleanup tmux force layout sync team"
            }),
            &context,
        )
        .await
        .expect("stop tmux force layout sync team");
    TeamDeleteTool
        .call(json!({ "name": "tmux-force-layout-sync-team" }), &context)
        .await
        .expect("delete tmux force layout sync team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn team_update_force_layout_sync_skips_when_tmux_host_has_no_live_panes() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace
        .root
        .join("tmux-force-layout-sync-empty-state.json");
    let tmux_log = workspace.root.join("tmux-force-layout-sync-empty.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-force-layout-sync-empty-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux force layout sync empty team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux force layout sync empty create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TeamStopTool
        .call(
            json!({
                "name": "tmux-force-layout-sync-empty-team",
                "reason": "simulate host panes gone"
            }),
            &context,
        )
        .await
        .expect("stop tmux force layout sync empty team");

    let updated = TeamUpdateTool
        .call(
            json!({
                "name": "tmux-force-layout-sync-empty-team",
                "force_layout_sync": true
            }),
            &context,
        )
        .await
        .expect("force layout sync empty host");
    let updated_text = match updated.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let updated_value: serde_json::Value =
        serde_json::from_str(&updated_text).expect("parse force layout sync empty host");
    assert_eq!(updated_value["force_layout_sync"].as_bool(), Some(true));
    assert_eq!(
        updated_value["layout_runtime_sync"]["status"].as_str(),
        Some("skipped")
    );
    assert_eq!(
        updated_value["layout_runtime_sync"]["reason"].as_str(),
        Some("no_live_panes")
    );
    let runtime = &updated_value["status"]["teams"][0]["layout"]["runtime"];
    assert_eq!(runtime["status"].as_str(), Some("pending_live_panes"));
    assert_eq!(runtime["live_tmux_group_panes"].as_u64(), Some(0));
    assert_eq!(runtime["host_runtime_status"].as_str(), Some("empty"));

    TeamDeleteTool
        .call(
            json!({ "name": "tmux-force-layout-sync-empty-team" }),
            &context,
        )
        .await
        .expect("delete tmux force layout sync empty team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn send_message_tmux_follow_up_recovers_when_primary_pane_was_stopped() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-primary-restart-state.json");
    let tmux_log = workspace.root.join("tmux-primary-restart.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-primary-restart-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux primary restart team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux primary restart create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TeamStopTool
        .call(
            json!({
                "name": "tmux-primary-restart-team",
                "targets": ["alice"],
                "reason": "restart primary"
            }),
            &context,
        )
        .await
        .expect("stop alice");

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "tmux-primary-restart-team",
                "to": "alice",
                "content": "Restart primary in background.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("restart alice");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        reply_text.contains("\"backend\": \"tmux_pane\""),
        "{reply_text}"
    );
    assert!(
        reply_text.contains("\"pane_target\": \"%3\""),
        "{reply_text}"
    );

    let status = TeamStatusTool
        .call(json!({ "name": "tmux-primary-restart-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"pane_target\": \"%3\""),
        "{status_text}"
    );

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[
            &["new-session"],
            &["select-layout", "even-horizontal"],
            &["split-window"],
            &["select-layout", "even-horizontal"],
            &["kill-pane", "-t %1"],
            &["new-session"],
            &["split-window", "-t %2"],
            &["select-layout", "even-horizontal"],
        ],
        &log,
    );

    TeamStopTool
        .call(
            json!({
                "name": "tmux-primary-restart-team",
                "reason": "cleanup tmux primary restart team"
            }),
            &context,
        )
        .await
        .expect("stop tmux primary restart team");
    TeamDeleteTool
        .call(json!({ "name": "tmux-primary-restart-team" }), &context)
        .await
        .expect("delete tmux primary restart team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn send_message_tmux_follow_up_recovers_when_anchor_pane_is_missing() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-anchor-restart-state.json");
    let tmux_log = workspace.root.join("tmux-anchor-restart.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-anchor-restart-team",
                "layout": "horizontal",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "charlie",
                        "prompt": "Third pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux anchor restart team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux anchor restart create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TeamStopTool
        .call(
            json!({
                "name": "tmux-anchor-restart-team",
                "targets": ["bob", "charlie"],
                "reason": "restart trailing panes"
            }),
            &context,
        )
        .await
        .expect("stop bob and charlie");

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "tmux-anchor-restart-team",
                "to": "charlie",
                "content": "Restart third pane in background.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("restart charlie");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        reply_text.contains("\"backend\": \"tmux_pane\""),
        "{reply_text}"
    );
    assert!(
        reply_text.contains("\"pane_target\": \"%4\""),
        "{reply_text}"
    );

    let status = TeamStatusTool
        .call(json!({ "name": "tmux-anchor-restart-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        status_text.contains("\"pane_target\": \"%4\""),
        "{status_text}"
    );

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[
            &["select-layout", "even-horizontal"],
            &["kill-pane", "-t %2"],
            &["kill-pane", "-t %3"],
            &["split-window", "-t %1"],
            &["select-layout", "even-horizontal"],
        ],
        &log,
    );

    TeamStopTool
        .call(
            json!({
                "name": "tmux-anchor-restart-team",
                "reason": "cleanup tmux anchor restart team"
            }),
            &context,
        )
        .await
        .expect("stop tmux anchor restart team");
    TeamDeleteTool
        .call(json!({ "name": "tmux-anchor-restart-team" }), &context)
        .await
        .expect("delete tmux anchor restart team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn send_message_tmux_follow_up_reconciles_stale_primary_pane_target_before_anchor_selection()
{
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused tmux response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let tmux_script = write_fake_tmux_script(&workspace.root);
    let tmux_state = workspace.root.join("tmux-reconcile-state.json");
    let tmux_log = workspace.root.join("tmux-reconcile.log");
    let env_guard = TmuxBackendEnvGuard::new(tmux_script, tmux_state.clone(), tmux_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "tmux-reconcile-team",
                "layout": "fanout",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "charlie",
                        "prompt": "Third pane",
                        "backend": "tmux",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create tmux reconcile team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse tmux reconcile create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TeamStopTool
        .call(
            json!({
                "name": "tmux-reconcile-team",
                "targets": ["alice", "charlie"],
                "reason": "simulate stale primary pane target"
            }),
            &context,
        )
        .await
        .expect("stop alice and charlie");

    let alice_runtime_before =
        StoredSession::load(&session_ids[0]).expect("load alice session before reconcile");
    assert!(alice_runtime_before
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "tmux-reconcile-team",
                "to": "charlie",
                "content": "Refresh third pane in background.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("restart charlie");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        reply_text.contains("\"pane_target\": \"%4\""),
        "{reply_text}"
    );

    let teams_raw = fs::read_to_string(workspace.root.join(".hellox").join("teams.json"))
        .expect("read team registry");
    let teams_value: serde_json::Value =
        serde_json::from_str(&teams_raw).expect("parse team registry");
    let team = teams_value
        .as_array()
        .expect("teams array")
        .iter()
        .find(|team| team["name"].as_str() == Some("tmux-reconcile-team"))
        .expect("tmux reconcile team");
    let alice_registry = team["members"]
        .as_array()
        .expect("registry members")
        .iter()
        .find(|member| member["name"].as_str() == Some("alice"))
        .expect("alice registry member");
    assert!(alice_registry["pane_target"].is_null(), "{teams_raw}");

    let alice_runtime_after =
        StoredSession::load(&session_ids[0]).expect("load alice session after reconcile");
    assert!(alice_runtime_after
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    let status = TeamStatusTool
        .call(json!({ "name": "tmux-reconcile-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let status_value: serde_json::Value =
        serde_json::from_str(&status_text).expect("parse tmux reconcile status");
    let status_team = &status_value["teams"][0];
    let alice_layout_member = status_team["layout"]["members"]
        .as_array()
        .expect("layout members")
        .iter()
        .find(|member| member["name"].as_str() == Some("alice"))
        .expect("alice layout member");
    assert!(
        alice_layout_member["pane_target"].is_null(),
        "{status_text}"
    );
    let alice_agent = status_team["members"]
        .as_array()
        .expect("status members")
        .iter()
        .find(|member| member["name"].as_str() == Some("alice"))
        .expect("alice member status");
    assert!(
        alice_agent["agent"]["pane_target"].is_null(),
        "{status_text}"
    );

    let log = fs::read_to_string(&tmux_log).expect("read tmux log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_command_sequence(
        &commands,
        &[
            &["new-session"],
            &["select-layout", "main-vertical"],
            &["split-window", "-t %1"],
            &["select-layout", "main-vertical"],
            &["split-window", "-t %1"],
            &["select-layout", "main-vertical"],
            &["kill-pane", "-t %1"],
            &["kill-pane", "-t %3"],
            &["split-window", "-t hellox-tmux-reconcile-team:0"],
            &["select-layout", "main-vertical"],
        ],
        &log,
    );

    TeamStopTool
        .call(
            json!({
                "name": "tmux-reconcile-team",
                "reason": "cleanup tmux reconcile team"
            }),
            &context,
        )
        .await
        .expect("stop tmux reconcile team");
    TeamDeleteTool
        .call(json!({ "name": "tmux-reconcile-team" }), &context)
        .await
        .expect("delete tmux reconcile team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(tmux_state);
    let _ = fs::remove_file(tmux_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn send_message_iterm_follow_up_reconciles_stale_primary_pane_target_metadata() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused iterm response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let iterm_script = write_fake_iterm_script(&workspace.root);
    let iterm_state = workspace.root.join("iterm-reconcile-state.json");
    let iterm_log = workspace.root.join("iterm-reconcile.log");
    let env_guard = ITermBackendEnvGuard::new(iterm_script, iterm_state.clone(), iterm_log.clone());

    let created = TeamCreateTool
        .call(
            json!({
                "name": "iterm-reconcile-team",
                "layout": "fanout",
                "members": [
                    {
                        "name": "alice",
                        "prompt": "Primary pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "bob",
                        "prompt": "Second pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    },
                    {
                        "name": "charlie",
                        "prompt": "Third pane",
                        "backend": "iterm",
                        "run_in_background": true,
                        "session_id": format!("team-{}", unique_suffix())
                    }
                ]
            }),
            &context,
        )
        .await
        .expect("create iterm reconcile team");
    let created_text = match created.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let created_value: serde_json::Value =
        serde_json::from_str(&created_text).expect("parse iterm reconcile create");
    let session_ids = created_value["members"]
        .as_array()
        .expect("members")
        .iter()
        .filter_map(|member| member["agent"]["session_id"].as_str())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TeamStopTool
        .call(
            json!({
                "name": "iterm-reconcile-team",
                "targets": ["alice", "charlie"],
                "reason": "simulate stale primary pane target"
            }),
            &context,
        )
        .await
        .expect("stop alice and charlie");

    let alice_runtime_before =
        StoredSession::load(&session_ids[0]).expect("load alice session before reconcile");
    assert!(alice_runtime_before
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    let reply = SendMessageTool
        .call(
            json!({
                "team_name": "iterm-reconcile-team",
                "to": "charlie",
                "content": "Refresh third pane in background.",
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("restart charlie");
    let reply_text = match reply.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        reply_text.contains("\"pane_target\": \"session-4\""),
        "{reply_text}"
    );

    let teams_raw = fs::read_to_string(workspace.root.join(".hellox").join("teams.json"))
        .expect("read team registry");
    let teams_value: serde_json::Value =
        serde_json::from_str(&teams_raw).expect("parse team registry");
    let team = teams_value
        .as_array()
        .expect("teams array")
        .iter()
        .find(|team| team["name"].as_str() == Some("iterm-reconcile-team"))
        .expect("iterm reconcile team");
    let alice_registry = team["members"]
        .as_array()
        .expect("registry members")
        .iter()
        .find(|member| member["name"].as_str() == Some("alice"))
        .expect("alice registry member");
    assert!(alice_registry["pane_target"].is_null(), "{teams_raw}");

    let alice_runtime_after =
        StoredSession::load(&session_ids[0]).expect("load alice session after reconcile");
    assert!(alice_runtime_after
        .snapshot
        .agent_runtime
        .as_ref()
        .and_then(|runtime| runtime.pane_target.as_deref())
        .is_none());

    let status = TeamStatusTool
        .call(json!({ "name": "iterm-reconcile-team" }), &context)
        .await
        .expect("team status");
    let status_text = match status.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let status_value: serde_json::Value =
        serde_json::from_str(&status_text).expect("parse iterm reconcile status");
    let status_team = &status_value["teams"][0];
    let alice_layout_member = status_team["layout"]["members"]
        .as_array()
        .expect("layout members")
        .iter()
        .find(|member| member["name"].as_str() == Some("alice"))
        .expect("alice layout member");
    assert!(
        alice_layout_member["pane_target"].is_null(),
        "{status_text}"
    );
    let alice_agent = status_team["members"]
        .as_array()
        .expect("status members")
        .iter()
        .find(|member| member["name"].as_str() == Some("alice"))
        .expect("alice member status");
    assert!(
        alice_agent["agent"]["pane_target"].is_null(),
        "{status_text}"
    );

    let log = fs::read_to_string(&iterm_log).expect("read iterm log");
    let commands = log.lines().collect::<Vec<_>>();
    assert_eq!(commands.len(), 6, "{log}");
    assert!(commands[3].contains("close target=session-1"), "{log}");
    assert!(commands[4].contains("close target=session-3"), "{log}");
    assert!(commands[5].contains("split direction=vertical"), "{log}");
    assert!(commands[5].contains("anchor=session-2"), "{log}");
    assert!(commands[5].contains("id=session-4"), "{log}");

    TeamStopTool
        .call(
            json!({
                "name": "iterm-reconcile-team",
                "reason": "cleanup iterm reconcile team"
            }),
            &context,
        )
        .await
        .expect("stop iterm reconcile team");
    TeamDeleteTool
        .call(json!({ "name": "iterm-reconcile-team" }), &context)
        .await
        .expect("delete iterm reconcile team");

    for session_id in session_ids {
        let _ = fs::remove_file(session_file_path(&session_id));
    }
    let _ = fs::remove_file(iterm_state);
    let _ = fs::remove_file(iterm_log);
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn detached_process_backend_completes_via_persisted_runtime() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused detached response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let env_guard = DetachedBackendEnvGuard::new(write_detached_script(
        &workspace.root,
        DetachedScriptMode::CompleteAfterDelay,
    ));

    let started = AgentTool
        .call(
            json!({
                "prompt": "Run detached backend.",
                "backend": "detached_process",
                "session_id": format!("detached-{}", unique_suffix()),
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("start detached agent");
    let started_text = match started.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(
        started_text.contains("\"backend\": \"detached_process\""),
        "{started_text}"
    );
    let started_value: serde_json::Value =
        serde_json::from_str(&started_text).expect("parse detached start");
    let session_id = started_value["session_id"]
        .as_str()
        .expect("session id")
        .to_string();
    assert!(started_value["pid"].as_u64().is_some(), "{started_text}");

    let waited_text = wait_for_agent_completion(&context, &session_id)
        .await
        .expect("wait detached backend");
    assert!(
        waited_text.contains("\"status\": \"completed\""),
        "{waited_text}"
    );
    assert!(
        waited_text.contains("detached backend done"),
        "{waited_text}"
    );
    assert!(
        waited_text.contains("\"backend\": \"detached_process\""),
        "{waited_text}"
    );

    let _ = fs::remove_file(session_file_path(&session_id));
    drop(env_guard);
}

#[cfg(windows)]
#[tokio::test]
async fn detached_process_backend_can_be_stopped_by_pid() {
    let _guard = acquire_agent_test_guard().await;
    reset_background_state().expect("reset background state");
    let workspace = TestWorkspace::new();
    let base_url = spawn_mock_gateway("unused detached response").await;
    write_config(&workspace.root, &base_url);
    let context = workspace.context();
    let env_guard = DetachedBackendEnvGuard::new(write_detached_script(
        &workspace.root,
        DetachedScriptMode::SleepForever,
    ));

    let started = AgentTool
        .call(
            json!({
                "prompt": "Run detached backend forever.",
                "backend": "pane",
                "session_id": format!("detached-{}", unique_suffix()),
                "run_in_background": true
            }),
            &context,
        )
        .await
        .expect("start detached agent");
    let started_text = match started.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    let started_value: serde_json::Value =
        serde_json::from_str(&started_text).expect("parse detached start");
    let session_id = started_value["session_id"]
        .as_str()
        .expect("session id")
        .to_string();

    let stopped = AgentStopTool
        .call(
            json!({
                "session_id": session_id.clone(),
                "reason": "stop detached backend"
            }),
            &context,
        )
        .await
        .expect("stop detached agent");
    let stopped_text = match stopped.content {
        hellox_gateway_api::ToolResultContent::Text(text) => text,
        _ => panic!("expected text result"),
    };
    assert!(stopped_text.contains("\"stopped\": true"), "{stopped_text}");
    assert!(
        stopped_text.contains("\"status\": \"cancelled\""),
        "{stopped_text}"
    );
    assert!(
        stopped_text.contains("\"backend\": \"detached_process\""),
        "{stopped_text}"
    );

    let _ = fs::remove_file(session_file_path(&session_id));
    drop(env_guard);
}

async fn spawn_mock_gateway(reply: &str) -> String {
    spawn_mock_gateway_with_delay(reply, 0).await
}

async fn spawn_mock_gateway_with_delay(reply: &str, delay_ms: u64) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test gateway");
    let address = listener.local_addr().expect("local addr");
    let body = serde_json::to_string(&AnthropicCompatResponse::new(
        "test-model",
        vec![ContentBlock::Text {
            text: reply.to_string(),
        }],
        Usage::default(),
    ))
    .expect("serialize response");

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(value) => value,
                Err(_) => break,
            };
            let response_body = body.clone();
            tokio::spawn(async move {
                let _ = read_http_body(&mut socket).await;
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });

    format!("http://{}", address)
}

async fn spawn_echo_gateway() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test gateway");
    let address = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(value) => value,
                Err(_) => break,
            };
            tokio::spawn(async move {
                let body = match read_http_body(&mut socket).await {
                    Ok(value) => value,
                    Err(_) => return,
                };
                let request = serde_json::from_str::<AnthropicCompatRequest>(&body)
                    .expect("parse anthropic request");
                let prompt = request
                    .messages
                    .last()
                    .map(|message| extract_text(&message.content))
                    .unwrap_or_default();
                let response_body = serde_json::to_string(&AnthropicCompatResponse::new(
                    "test-model",
                    vec![ContentBlock::Text { text: prompt }],
                    Usage::default(),
                ))
                .expect("serialize response");
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });

    format!("http://{}", address)
}

async fn read_http_body(socket: &mut tokio::net::TcpStream) -> std::io::Result<String> {
    let mut buffer = Vec::with_capacity(16_384);
    let mut chunk = vec![0_u8; 4_096];
    let mut header_end = None;
    let mut content_length = 0_usize;

    loop {
        let bytes_read = socket.read(&mut chunk).await?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);

        if header_end.is_none() {
            if let Some(index) = find_bytes(&buffer, b"\r\n\r\n") {
                header_end = Some(index + 4);
                let headers = String::from_utf8_lossy(&buffer[..index + 4]);
                content_length = parse_content_length(&headers);
            }
        }

        if let Some(index) = header_end {
            if buffer.len() >= index + content_length {
                let body =
                    String::from_utf8_lossy(&buffer[index..index + content_length]).into_owned();
                return Ok(body);
            }
        }
    }

    Ok("{}".to_string())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn write_config(root: &PathBuf, base_url: &str) {
    let config = format!(
        "[gateway]\nlisten = \"{}\"\n\n[session]\npersist = true\nmodel = \"mock-model\"\n",
        base_url
    );
    let config_path = root.join(".hellox").join("config.toml");
    fs::create_dir_all(config_path.parent().expect("config dir")).expect("create config dir");
    fs::write(config_path, config).expect("write config");
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos()
}

async fn wait_for_agent_completion(
    context: &ToolExecutionContext,
    session_id: &str,
) -> anyhow::Result<String> {
    let deadline = Instant::now() + Duration::from_secs(30);

    loop {
        if Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "agent session `{session_id}` did not complete in time"
            ));
        }

        match AgentWaitTool
            .call(
                json!({
                    "session_id": session_id,
                    "timeout_ms": 10_000,
                    "poll_interval_ms": 20
                }),
                context,
            )
            .await
        {
            Ok(result) => {
                let text = match result.content {
                    hellox_gateway_api::ToolResultContent::Text(text) => text,
                    _ => panic!("expected text result"),
                };
                return Ok(text);
            }
            Err(error)
                if error.to_string().contains("was not found")
                    || error
                        .to_string()
                        .contains("timed out waiting for background agent") =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(windows)]
enum DetachedScriptMode {
    CompleteAfterDelay,
    SleepForever,
}

#[cfg(windows)]
struct DetachedBackendEnvGuard {
    previous: Option<String>,
}

#[cfg(windows)]
impl DetachedBackendEnvGuard {
    fn new(script_path: PathBuf) -> Self {
        let previous = env::var("HELLOX_AGENT_BACKEND_COMMAND").ok();
        let command = serde_json::to_string(&vec![
            "pwsh".to_string(),
            "-NoProfile".to_string(),
            "-File".to_string(),
            script_path.display().to_string(),
        ])
        .expect("serialize backend command");
        env::set_var("HELLOX_AGENT_BACKEND_COMMAND", command);
        Self { previous }
    }
}

#[cfg(windows)]
impl Drop for DetachedBackendEnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            env::set_var("HELLOX_AGENT_BACKEND_COMMAND", previous);
        } else {
            env::remove_var("HELLOX_AGENT_BACKEND_COMMAND");
        }
    }
}

#[cfg(windows)]
struct TmuxBackendEnvGuard {
    previous_command: Option<String>,
    previous_state: Option<String>,
    previous_log: Option<String>,
}

#[cfg(windows)]
impl TmuxBackendEnvGuard {
    fn new(script_path: PathBuf, state_path: PathBuf, log_path: PathBuf) -> Self {
        let previous_command = env::var("HELLOX_AGENT_TMUX_COMMAND").ok();
        let previous_state = env::var("HELLOX_FAKE_TMUX_STATE").ok();
        let previous_log = env::var("HELLOX_FAKE_TMUX_LOG").ok();
        let command = serde_json::to_string(&vec![
            "pwsh".to_string(),
            "-NoProfile".to_string(),
            "-File".to_string(),
            script_path.display().to_string(),
        ])
        .expect("serialize tmux command");
        env::set_var("HELLOX_AGENT_TMUX_COMMAND", command);
        env::set_var("HELLOX_FAKE_TMUX_STATE", state_path.display().to_string());
        env::set_var("HELLOX_FAKE_TMUX_LOG", log_path.display().to_string());
        Self {
            previous_command,
            previous_state,
            previous_log,
        }
    }
}

#[cfg(windows)]
impl Drop for TmuxBackendEnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous_command.as_ref() {
            env::set_var("HELLOX_AGENT_TMUX_COMMAND", previous);
        } else {
            env::remove_var("HELLOX_AGENT_TMUX_COMMAND");
        }
        if let Some(previous) = self.previous_state.as_ref() {
            env::set_var("HELLOX_FAKE_TMUX_STATE", previous);
        } else {
            env::remove_var("HELLOX_FAKE_TMUX_STATE");
        }
        if let Some(previous) = self.previous_log.as_ref() {
            env::set_var("HELLOX_FAKE_TMUX_LOG", previous);
        } else {
            env::remove_var("HELLOX_FAKE_TMUX_LOG");
        }
    }
}

#[cfg(windows)]
struct ITermBackendEnvGuard {
    previous_command: Option<String>,
    previous_state: Option<String>,
    previous_log: Option<String>,
}

#[cfg(windows)]
impl ITermBackendEnvGuard {
    fn new(script_path: PathBuf, state_path: PathBuf, log_path: PathBuf) -> Self {
        let previous_command = env::var("HELLOX_AGENT_ITERM_COMMAND").ok();
        let previous_state = env::var("HELLOX_FAKE_ITERM_STATE").ok();
        let previous_log = env::var("HELLOX_FAKE_ITERM_LOG").ok();
        let command = serde_json::to_string(&vec![
            "pwsh".to_string(),
            "-NoProfile".to_string(),
            "-File".to_string(),
            script_path.display().to_string(),
        ])
        .expect("serialize iterm command");
        env::set_var("HELLOX_AGENT_ITERM_COMMAND", command);
        env::set_var("HELLOX_FAKE_ITERM_STATE", state_path.display().to_string());
        env::set_var("HELLOX_FAKE_ITERM_LOG", log_path.display().to_string());
        Self {
            previous_command,
            previous_state,
            previous_log,
        }
    }
}

#[cfg(windows)]
impl Drop for ITermBackendEnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous_command.as_ref() {
            env::set_var("HELLOX_AGENT_ITERM_COMMAND", previous);
        } else {
            env::remove_var("HELLOX_AGENT_ITERM_COMMAND");
        }
        if let Some(previous) = self.previous_state.as_ref() {
            env::set_var("HELLOX_FAKE_ITERM_STATE", previous);
        } else {
            env::remove_var("HELLOX_FAKE_ITERM_STATE");
        }
        if let Some(previous) = self.previous_log.as_ref() {
            env::set_var("HELLOX_FAKE_ITERM_LOG", previous);
        } else {
            env::remove_var("HELLOX_FAKE_ITERM_LOG");
        }
    }
}

#[cfg(windows)]
fn write_detached_script(root: &PathBuf, mode: DetachedScriptMode) -> PathBuf {
    let path = root.join(format!("detached-{}.ps1", unique_suffix()));
    let body = match mode {
        DetachedScriptMode::CompleteAfterDelay => r#"
$jobPath = $args[$args.Length - 1]
$spec = Get-Content $jobPath -Raw | ConvertFrom-Json
Start-Sleep -Milliseconds 250
$session = Get-Content $spec.session_path -Raw | ConvertFrom-Json
$runtime = $session.agent_runtime
$runtime.status = "completed"
$runtime.finished_at = [int][DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$runtime.iterations = 1
$runtime.result = "detached backend done"
$runtime.error = $null
$session.agent_runtime = $runtime
$session.updated_at = $runtime.finished_at
$session | ConvertTo-Json -Depth 16 | Set-Content -Encoding utf8 $spec.session_path
Remove-Item $jobPath -ErrorAction SilentlyContinue
"#
        .to_string(),
        DetachedScriptMode::SleepForever => r#"
$jobPath = $args[$args.Length - 1]
while ($true) {
  Start-Sleep -Seconds 1
}
"#
        .to_string(),
    };
    fs::write(&path, body).expect("write detached backend script");
    path
}

#[cfg(windows)]
fn write_fake_tmux_script(root: &PathBuf) -> PathBuf {
    let path = root.join(format!("fake-tmux-{}.ps1", unique_suffix()));
    let body = r#"
$StatePath = $env:HELLOX_FAKE_TMUX_STATE
$LogPath = $env:HELLOX_FAKE_TMUX_LOG
$CmdArgs = $args

$state = @{ next = 1; groups = @{}; panes = @{} }
if (Test-Path $StatePath) {
  $raw = Get-Content $StatePath -Raw
  if ($raw.Trim().Length -gt 0) {
    $json = $raw | ConvertFrom-Json -AsHashtable
    if ($json.ContainsKey("next")) {
      $state.next = [int]$json["next"]
    }
    if ($json.ContainsKey("groups")) {
      $state.groups = @{}
      foreach ($entry in $json["groups"].GetEnumerator()) {
        $values = @()
        foreach ($pane in $entry.Value) {
          $values += [string]$pane
        }
        $state.groups[$entry.Key] = $values
      }
    }
    if ($json.ContainsKey("panes")) {
      $state.panes = @{}
      foreach ($entry in $json["panes"].GetEnumerator()) {
        $state.panes[$entry.Key] = [string]$entry.Value
      }
    }
  }
}

Add-Content -Encoding utf8 $LogPath ($CmdArgs -join " ")

function Save-State {
  $state | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $StatePath
}

function Arg-After {
  param([string]$Name)
  for ($index = 0; $index -lt $CmdArgs.Length - 1; $index++) {
    if ($CmdArgs[$index] -eq $Name) {
      return [string]$CmdArgs[$index + 1]
    }
  }
  return $null
}

switch ($CmdArgs[0]) {
  "new-session" {
    $group = Arg-After "-s"
    if ($null -ne $group -and $state.groups.ContainsKey($group) -and $state.groups[$group].Count -gt 0) {
      Write-Error "duplicate session: $group"
      exit 1
    }
    $id = "%$($state.next)"
    $state.next = $state.next + 1
    if ($null -eq $group) {
      $group = "default"
    }
    $state.groups[$group] = @($id)
    $state.panes[$id] = $group
    Save-State
    Write-Output $id
  }
  "split-window" {
    $target = Arg-After "-t"
    $group = $null
    if ($null -ne $target -and $target.Contains(":")) {
      $group = $target.Split(":")[0]
      if (-not $state.groups.ContainsKey($group) -or $state.groups[$group].Count -eq 0) {
        Write-Error "missing session: $group"
        exit 1
      }
    } elseif ($null -ne $target -and $state.panes.ContainsKey($target)) {
      $group = $state.panes[$target]
    } else {
      Write-Error "missing pane target: $target"
      exit 1
    }
    $id = "%$($state.next)"
    $state.next = $state.next + 1
    $state.groups[$group] = @($state.groups[$group] + $id)
    $state.panes[$id] = $group
    Save-State
    Write-Output $id
  }
  "kill-pane" {
    $target = Arg-After "-t"
    if ($null -ne $target -and $state.panes.ContainsKey($target)) {
      $group = $state.panes[$target]
      $remaining = @()
      foreach ($pane in $state.groups[$group]) {
        if ([string]$pane -ne $target) {
          $remaining += [string]$pane
        }
      }
      if ($remaining.Count -eq 0) {
        $state.groups.Remove($group)
      } else {
        $state.groups[$group] = $remaining
      }
      $state.panes.Remove($target)
      Save-State
    }
  }
  "list-panes" {
    $group = Arg-After "-t"
    if ($null -ne $group -and $state.groups.ContainsKey($group)) {
      foreach ($pane in $state.groups[$group]) {
        Write-Output $pane
      }
      exit 0
    }
    Write-Error "missing session: $group"
    exit 1
  }
  "select-layout" {
    Save-State
    exit 0
  }
  default {
    Write-Error "unsupported fake tmux command: $($CmdArgs -join ' ')"
    exit 1
  }
}
"#
    .to_string();
    fs::write(&path, body).expect("write fake tmux script");
    path
}

#[cfg(windows)]
fn write_fake_iterm_script(root: &PathBuf) -> PathBuf {
    let path = root.join(format!("fake-iterm-{}.ps1", unique_suffix()));
    let body = r#"
$StatePath = $env:HELLOX_FAKE_ITERM_STATE
$LogPath = $env:HELLOX_FAKE_ITERM_LOG
$ScriptBody = if ($args.Length -ge 2 -and $args[0] -eq "-e") { [string]$args[1] } else { "" }

$state = @{ next = 1; groups = @{}; sessions = @{} }
if (Test-Path $StatePath) {
  $raw = Get-Content $StatePath -Raw
  if ($raw.Trim().Length -gt 0) {
    $json = $raw | ConvertFrom-Json -AsHashtable
    if ($json.ContainsKey("next")) {
      $state.next = [int]$json["next"]
    }
    if ($json.ContainsKey("groups")) {
      $state.groups = @{}
      foreach ($entry in $json["groups"].GetEnumerator()) {
        $values = @()
        foreach ($sessionId in $entry.Value) {
          $values += [string]$sessionId
        }
        $state.groups[$entry.Key] = $values
      }
    }
    if ($json.ContainsKey("sessions")) {
      $state.sessions = @{}
      foreach ($entry in $json["sessions"].GetEnumerator()) {
        $state.sessions[$entry.Key] = [string]$entry.Value
      }
    }
  }
}

function Save-State {
  $state | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $StatePath
}

function Match-First {
  param([string]$Pattern)
  $match = [regex]::Match($ScriptBody, $Pattern)
  if ($match.Success) {
    return [string]$match.Groups[1].Value
  }
  return $null
}

if ($ScriptBody.Contains("tell candidateSession to close")) {
  $target = Match-First 'if \(id of candidateSession as text\) is "([^"]+)" then'
  if ($null -ne $target -and $state.sessions.ContainsKey($target)) {
    $group = [string]$state.sessions[$target]
    $remaining = @()
    foreach ($sessionId in $state.groups[$group]) {
      if ([string]$sessionId -ne $target) {
        $remaining += [string]$sessionId
      }
    }
    if ($remaining.Count -eq 0) {
      $state.groups.Remove($group)
    } else {
      $state.groups[$group] = $remaining
    }
    $state.sessions.Remove($target)
    Add-Content -Encoding utf8 $LogPath "close target=$target"
  } else {
    Add-Content -Encoding utf8 $LogPath "close-missing target=$target"
  }
  Save-State
  Write-Output "closed"
  exit 0
}

if ($ScriptBody.Contains("set sessionIds to {}")) {
  $group = Match-First 'if \(custom title of candidateSession as text\) is "([^"]+)" then'
  if ($null -ne $group -and $state.groups.ContainsKey($group)) {
    foreach ($sessionId in $state.groups[$group]) {
      Write-Output ([string]$sessionId)
    }
  }
  exit 0
}

$group = Match-First 'custom title of candidateSession as text\) is "([^"]+)"'
if ($null -eq $group -or $group.Trim().Length -eq 0) {
  $group = "default"
}
$groupExisted = $state.groups.ContainsKey($group) -and $state.groups[$group].Count -gt 0
$anchor = Match-First 'if \(id of candidateSession as text\) is "([^"]+)" then'
$resolvedAnchor = $null
if ($null -ne $anchor -and $state.sessions.ContainsKey($anchor)) {
  $resolvedAnchor = $anchor
} elseif ($groupExisted) {
  $resolvedAnchor = [string]$state.groups[$group][0]
}

$newId = "session-$($state.next)"
$state.next = $state.next + 1
if (-not $state.groups.ContainsKey($group)) {
  $state.groups[$group] = @()
}
$state.groups[$group] = @($state.groups[$group] + $newId)
$state.sessions[$newId] = $group

if ($ScriptBody.Contains("split horizontal") -and ($groupExisted -or $null -ne $resolvedAnchor)) {
  Add-Content -Encoding utf8 $LogPath "split direction=horizontal group=$group anchor=$resolvedAnchor id=$newId"
} elseif ($ScriptBody.Contains("split vertical") -and ($groupExisted -or $null -ne $resolvedAnchor)) {
  Add-Content -Encoding utf8 $LogPath "split direction=vertical group=$group anchor=$resolvedAnchor id=$newId"
} elseif ($ScriptBody.Contains("create window with default profile command")) {
  Add-Content -Encoding utf8 $LogPath "create group=$group id=$newId"
} else {
  Write-Error "unsupported fake iterm script"
  exit 1
}

Save-State
Write-Output $newId
"#;
    fs::write(&path, body).expect("write fake iterm script");
    path
}
