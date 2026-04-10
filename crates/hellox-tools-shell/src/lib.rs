use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

/// Minimal shell-tool context contract needed by the `Bash`/`PowerShell` tool.
#[async_trait]
pub trait ShellToolContext: Send + Sync {
    /// Validates whether the command may run under the current permission policy.
    async fn ensure_shell_allowed(&self, command: &str) -> Result<()>;

    /// Returns the current working directory for the shell invocation.
    fn working_directory(&self) -> &Path;
}

/// Registers shell-domain tools into a shared tool registry.
pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: ShellToolContext + Send + Sync + 'static,
{
    registry.register(RunShellTool);
}

pub struct RunShellTool;

#[async_trait]
impl<C> LocalTool<C> for RunShellTool
where
    C: ShellToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        let tool_name = if cfg!(windows) { "PowerShell" } else { "Bash" };
        hellox_gateway_api::ToolDefinition {
            name: tool_name.to_string(),
            description: Some(format!(
                "Run a {tool_name} command in the current workspace."
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute." },
                    "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds (max 600000). Defaults to 120000." },
                    "timeout_secs": { "type": "integer", "description": "Alias for `timeout_ms` expressed in seconds." },
                    "description": { "type": "string", "description": "Optional short description of what the command does." },
                    "run_in_background": { "type": "boolean", "description": "Whether to run the command in the background (not supported yet)." },
                    "dangerouslyDisableSandbox": { "type": "boolean", "description": "Disable sandboxing (not supported in local-first mode)." }
                },
                "required": ["command"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let command_text = required_string(&input, "command")?;
        let run_in_background = input
            .get("run_in_background")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if run_in_background {
            return Err(anyhow!(
                "run_in_background is not supported yet; run the command synchronously"
            ));
        }
        if input
            .get("dangerouslyDisableSandbox")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(anyhow!(
                "dangerouslyDisableSandbox is not supported in local-first mode"
            ));
        }

        let timeout_ms = match (
            input.get("timeout_ms").and_then(Value::as_u64),
            input.get("timeout_secs").and_then(Value::as_u64),
        ) {
            (Some(ms), _) => ms,
            (None, Some(secs)) => secs.saturating_mul(1000),
            (None, None) => DEFAULT_TIMEOUT_MS,
        };
        if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
            return Err(anyhow!(
                "timeout_ms must be between 1 and {MAX_TIMEOUT_MS} (got {timeout_ms})"
            ));
        }
        context.ensure_shell_allowed(command_text).await?;

        let started = std::time::Instant::now();
        let mut command = if cfg!(windows) {
            let mut cmd = Command::new("powershell");
            cmd.arg("-NoProfile").arg("-Command").arg(command_text);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.arg("-lc").arg(command_text);
            cmd
        };
        command.current_dir(context.working_directory());

        let output = match timeout(Duration::from_millis(timeout_ms), command.output()).await {
            Ok(result) => result?,
            Err(_) => {
                let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                let body = serde_json::to_string_pretty(&json!({
                    "interrupted": true,
                    "timeout_ms": timeout_ms,
                    "duration_ms": duration_ms,
                    "stdout": "",
                    "stderr": format!("shell command timed out after {timeout_ms}ms"),
                }))?;
                return Ok(LocalToolResult::error(body));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let body = serde_json::to_string_pretty(&json!({
            "interrupted": false,
            "exit_code": exit_code,
            "duration_ms": duration_ms,
            "stdout": stdout.trim_end(),
            "stderr": stderr.trim_end(),
        }))?;

        if output.status.success() {
            Ok(LocalToolResult::text(body))
        } else {
            Ok(LocalToolResult::error(body))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use hellox_gateway_api::ToolResultContent;
    use serde_json::json;

    use super::{register_tools, ShellToolContext};
    use hellox_tool_runtime::ToolRegistry;

    #[derive(Clone, Default)]
    struct TestContext {
        working_directory: PathBuf,
        denied_commands: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl ShellToolContext for TestContext {
        async fn ensure_shell_allowed(&self, command: &str) -> anyhow::Result<()> {
            let denied = self.denied_commands.lock().expect("lock");
            if denied.iter().any(|item| item == command) {
                anyhow::bail!("blocked command: {command}");
            }
            Ok(())
        }

        fn working_directory(&self) -> &Path {
            &self.working_directory
        }
    }

    #[tokio::test]
    async fn run_shell_tool_executes_command_through_registry() {
        let mut registry = ToolRegistry::<TestContext>::default();
        register_tools(&mut registry);
        let context = TestContext {
            working_directory: std::env::temp_dir(),
            denied_commands: Arc::new(Mutex::new(Vec::new())),
        };

        let command = if cfg!(windows) {
            "Write-Output hello-shell"
        } else {
            "printf hello-shell"
        };
        let tool_name = if cfg!(windows) { "PowerShell" } else { "Bash" };

        let result = registry
            .execute(tool_name, json!({ "command": command }), &context)
            .await;

        match result.content {
            ToolResultContent::Text(text) => {
                assert!(text.contains("\"exit_code\": 0"), "{text}");
                assert!(text.contains("hello-shell"), "{text}");
            }
            other => panic!("expected text result, got {other:?}"),
        }
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn run_shell_tool_respects_context_permission_check() {
        let mut registry = ToolRegistry::<TestContext>::default();
        register_tools(&mut registry);
        let blocked = if cfg!(windows) {
            "Write-Output blocked"
        } else {
            "printf blocked"
        };
        let tool_name = if cfg!(windows) { "PowerShell" } else { "Bash" };
        let context = TestContext {
            working_directory: std::env::temp_dir(),
            denied_commands: Arc::new(Mutex::new(vec![blocked.to_string()])),
        };

        let result = registry
            .execute(tool_name, json!({ "command": blocked }), &context)
            .await;

        match result.content {
            ToolResultContent::Text(text) => assert!(text.contains("blocked command"), "{text}"),
            other => panic!("expected text result, got {other:?}"),
        }
        assert!(result.is_error);
    }
}
