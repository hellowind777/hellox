use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::UiToolContext;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult};

pub struct ToolSearchTool;

#[async_trait]
impl<C> LocalTool<C> for ToolSearchTool
where
    C: UiToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "ToolSearch".to_string(),
            description: Some("Search available local tools by name or description.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let query = required_string(&input, "query")?
            .trim()
            .to_ascii_lowercase();
        if query.is_empty() {
            return Err(anyhow!("tool search query cannot be empty"));
        }
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(20);

        let matches = context
            .available_tool_definitions()
            .into_iter()
            .filter(|tool| {
                tool.name.to_ascii_lowercase().contains(&query)
                    || tool
                        .description
                        .as_ref()
                        .is_some_and(|text| text.to_ascii_lowercase().contains(&query))
            })
            .take(limit)
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                })
            })
            .collect::<Vec<_>>();

        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "query": query,
                "matches": matches,
            }),
        )?))
    }
}
