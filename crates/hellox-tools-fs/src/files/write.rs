use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::support::{read_text_file, write_text_file};
use crate::FsToolContext;
use hellox_tool_runtime::{display_path, LocalTool, LocalToolResult};

use super::diff::build_patch;

pub struct WriteFileTool;

#[async_trait]
impl<C> LocalTool<C> for WriteFileTool
where
    C: FsToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Write".to_string(),
            description: Some(
                "Write a UTF-8 text file to disk, creating parent directories when needed"
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "File path to write." },
                    "path": { "type": "string", "description": "Alias for `file_path`." },
                    "content": { "type": "string", "description": "Full file content." }
                },
                "required": ["file_path", "content"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let raw_path = input
            .get("file_path")
            .and_then(Value::as_str)
            .or_else(|| input.get("path").and_then(Value::as_str))
            .ok_or_else(|| anyhow!("missing required string field `file_path` (or `path`)"))?;
        let content = input
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("missing required string field `content`"))?;
        let path = context.resolve_path(raw_path);
        let label = display_path(context.working_directory(), &path);
        let existed = path.exists();
        let original = if existed {
            Some(read_text_file(&path)?)
        } else {
            None
        };

        context.ensure_write_allowed(&path).await?;
        write_text_file(&path, content)?;

        let patch = build_patch(original.as_deref().unwrap_or(""), content, &label);
        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "type": if existed { "update" } else { "create" },
                "file_path": label,
                "bytes_written": content.as_bytes().len(),
                "structured_patch": patch.hunks,
                "unified_diff": patch.unified_diff,
            }),
        )?))
    }
}
