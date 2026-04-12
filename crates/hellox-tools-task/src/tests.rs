use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use hellox_gateway_api::ToolResultContent;
use hellox_tool_runtime::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{register_tools, task_file_path, PlanItem, TaskToolContext};

static TEST_WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct TestPlanningState {
    active: bool,
    plan: Vec<PlanItem>,
    allowed_prompts: Vec<String>,
}

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let unique = TEST_WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("hellox-tools-task-{suffix}-{unique}"));
        fs::create_dir_all(&root).expect("create temp root");
        Self { root }
    }

    fn context(&self) -> TestContext {
        TestContext {
            root: self.root.clone(),
            config_path: self.root.join("config.toml"),
            planning_state: Arc::new(Mutex::new(TestPlanningState::default())),
        }
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
struct TestContext {
    root: PathBuf,
    config_path: PathBuf,
    planning_state: Arc<Mutex<TestPlanningState>>,
}

#[async_trait]
impl TaskToolContext for TestContext {
    fn working_directory(&self) -> &Path {
        &self.root
    }

    fn config_path(&self) -> &Path {
        &self.config_path
    }

    async fn ensure_write_allowed(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    fn enter_plan_mode(&self) -> anyhow::Result<Value> {
        let mut state = self.planning_state.lock().expect("lock");
        state.active = true;
        serde_json::to_value(&*state).map_err(Into::into)
    }

    fn exit_plan_mode(
        &self,
        plan: Vec<PlanItem>,
        allowed_prompts: Vec<String>,
    ) -> anyhow::Result<Value> {
        let mut state = self.planning_state.lock().expect("lock");
        state.active = false;
        state.plan = plan;
        state.allowed_prompts = allowed_prompts;
        serde_json::to_value(&*state).map_err(Into::into)
    }
}

fn text_result(result: hellox_tool_runtime::LocalToolResult) -> String {
    match result.content {
        ToolResultContent::Text(text) => text,
        other => panic!("expected text result, got {other:?}"),
    }
}

#[tokio::test]
async fn task_tools_roundtrip_structured_tasks() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);

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

    let raw = fs::read_to_string(task_file_path(&workspace.root)).expect("read task file");
    assert!(raw.contains("\"cancelled\""));
    assert!(raw.contains("waiting for workflow layer"));
}

#[tokio::test]
async fn plan_mode_tools_update_context_state() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);

    registry.execute("EnterPlanMode", json!({}), &context).await;
    assert!(context.planning_state.lock().expect("lock").active);

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

    let planning = context.planning_state.lock().expect("lock").clone();
    assert!(!planning.active);
    assert_eq!(planning.plan.len(), 2);
    assert_eq!(
        planning.allowed_prompts,
        vec![String::from("continue implementation")]
    );
}

#[tokio::test]
async fn todo_write_persists_and_returns_previous_items() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);

    registry
        .execute(
            "TodoWrite",
            json!({
                "todos": [{ "content": "build runtime", "status": "pending" }]
            }),
            &context,
        )
        .await;

    let second = text_result(
        registry
            .execute(
                "TodoWrite",
                json!({
                    "todos": [{ "content": "verify runtime", "status": "in_progress" }]
                }),
                &context,
            )
            .await,
    );

    assert!(second.contains("build runtime"), "{second}");
    assert!(second.contains("verify runtime"), "{second}");
    let stored =
        fs::read_to_string(workspace.root.join(".hellox").join("todos.json")).expect("read todos");
    assert!(stored.contains("verify runtime"), "{stored}");
}

#[tokio::test]
async fn cron_tools_create_list_and_delete_scheduled_tasks() {
    crate::cron_storage::reset_session_tasks();
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);

    let created = text_result(
        registry
            .execute(
                "CronCreate",
                json!({
                    "cron": "*/5 * * * *",
                    "prompt": "Check staging health",
                    "durable": true
                }),
                &context,
            )
            .await,
    );
    assert!(created.contains("\"id\": \"cron-1\""), "{created}");
    assert!(created.contains("Check staging health"), "{created}");

    let listed = text_result(registry.execute("CronList", json!({}), &context).await);
    assert!(listed.contains("\"cron-1\""), "{listed}");
    assert!(listed.contains("Check staging health"), "{listed}");

    let deleted = text_result(
        registry
            .execute("CronDelete", json!({ "id": "cron-1" }), &context)
            .await,
    );
    assert!(deleted.contains("cron-1"), "{deleted}");

    let after = text_result(registry.execute("CronList", json!({}), &context).await);
    assert!(after.contains("\"tasks\": []"), "{after}");
}
