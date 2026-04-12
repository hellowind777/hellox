use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};

use crate::shared::{render_json, AgentRunRequest};
use crate::workflow_branching::{
    evaluate_step_condition, summarize_step_statuses, WorkflowStepState,
};
use crate::workflow_support::{
    parse_step_permission_mode, render_prompt_template, resolve_workflow_input, WorkflowToolContext,
};

/// Registers workflow-domain tools into a shared runtime registry.
pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: WorkflowToolContext + Send + Sync + 'static,
{
    registry.register(WorkflowTool);
}

pub struct WorkflowTool;

#[async_trait]
impl<C> LocalTool<C> for WorkflowTool
where
    C: WorkflowToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        crate::workflow_support::workflow_tool_definition()
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let resolved = resolve_workflow_input(&input, context)?;
        let continue_on_error = resolved.continue_on_error;
        let shared_context = resolved.shared_context;
        let mut overall_status = "completed".to_string();
        let mut outputs = Vec::with_capacity(resolved.steps.len());
        let mut history = Vec::with_capacity(resolved.steps.len());

        for (index, step) in resolved.steps.into_iter().enumerate() {
            let step_name = step
                .name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| format!("step-{}", index + 1));
            if let Some(reason) = evaluate_step_condition(step.when.as_ref(), &history)? {
                history.push(WorkflowStepState {
                    name: step_name.clone(),
                    status: "skipped".to_string(),
                    result_text: None,
                });
                outputs.push(json!({
                    "name": step_name,
                    "status": "skipped",
                    "reason": reason,
                }));
                continue;
            }

            let prompt = render_prompt_template(&step.prompt, shared_context.as_deref(), &history)?;
            let prompt = prompt.trim().to_string();
            if prompt.is_empty() {
                return Err(anyhow!(
                    "workflow step `{step_name}` prompt cannot be empty"
                ));
            }
            let permission_mode = parse_step_permission_mode(&step)?;

            match context
                .run_workflow_step(AgentRunRequest {
                    prompt,
                    model: step.model,
                    backend: step.backend,
                    isolation: None,
                    worktree_name: None,
                    worktree_base_ref: None,
                    permission_mode,
                    agent_name: Some(step_name.clone()),
                    pane_group: None,
                    layout_strategy: None,
                    layout_slot: None,
                    pane_anchor_target: None,
                    cwd: step.cwd,
                    session_id: step.session_id,
                    max_turns: step.max_turns.map(|value| value as usize).unwrap_or(8),
                    reuse_existing_worktree: false,
                    run_in_background: step.run_in_background.unwrap_or(false),
                    allow_interaction: false,
                })
                .await
            {
                Ok(result) => {
                    let step_status = result
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("completed")
                        .to_string();
                    if step_status == "running" && overall_status == "completed" {
                        overall_status = "running".to_string();
                    }
                    history.push(WorkflowStepState {
                        name: step_name.clone(),
                        status: step_status.clone(),
                        result_text: step_result_text(&result),
                    });
                    outputs.push(json!({
                        "name": step_name,
                        "status": step_status,
                        "result": result,
                    }));
                }
                Err(error) => {
                    overall_status = "failed".to_string();
                    history.push(WorkflowStepState {
                        name: step_name.clone(),
                        status: "failed".to_string(),
                        result_text: None,
                    });
                    outputs.push(json!({
                        "name": step_name,
                        "status": "failed",
                        "error": error.to_string(),
                    }));
                    if !continue_on_error {
                        break;
                    }
                }
            }
        }

        Ok(LocalToolResult::text(render_json(json!({
            "status": overall_status,
            "workflow_source": resolved.source,
            "summary": summarize_step_statuses(&history),
            "steps": outputs,
        }))?))
    }
}

fn step_result_text(result: &Value) -> Option<String> {
    result
        .get("result")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use hellox_gateway_api::ToolResultContent;

    use super::*;

    #[derive(Default)]
    struct TestContext {
        working_directory: PathBuf,
        requests: Mutex<Vec<AgentRunRequest>>,
        responses: Mutex<Vec<std::result::Result<Value, String>>>,
    }

    #[async_trait]
    impl WorkflowToolContext for TestContext {
        fn working_directory(&self) -> &Path {
            &self.working_directory
        }

        fn resolve_path(&self, raw: &str) -> PathBuf {
            self.working_directory.join(raw)
        }

        async fn run_workflow_step(&self, request: AgentRunRequest) -> Result<Value> {
            self.requests.lock().expect("lock requests").push(request);
            let mut responses = self.responses.lock().expect("lock responses");
            let next = responses.remove(0);
            next.map_err(|error| anyhow!(error))
        }
    }

    #[tokio::test]
    async fn workflow_tool_renders_previous_result_into_follow_up_prompt() {
        let context = TestContext {
            working_directory: PathBuf::from("D:/workspace"),
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(vec![
                Ok(json!({ "status": "completed", "result": "alpha" })),
                Ok(json!({ "status": "completed", "result": "beta" })),
            ]),
        };

        let result = WorkflowTool
            .call(
                json!({
                    "steps": [
                        { "name": "collect", "prompt": "collect facts" },
                        { "name": "summarize", "prompt": "summarize {{workflow.previous_result}}" }
                    ]
                }),
                &context,
            )
            .await
            .expect("workflow call");

        let text = match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse workflow result");
        assert_eq!(value["status"].as_str(), Some("completed"));
        assert_eq!(value["summary"]["completed_steps"].as_u64(), Some(2));

        let requests = context.requests.lock().expect("lock requests");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].prompt, "collect facts");
        assert_eq!(requests[1].prompt, "summarize alpha");
    }

    #[tokio::test]
    async fn workflow_tool_skips_unmatched_branch_without_invoking_runner() {
        let context = TestContext {
            working_directory: PathBuf::from("D:/workspace"),
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(vec![Ok(json!({
                "status": "completed",
                "result": "ready to ship"
            }))]),
        };

        let result = WorkflowTool
            .call(
                json!({
                    "steps": [
                        { "name": "review", "prompt": "review changes" },
                        {
                            "name": "release",
                            "prompt": "ship it",
                            "when": { "previous_status": "failed" }
                        }
                    ]
                }),
                &context,
            )
            .await
            .expect("workflow call");

        let text = match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse workflow result");
        assert_eq!(value["summary"]["completed_steps"].as_u64(), Some(1));
        assert_eq!(value["summary"]["skipped_steps"].as_u64(), Some(1));
        assert_eq!(value["steps"][1]["status"].as_str(), Some("skipped"));
        assert!(value["steps"][1]["reason"]
            .as_str()
            .expect("skip reason")
            .contains("condition not met"));

        let requests = context.requests.lock().expect("lock requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].prompt, "review changes");
    }
}
