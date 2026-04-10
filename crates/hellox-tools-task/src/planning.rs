use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::storage::render_json;
use crate::{PlanItem, TaskToolContext};
use hellox_tool_runtime::{LocalTool, LocalToolResult, ToolRegistry};

pub(crate) fn register_planning_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TaskToolContext + Send + Sync + 'static,
{
    registry.register(EnterPlanModeTool);
    registry.register(ExitPlanModeTool);
}

pub struct EnterPlanModeTool;
pub struct ExitPlanModeTool;

#[async_trait]
impl<C> LocalTool<C> for EnterPlanModeTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "EnterPlanMode".to_string(),
            description: Some("Mark the current session as being in plan mode.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, _input: Value, context: &C) -> Result<LocalToolResult> {
        let planning = context.enter_plan_mode()?;
        Ok(LocalToolResult::text(render_json(json!({
            "message": "Plan mode is now active.",
            "planning": planning,
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for ExitPlanModeTool
where
    C: TaskToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "ExitPlanMode".to_string(),
            description: Some("Store the accepted plan and exit plan mode.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "plan": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "step": { "type": "string" },
                                "status": { "type": "string" }
                            },
                            "required": ["step", "status"]
                        }
                    },
                    "allowed_prompts": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["plan"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let plan_value = input
            .get("plan")
            .cloned()
            .ok_or_else(|| anyhow!("missing required array field `plan`"))?;
        let plan = serde_json::from_value::<Vec<PlanItem>>(plan_value)
            .context("failed to parse `plan` items")?;
        let mut normalized_plan = Vec::with_capacity(plan.len());
        for item in plan {
            let item = item.normalized();
            item.validate()?;
            normalized_plan.push(item);
        }
        let allowed_prompts = input
            .get("allowed_prompts")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(ToString::to_string)
                            .ok_or_else(|| anyhow!("allowed_prompts must contain strings"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();

        let planning = context.exit_plan_mode(normalized_plan, allowed_prompts)?;
        Ok(LocalToolResult::text(render_json(json!({
            "message": "Plan mode is now inactive.",
            "planning": planning,
        }))?))
    }
}
