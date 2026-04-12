mod question;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_gateway_api::{ToolDefinition, ToolResultContent};
use serde_json::Value;

pub use question::{AskUserQuestionTool, BlockingQuestion, QuestionToolContext};

/// Represents a local tool execution result and whether it should be treated as an error.
#[derive(Debug, Clone)]
pub struct LocalToolResult {
    pub content: ToolResultContent,
    pub is_error: bool,
}

impl LocalToolResult {
    /// Creates a successful text result.
    pub fn text(text: String) -> Self {
        Self {
            content: ToolResultContent::Text(text),
            is_error: false,
        }
    }

    /// Creates a failing text result.
    pub fn error(text: String) -> Self {
        Self {
            content: ToolResultContent::Text(text),
            is_error: true,
        }
    }
}

/// Defines a local tool that can be registered into a runtime registry.
#[async_trait]
pub trait LocalTool<Context>: Send + Sync {
    /// Returns the static definition exposed to the model/runtime.
    fn definition(&self) -> ToolDefinition;

    /// Executes the tool with JSON input against the provided runtime context.
    async fn call(&self, input: Value, context: &Context) -> Result<LocalToolResult>;
}

/// Stores local tools by name and dispatches calls against a concrete runtime context type.
#[derive(Clone)]
pub struct ToolRegistry<Context> {
    tools: BTreeMap<String, Arc<dyn LocalTool<Context>>>,
}

impl<Context> Default for ToolRegistry<Context> {
    fn default() -> Self {
        Self {
            tools: BTreeMap::new(),
        }
    }
}

impl<Context> ToolRegistry<Context> {
    /// Registers a tool instance by its definition name.
    pub fn register<T>(&mut self, tool: T)
    where
        T: LocalTool<Context> + 'static,
    {
        let definition = tool.definition();
        self.tools.insert(definition.name.clone(), Arc::new(tool));
    }

    /// Returns the registered tool definitions.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    /// Executes a registered tool and normalizes unknown tools or internal failures into errors.
    pub async fn execute(&self, name: &str, input: Value, context: &Context) -> LocalToolResult {
        let Some(tool) = self
            .tools
            .get(name)
            .or_else(|| resolve_tool_alias(name).and_then(|alias| self.tools.get(alias)))
        else {
            return LocalToolResult::error(format!("unknown tool: {name}"));
        };

        match tool.call(input, context).await {
            Ok(result) => result,
            Err(error) => LocalToolResult::error(error.to_string()),
        }
    }
}

fn resolve_tool_alias(name: &str) -> Option<&'static str> {
    Some(match name {
        // Filesystem tools.
        "list_files" => "ListFiles",
        "read_file" => "Read",
        "write_file" => "Write",
        "edit_file" => "Edit",
        "notebook_edit" => "NotebookEdit",
        "glob" => "Glob",
        "grep" => "Grep",
        // Shell tools.
        "run_shell" => {
            if cfg!(windows) {
                "PowerShell"
            } else {
                "Bash"
            }
        }
        // Web tools.
        "web_fetch" => "WebFetch",
        "web_search" => "WebSearch",
        // UI tools.
        "brief" => "SendUserMessage",
        "config" => "Config",
        "tool_search" => "ToolSearch",
        // Task/planning tools.
        "task_create" => "TaskCreate",
        "task_get" => "TaskGet",
        "task_list" => "TaskList",
        "task_update" => "TaskUpdate",
        "task_stop" => "TaskStop",
        "task_output" => "TaskOutput",
        "enter_plan_mode" => "EnterPlanMode",
        "exit_plan_mode" => "ExitPlanMode",
        "todo_write" => "TodoWrite",
        // User interaction tools.
        "ask_user_question" => "AskUserQuestion",
        // Agent/team/workflow tools.
        "agent" => "Agent",
        "agent_status" => "AgentStatus",
        "agent_wait" => "AgentWait",
        "agent_list" => "AgentList",
        "agent_stop" => "AgentStop",
        "send_message" => "SendMessage",
        "team_create" => "TeamCreate",
        "team_update" => "TeamUpdate",
        "team_delete" => "TeamDelete",
        "team_status" => "TeamStatus",
        "team_wait" => "TeamWait",
        "team_stop" => "TeamStop",
        "team_run" => "TeamRun",
        "workflow" => "Workflow",
        "sleep" => "Sleep",
        "remote_trigger" => "RemoteTrigger",
        // MCP tools.
        "mcp" => "MCP",
        "list_mcp_resources" => "ListMcpResources",
        "read_mcp_resource" => "ReadMcpResource",
        "list_mcp_prompts" => "ListMcpPrompts",
        "get_mcp_prompt" => "GetMcpPrompt",
        "mcp_auth" => "McpAuth",
        _ => return None,
    })
}

/// Reads a required string field from a JSON input object.
pub fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing required string field `{key}`"))
}

/// Formats a path relative to the provided root when possible.
pub fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_trait::async_trait;
    use hellox_gateway_api::ToolResultContent;
    use serde_json::json;

    use super::{display_path, required_string, LocalTool, LocalToolResult, ToolRegistry};

    struct EchoTool;

    #[async_trait]
    impl LocalTool<String> for EchoTool {
        fn definition(&self) -> hellox_gateway_api::ToolDefinition {
            hellox_gateway_api::ToolDefinition {
                name: "echo".to_string(),
                description: Some("Echo input with context".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }),
            }
        }

        async fn call(
            &self,
            input: serde_json::Value,
            context: &String,
        ) -> Result<LocalToolResult> {
            let message = required_string(&input, "message")?;
            Ok(LocalToolResult::text(format!("{context}:{message}")))
        }
    }

    #[tokio::test]
    async fn registry_executes_registered_tool() {
        let mut registry = ToolRegistry::<String>::default();
        registry.register(EchoTool);

        let result = registry
            .execute("echo", json!({ "message": "hello" }), &"ctx".to_string())
            .await;

        match result.content {
            ToolResultContent::Text(text) => assert_eq!(text, "ctx:hello"),
            other => panic!("expected text result, got {other:?}"),
        }
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn registry_reports_unknown_tool_as_error() {
        let registry = ToolRegistry::<String>::default();
        let result = registry
            .execute("missing", json!({}), &"ctx".to_string())
            .await;

        match result.content {
            ToolResultContent::Text(text) => assert!(text.contains("unknown tool: missing")),
            other => panic!("expected text result, got {other:?}"),
        }
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn registry_resolves_known_tool_aliases() {
        let mut registry = ToolRegistry::<String>::default();

        struct ReadTool;

        #[async_trait]
        impl LocalTool<String> for ReadTool {
            fn definition(&self) -> hellox_gateway_api::ToolDefinition {
                hellox_gateway_api::ToolDefinition {
                    name: "Read".to_string(),
                    description: None,
                    input_schema: json!({}),
                }
            }

            async fn call(
                &self,
                _input: serde_json::Value,
                _context: &String,
            ) -> Result<LocalToolResult> {
                Ok(LocalToolResult::text("ok".to_string()))
            }
        }

        registry.register(ReadTool);

        let result = registry
            .execute("read_file", json!({}), &"ctx".to_string())
            .await;

        match result.content {
            ToolResultContent::Text(text) => assert_eq!(text, "ok"),
            other => panic!("expected text result, got {other:?}"),
        }
        assert!(!result.is_error);
    }

    #[test]
    fn display_path_prefers_relative_paths() {
        let root = std::path::Path::new("D:/repo");
        let path = std::path::Path::new("D:/repo/docs/readme.md");
        assert_eq!(display_path(root, path), "docs/readme.md");
    }
}
