use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{LocalTool, LocalToolResult};
use serde_json::{json, Value};

use crate::shared::render_json;

/// Pauses the local runtime for a short duration. Useful for pacing loops or tick-based flows.
pub struct SleepTool;

#[async_trait]
impl<C> LocalTool<C> for SleepTool
where
    C: Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Sleep".to_string(),
            description: Some("Pause the local agent for a short amount of time.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "duration_ms": { "type": "integer", "minimum": 1, "maximum": 60000 }
                },
                "required": ["duration_ms"]
            }),
        }
    }

    async fn call(&self, input: Value, _context: &C) -> Result<LocalToolResult> {
        let duration_ms = input
            .get("duration_ms")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("missing required integer field `duration_ms`"))?;
        tokio::time::sleep(Duration::from_millis(duration_ms)).await;
        Ok(LocalToolResult::text(render_json(json!({
            "slept_ms": duration_ms,
        }))?))
    }
}
