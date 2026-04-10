use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{LocalTool, LocalToolResult};
use serde_json::{json, Value};

use crate::shared::{optional_string, render_json};

/// Placeholder tool reserved for future remote-control / trigger integrations.
///
/// The local-first build intentionally does not provide a backing remote hub, so
/// this tool returns a `not_supported` error unless a future implementation is
/// wired in.
pub struct RemoteTriggerTool;

#[async_trait]
impl<C> LocalTool<C> for RemoteTriggerTool
where
    C: Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "RemoteTrigger".to_string(),
            description: Some(
                "Manage remote triggers (reserved for optional remote-control extensions)."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "description": "Trigger action (e.g. list/create/delete)." },
                    "trigger_id": { "type": "string" },
                    "payload": { "type": "object" }
                },
                "required": ["action"]
            }),
        }
    }

    async fn call(&self, input: Value, _context: &C) -> Result<LocalToolResult> {
        let action = optional_string(&input, "action").ok_or_else(|| {
            anyhow!("missing required string field `action` (must be a non-empty string)")
        })?;
        let trigger_id = optional_string(&input, "trigger_id");

        Ok(LocalToolResult::error(render_json(json!({
            "status": "not_supported",
            "action": action,
            "trigger_id": trigger_id,
            "message": "Remote triggers are not available in local-first mode. Enable a remote hub/transport before wiring this tool.",
        }))?))
    }
}
