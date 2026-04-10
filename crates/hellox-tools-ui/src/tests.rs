use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hellox_gateway_api::{ToolDefinition, ToolResultContent};
use hellox_tool_runtime::ToolRegistry;
use serde_json::json;
use uuid::Uuid;

use crate::{register_tools, UiToolContext};

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new() -> Self {
        let root = env::temp_dir().join(format!("hellox-tools-ui-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp root");
        Self { root }
    }

    fn context(&self) -> TestContext {
        TestContext {
            working_directory: self.root.clone(),
            config_path: self.root.join(".hellox").join("config.toml"),
            denied_paths: Arc::new(Mutex::new(Vec::new())),
            tool_definitions: vec![
                ToolDefinition {
                    name: "MCP".to_string(),
                    description: Some("Manage local MCP servers".to_string()),
                    input_schema: json!({}),
                },
                ToolDefinition {
                    name: "Workflow".to_string(),
                    description: Some("Run workflow scripts".to_string()),
                    input_schema: json!({}),
                },
            ],
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
    working_directory: PathBuf,
    config_path: PathBuf,
    denied_paths: Arc<Mutex<Vec<PathBuf>>>,
    tool_definitions: Vec<ToolDefinition>,
}

#[async_trait]
impl UiToolContext for TestContext {
    async fn ensure_write_allowed(&self, path: &Path) -> anyhow::Result<()> {
        let denied = self.denied_paths.lock().expect("lock");
        if denied.iter().any(|item| item == path) {
            anyhow::bail!("blocked path: {}", path.display());
        }
        Ok(())
    }

    fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    fn config_path(&self) -> &Path {
        &self.config_path
    }

    fn available_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_definitions.clone()
    }
}

fn text_result(result: hellox_tool_runtime::LocalToolResult) -> String {
    match result.content {
        ToolResultContent::Text(text) => text,
        other => panic!("expected text result, got {other:?}"),
    }
}

#[tokio::test]
async fn brief_tool_persists_structured_brief_through_registry() {
    let workspace = TestWorkspace::new();
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);

    let text = text_result(
        registry
            .execute(
                "SendUserMessage",
                json!({
                    "message": "Need review on the latest notebook output.",
                    "status": "in_progress",
                    "attachments": [
                        "notes/todo.md",
                        { "path": "artifacts/summary.txt", "label": "summary" }
                    ]
                }),
                &workspace.context(),
            )
            .await,
    );

    assert!(text.contains("Need review"), "{text}");
    assert!(text.contains("notes/todo.md"), "{text}");
    assert!(text.contains("\"status\": \"in_progress\""), "{text}");

    let stored = fs::read_to_string(workspace.root.join(".hellox").join("brief.json"))
        .expect("read brief file");
    assert!(stored.contains("artifacts/summary.txt"), "{stored}");
    assert!(stored.contains("\"label\": \"summary\""), "{stored}");
}

#[tokio::test]
async fn config_tool_updates_and_shows_local_config_through_registry() {
    let workspace = TestWorkspace::new();
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);
    let context = workspace.context();

    let result = registry
        .execute(
            "Config",
            json!({
                "key": "prompt.fragments",
                "value": ["safety", "checklist"]
            }),
            &context,
        )
        .await;
    assert!(!result.is_error);

    let text = text_result(registry.execute("Config", json!({}), &context).await);
    assert!(text.contains("\"fragments\": ["), "{text}");
    assert!(text.contains("\"safety\""), "{text}");
    assert!(text.contains("\"checklist\""), "{text}");
}

#[tokio::test]
async fn tool_search_filters_available_definitions_through_registry() {
    let workspace = TestWorkspace::new();
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);

    let text = text_result(
        registry
            .execute(
                "ToolSearch",
                json!({ "query": "mcp" }),
                &workspace.context(),
            )
            .await,
    );

    assert!(text.contains("\"name\": \"MCP\""), "{text}");
    assert!(!text.contains("\"name\": \"Workflow\""), "{text}");
}
