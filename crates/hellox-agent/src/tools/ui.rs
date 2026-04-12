use async_trait::async_trait;

use super::{ToolExecutionContext, ToolRegistry};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tools_ui::BriefTool);
    registry.register_runtime(hellox_tools_ui::ConfigTool);
    registry.register_runtime(hellox_tools_ui::SkillTool);
    registry.register_runtime(hellox_tools_ui::ToolSearchTool);
}

#[async_trait]
impl hellox_tools_ui::UiToolContext for ToolExecutionContext {
    async fn ensure_write_allowed(&self, path: &std::path::Path) -> anyhow::Result<()> {
        ToolExecutionContext::ensure_write_allowed(self, path).await
    }

    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    fn available_tool_definitions(&self) -> Vec<hellox_gateway_api::ToolDefinition> {
        crate::default_tool_registry().definitions()
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use hellox_gateway_api::ToolResultContent;
    use serde_json::{json, Value};
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
            let root = env::temp_dir().join(format!("hellox-ui-tool-{}", Uuid::new_v4()));
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

    fn text_result(result: super::super::LocalToolResult) -> String {
        match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        }
    }

    async fn execute(name: &str, input: Value, context: &ToolExecutionContext) -> String {
        let registry = default_tool_registry();
        text_result(registry.execute(name, input, context).await)
    }

    #[tokio::test]
    async fn brief_tool_persists_structured_brief() {
        let workspace = TestWorkspace::new();
        let text = execute(
            "brief",
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
        .await;
        assert!(text.contains("Need review"), "{text}");
        assert!(text.contains("notes/todo.md"), "{text}");
        assert!(text.contains("\"status\": \"in_progress\""), "{text}");

        let stored = fs::read_to_string(workspace.root.join(".hellox").join("brief.json"))
            .expect("read brief file");
        assert!(stored.contains("artifacts/summary.txt"), "{stored}");
        assert!(stored.contains("\"label\": \"summary\""), "{stored}");
    }

    #[tokio::test]
    async fn tool_search_lists_registry_definitions() {
        let workspace = TestWorkspace::new();
        let text = execute(
            "ToolSearch",
            json!({
                "query": "brief"
            }),
            &workspace.context(),
        )
        .await;
        // ToolSearch returns canonical tool names (Claude Code naming).
        assert!(text.contains("\"name\": \"SendUserMessage\""), "{text}");
    }

    #[tokio::test]
    async fn skill_tool_is_available_in_default_registry() {
        let workspace = TestWorkspace::new();
        let skills_root = workspace.root.join(".hellox").join("skills");
        fs::create_dir_all(&skills_root).expect("create skills root");
        fs::write(
            skills_root.join("review.md"),
            r#"---
name: review
description: Review the current change set.
---
Focus on correctness first."#,
        )
        .expect("write skill");

        let text = execute(
            "Skill",
            json!({
                "skill": "review",
                "args": "src/lib.rs"
            }),
            &workspace.context(),
        )
        .await;

        assert!(text.contains("\"name\": \"review\""), "{text}");
        assert!(text.contains("Focus on correctness first."), "{text}");
    }
}
