use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};

use crate::shared::{optional_string, parse_permission_mode, render_json, AgentRunRequest};

#[async_trait]
pub trait AgentRuntimeToolContext {
    async fn run_agent_prompt(&self, request: AgentRunRequest) -> Result<Value>;
    async fn reconcile_team_runtime_for_session(&self, session_id: &str) -> Result<()>;
    fn agent_status_value(&self, session_id: &str) -> Result<Value>;
}

pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: AgentRuntimeToolContext + Send + Sync + 'static,
{
    registry.register(AgentTool);
    registry.register(AgentStatusTool);
    registry.register(AgentWaitTool);
}

pub struct AgentTool;
pub struct AgentStatusTool;
pub struct AgentWaitTool;

#[async_trait]
impl<C> LocalTool<C> for AgentTool
where
    C: AgentRuntimeToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Agent".to_string(),
            description: Some(
                "Run a nested local agent task and optionally keep it running as a background session."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" },
                    "model": { "type": "string" },
                    "backend": { "type": "string" },
                    "isolation": { "type": "string", "enum": ["worktree"] },
                    "worktree_name": { "type": "string" },
                    "worktree_base_ref": { "type": "string" },
                    "reuse_existing_worktree": { "type": "boolean" },
                    "permission_mode": { "type": "string" },
                    "cwd": { "type": "string" },
                    "session_id": { "type": "string" },
                    "max_turns": { "type": "integer", "minimum": 1, "maximum": 64 },
                    "run_in_background": { "type": "boolean" }
                },
                "required": ["prompt"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let prompt = required_string(&input, "prompt")?.trim().to_string();
        if prompt.is_empty() {
            return Err(anyhow!("agent prompt cannot be empty"));
        }

        let run_in_background = input
            .get("run_in_background")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let value = context
            .run_agent_prompt(AgentRunRequest {
                prompt,
                model: optional_string(&input, "model"),
                backend: optional_string(&input, "backend"),
                isolation: optional_string(&input, "isolation"),
                worktree_name: optional_string(&input, "worktree_name"),
                worktree_base_ref: optional_string(&input, "worktree_base_ref"),
                permission_mode: parse_permission_mode(&input, "permission_mode")?,
                agent_name: None,
                pane_group: None,
                layout_strategy: None,
                layout_slot: None,
                pane_anchor_target: None,
                cwd: optional_string(&input, "cwd"),
                session_id: optional_string(&input, "session_id"),
                max_turns: input
                    .get("max_turns")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(8),
                reuse_existing_worktree: input
                    .get("reuse_existing_worktree")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                run_in_background,
                allow_interaction: input
                    .get("run_in_background")
                    .and_then(Value::as_bool)
                    .is_none_or(|value| !value),
            })
            .await?;

        Ok(LocalToolResult::text(render_json(value)?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for AgentStatusTool
where
    C: AgentRuntimeToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "AgentStatus".to_string(),
            description: Some(
                "Inspect a local agent/background session by session id.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" }
                },
                "required": ["session_id"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let session_id = required_string(&input, "session_id")?;
        context
            .reconcile_team_runtime_for_session(session_id)
            .await?;
        Ok(LocalToolResult::text(render_json(
            context.agent_status_value(session_id)?,
        )?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for AgentWaitTool
where
    C: AgentRuntimeToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "AgentWait".to_string(),
            description: Some("Wait for a background local agent session to finish.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "timeout_ms": { "type": "integer", "minimum": 1 },
                    "poll_interval_ms": { "type": "integer", "minimum": 1 }
                },
                "required": ["session_id"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let session_id = required_string(&input, "session_id")?;
        let timeout_ms = input
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(30_000);
        let poll_interval_ms = input
            .get("poll_interval_ms")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        let started = SystemTime::now();

        loop {
            context
                .reconcile_team_runtime_for_session(session_id)
                .await?;
            let status = context.agent_status_value(session_id)?;
            if status.get("status").and_then(Value::as_str) != Some("running") {
                return Ok(LocalToolResult::text(render_json(status)?));
            }

            if started.elapsed().unwrap_or_default() >= Duration::from_millis(timeout_ms) {
                return Err(anyhow!(
                    "timed out waiting for background agent `{session_id}`"
                ));
            }

            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use hellox_gateway_api::ToolResultContent;

    use super::*;

    #[derive(Default)]
    struct TestContext {
        statuses: Mutex<Vec<Value>>,
        requests: Mutex<Vec<AgentRunRequest>>,
    }

    #[async_trait]
    impl AgentRuntimeToolContext for TestContext {
        async fn run_agent_prompt(&self, request: AgentRunRequest) -> Result<Value> {
            self.requests.lock().expect("lock requests").push(request);
            Ok(json!({"status": "completed", "result": "ok"}))
        }

        async fn reconcile_team_runtime_for_session(&self, _session_id: &str) -> Result<()> {
            Ok(())
        }

        fn agent_status_value(&self, _session_id: &str) -> Result<Value> {
            let mut statuses = self.statuses.lock().expect("lock statuses");
            if statuses.is_empty() {
                return Ok(json!({"status": "completed"}));
            }
            Ok(statuses.remove(0))
        }
    }

    #[tokio::test]
    async fn agent_tool_builds_agent_run_request() {
        let context = TestContext::default();
        let result = AgentTool
            .call(
                json!({
                    "prompt": "review this diff",
                    "backend": "detached_process",
                    "isolation": "worktree",
                    "worktree_name": "review-agent",
                    "reuse_existing_worktree": true,
                    "max_turns": 4,
                    "run_in_background": true
                }),
                &context,
            )
            .await
            .expect("agent call");

        match result.content {
            ToolResultContent::Text(text) => assert!(text.contains("\"completed\""), "{text}"),
            other => panic!("expected text result, got {other:?}"),
        }

        let requests = context.requests.lock().expect("lock requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].prompt, "review this diff");
        assert_eq!(requests[0].backend.as_deref(), Some("detached_process"));
        assert_eq!(requests[0].isolation.as_deref(), Some("worktree"));
        assert_eq!(requests[0].worktree_name.as_deref(), Some("review-agent"));
        assert!(requests[0].reuse_existing_worktree);
        assert!(requests[0].run_in_background);
    }

    #[tokio::test]
    async fn agent_wait_tool_polls_until_non_running_status() {
        let context = TestContext {
            statuses: Mutex::new(vec![
                json!({"status": "running"}),
                json!({"status": "completed"}),
            ]),
            requests: Mutex::new(Vec::new()),
        };

        let result = AgentWaitTool
            .call(
                json!({"session_id": "sess_1", "poll_interval_ms": 1}),
                &context,
            )
            .await
            .expect("wait call");

        match result.content {
            ToolResultContent::Text(text) => assert!(text.contains("\"completed\""), "{text}"),
            other => panic!("expected text result, got {other:?}"),
        }
    }
}
