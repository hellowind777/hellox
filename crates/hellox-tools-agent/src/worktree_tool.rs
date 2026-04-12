use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};
use serde_json::json;

use crate::worktree::{enter_worktree, exit_worktree, ExitWorktreeAction};

#[async_trait]
pub trait WorktreeToolContext {
    fn working_directory(&self) -> &Path;
}

pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: WorktreeToolContext + Send + Sync + 'static,
{
    registry.register(EnterWorktreeTool);
    registry.register(ExitWorktreeTool);
}

pub struct EnterWorktreeTool;
pub struct ExitWorktreeTool;

#[async_trait]
impl<C> LocalTool<C> for EnterWorktreeTool
where
    C: WorktreeToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "EnterWorktree".to_string(),
            description: Some(
                "Create or reuse a managed local git worktree under .hellox/worktrees.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "base_ref": { "type": "string" },
                    "reuse_existing": { "type": "boolean" }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, input: serde_json::Value, context: &C) -> Result<LocalToolResult> {
        let name = required_string(&input, "name")?;
        let record = enter_worktree(
            context.working_directory(),
            name,
            optional_string(&input, "base_ref").as_deref(),
            input
                .get("reuse_existing")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        )?;
        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "name": record.name,
                "repo_root": normalize_path(&record.repo_root),
                "worktree_path": normalize_path(&record.path),
                "branch": record.branch,
                "head": record.head,
            }),
        )?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for ExitWorktreeTool
where
    C: WorktreeToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "ExitWorktree".to_string(),
            description: Some(
                "Inspect or remove a managed local git worktree under .hellox/worktrees."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "path": { "type": "string" },
                    "action": {
                        "type": "string",
                        "enum": ["keep", "remove", "force_remove"]
                    }
                }
            }),
        }
    }

    async fn call(&self, input: serde_json::Value, context: &C) -> Result<LocalToolResult> {
        let action = match optional_string(&input, "action").as_deref() {
            Some("keep") => ExitWorktreeAction::Keep,
            Some("force_remove") => ExitWorktreeAction::ForceRemove,
            Some("remove") | None => ExitWorktreeAction::Remove,
            Some(other) => anyhow::bail!("unsupported ExitWorktree action `{other}`"),
        };
        let record = exit_worktree(
            context.working_directory(),
            optional_string(&input, "name").as_deref(),
            optional_string(&input, "path").as_deref(),
            action,
        )?;
        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "name": record.name,
                "repo_root": normalize_path(&record.repo_root),
                "worktree_path": normalize_path(&record.path),
                "branch": record.branch,
                "head": record.head,
                "action": match action {
                    ExitWorktreeAction::Keep => "keep",
                    ExitWorktreeAction::Remove => "remove",
                    ExitWorktreeAction::ForceRemove => "force_remove",
                },
            }),
        )?))
    }
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn optional_string(input: &serde_json::Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use async_trait::async_trait;
    use hellox_gateway_api::ToolResultContent;
    use hellox_tool_runtime::ToolRegistry;
    use serde_json::json;

    use super::{register_tools, WorktreeToolContext};

    struct TestContext {
        working_directory: PathBuf,
    }

    #[async_trait]
    impl WorktreeToolContext for TestContext {
        fn working_directory(&self) -> &Path {
            &self.working_directory
        }
    }

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("hellox-worktree-tool-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn git(directory: &PathBuf, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_repo(root: &PathBuf) {
        git(root, &["init"]);
        git(root, &["config", "user.email", "local@example.com"]);
        git(root, &["config", "user.name", "Local"]);
        std::fs::write(root.join("README.md"), "hello\n").expect("write readme");
        git(root, &["add", "README.md"]);
        git(root, &["commit", "-m", "init"]);
    }

    fn text_result(result: hellox_tool_runtime::LocalToolResult) -> String {
        match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn enter_and_exit_worktree_through_registry() {
        let root = temp_dir();
        init_repo(&root);
        let mut registry = ToolRegistry::<TestContext>::default();
        register_tools(&mut registry);
        let context = TestContext {
            working_directory: root.clone(),
        };

        let entered = text_result(
            registry
                .execute("EnterWorktree", json!({"name": "review"}), &context)
                .await,
        );
        assert!(entered.contains("\"name\": \"review\""), "{entered}");
        assert!(entered.contains(".hellox/worktrees/review"), "{entered}");

        let exited = text_result(
            registry
                .execute(
                    "ExitWorktree",
                    json!({"name": "review", "action": "remove"}),
                    &context,
                )
                .await,
        );
        assert!(exited.contains("\"action\": \"remove\""), "{exited}");
    }
}
