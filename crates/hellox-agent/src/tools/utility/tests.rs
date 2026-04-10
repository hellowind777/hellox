use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use hellox_gateway_api::ToolResultContent;
use serde_json::json;
use uuid::Uuid;

use crate::permissions::PermissionPolicy;
use crate::planning::PlanningState;
use crate::tools::{default_tool_registry, ToolExecutionContext};
use hellox_config::PermissionMode;

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new() -> Self {
        let root = env::temp_dir().join(format!("hellox-utility-tool-{}", Uuid::new_v4()));
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
async fn config_tool_updates_and_shows_local_config() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let registry = default_tool_registry();

    let result = registry
        .execute(
            "config",
            json!({
                "key": "session.model",
                "value": "gpt-5"
            }),
            &context,
        )
        .await;
    assert!(!result.is_error);

    let text = text_result(registry.execute("config", json!({}), &context).await);
    assert!(text.contains("\"model\": \"gpt-5\""), "{text}");
}

#[tokio::test]
async fn config_tool_updates_prompt_layers() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let registry = default_tool_registry();

    let persona = registry
        .execute(
            "Config",
            json!({
                "key": "prompt.persona",
                "value": "reviewer"
            }),
            &context,
        )
        .await;
    assert!(!persona.is_error);

    let fragments = registry
        .execute(
            "Config",
            json!({
                "key": "prompt.fragments",
                "value": ["safety", "checklist"]
            }),
            &context,
        )
        .await;
    assert!(!fragments.is_error);

    let text = text_result(registry.execute("Config", json!({}), &context).await);
    assert!(text.contains("\"persona\": \"reviewer\""), "{text}");
    assert!(text.contains("\"fragments\": ["), "{text}");
    assert!(text.contains("\"safety\""), "{text}");
    assert!(text.contains("\"checklist\""), "{text}");
}

#[tokio::test]
async fn tool_search_finds_matching_tool_names() {
    let workspace = TestWorkspace::new();
    let registry = default_tool_registry();
    let text = text_result(
        registry
            .execute(
                "ToolSearch",
                json!({ "query": "mcp" }),
                &workspace.context(),
            )
            .await,
    );
    // ToolSearch returns canonical tool names (Claude Code naming).
    assert!(text.contains("\"name\": \"MCP\""), "{text}");
}

#[tokio::test]
async fn sleep_tool_waits_for_requested_duration() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();
    let registry = default_tool_registry();
    let started = Instant::now();
    let result = registry
        .execute("Sleep", json!({ "duration_ms": 25 }), &context)
        .await;
    assert!(!result.is_error);
    assert!(started.elapsed().as_millis() >= 20);
}
