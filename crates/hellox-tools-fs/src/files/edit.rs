use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::support::{read_text_file, write_text_file};
use crate::FsToolContext;
use hellox_tool_runtime::{display_path, LocalTool, LocalToolResult};

use super::diff::build_patch;

pub struct EditFileTool;

#[async_trait]
impl<C> LocalTool<C> for EditFileTool
where
    C: FsToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Edit".to_string(),
            description: Some(
                "Edit a UTF-8 text file with a targeted string replacement".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "File path to edit." },
                    "path": { "type": "string", "description": "Alias for `file_path`." },
                    "old_string": { "type": "string", "description": "Existing text to replace. Must match exactly." },
                    "new_string": { "type": "string", "description": "Replacement text." },
                    "old_text": { "type": "string", "description": "Alias for `old_string`." },
                    "new_text": { "type": "string", "description": "Alias for `new_string`." },
                    "replace_all": { "type": "boolean", "description": "Replace all matches instead of requiring a unique match." }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let raw_path = input
            .get("file_path")
            .and_then(Value::as_str)
            .or_else(|| input.get("path").and_then(Value::as_str))
            .ok_or_else(|| anyhow!("missing required string field `file_path` (or `path`)"))?;
        let old_text = input
            .get("old_text")
            .and_then(Value::as_str)
            .or_else(|| input.get("old_string").and_then(Value::as_str))
            .ok_or_else(|| anyhow!("missing required string field `old_string` (or `old_text`)"))?;
        let new_text = input
            .get("new_text")
            .and_then(Value::as_str)
            .or_else(|| input.get("new_string").and_then(Value::as_str))
            .ok_or_else(|| anyhow!("missing required string field `new_string` (or `new_text`)"))?;
        let replace_all = input
            .get("replace_all")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if old_text.is_empty() {
            return Err(anyhow!("old_text must not be empty"));
        }

        let path = context.resolve_path(raw_path);
        let label = display_path(context.working_directory(), &path);
        let content = read_text_file(&path)?;
        let match_count = content.matches(old_text).count();
        if match_count == 0 {
            return Err(anyhow!("old_text was not found in {label}"));
        }
        if match_count > 1 && !replace_all {
            return Err(anyhow!(
                "old_text matched {match_count} times in {label}; set replace_all=true or provide a more specific match"
            ));
        }

        let updated = if replace_all {
            content.replace(old_text, new_text)
        } else {
            content.replacen(old_text, new_text, 1)
        };

        context.ensure_write_allowed(&path).await?;
        write_text_file(&path, &updated)?;

        let replacement_count = if replace_all { match_count } else { 1 };
        let patch = build_patch(&content, &updated, &label);
        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "file_path": label,
                "replacements": replacement_count,
                "replace_all": replace_all,
                "structured_patch": patch.hunks,
                "unified_diff": patch.unified_diff,
            }),
        )?))
    }
}
