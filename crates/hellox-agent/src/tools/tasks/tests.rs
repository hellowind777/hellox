use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_gateway_api::ToolResultContent;
use serde_json::json;

use crate::permissions::PermissionPolicy;
use crate::planning::PlanningState;
use crate::tools::{default_tool_registry, ToolExecutionContext};
use hellox_config::PermissionMode;

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-agent-tasks-{suffix}"));
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

fn text_result(result: crate::tools::LocalToolResult) -> String {
    match result.content {
        ToolResultContent::Text(text) => text,
        other => panic!("expected text result, got {other:?}"),
    }
}

#[tokio::test]
async fn task_tools_roundtrip_structured_tasks() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let registry = default_tool_registry();

    registry
        .execute(
            "TaskCreate",
            json!({
                "title": "Implement plan mode",
                "description": "Persist planning state in sessions",
                "priority": "high"
            }),
            &context,
        )
        .await;

    let listed_text = text_result(
        registry
            .execute("TaskList", json!({ "status": "pending" }), &context)
            .await,
    );
    assert!(listed_text.contains("Implement plan mode"));
    assert!(listed_text.contains("Persist planning state in sessions"));

    registry
        .execute(
            "TaskUpdate",
            json!({
                "id": "task-1",
                "status": "in_progress",
                "output": "session prompt now reflects planning guidance"
            }),
            &context,
        )
        .await;

    let fetched_text = text_result(
        registry
            .execute("TaskGet", json!({ "id": "task-1" }), &context)
            .await,
    );
    assert!(fetched_text.contains("\"status\": \"in_progress\""));
    assert!(fetched_text.contains("planning guidance"));

    let output_text = text_result(
        registry
            .execute("TaskOutput", json!({ "id": "task-1" }), &context)
            .await,
    );
    assert!(output_text.contains("\"output\": \"session prompt now reflects planning guidance\""));

    registry
        .execute(
            "TaskStop",
            json!({
                "id": "task-1",
                "reason": "waiting for workflow layer"
            }),
            &context,
        )
        .await;

    let raw = fs::read_to_string(hellox_tools_task::task_file_path(&workspace.root))
        .expect("read task file");
    assert!(raw.contains("\"cancelled\""));
    assert!(raw.contains("waiting for workflow layer"));
}

#[tokio::test]
async fn plan_mode_tools_update_context_state() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let registry = default_tool_registry();

    registry.execute("EnterPlanMode", json!({}), &context).await;
    assert!(context.planning_state().expect("planning state").active);

    registry
        .execute(
            "ExitPlanMode",
            json!({
                "plan": [
                    { "step": "Design the session state", "status": "completed" },
                    { "step": "Implement plan-mode tools", "status": "in_progress" }
                ],
                "allowed_prompts": ["continue implementation"]
            }),
            &context,
        )
        .await;

    let planning = context.planning_state().expect("planning state");
    assert!(!planning.active);
    assert_eq!(planning.plan.len(), 2);
    assert_eq!(
        planning.allowed_prompts,
        vec![String::from("continue implementation")]
    );
}
