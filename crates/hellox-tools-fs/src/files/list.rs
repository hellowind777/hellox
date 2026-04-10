use std::collections::VecDeque;
use std::fs;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::support::DEFAULT_LIST_LIMIT;
use crate::FsToolContext;
use hellox_tool_runtime::{display_path, LocalTool, LocalToolResult};

pub struct ListFilesTool;

#[async_trait]
impl<C> LocalTool<C> for ListFilesTool
where
    C: FsToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "ListFiles".to_string(),
            description: Some("List files and directories in the workspace".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to inspect. Defaults to the current workspace." },
                    "recursive": { "type": "boolean", "description": "Whether to recurse into subdirectories." },
                    "max_entries": { "type": "integer", "description": "Maximum number of entries to return." }
                }
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let raw_path = input.get("path").and_then(Value::as_str).unwrap_or(".");
        let recursive = input
            .get("recursive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_entries = input
            .get("max_entries")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_LIST_LIMIT as u64) as usize;

        let root = context.resolve_path(raw_path);
        let mut queue = VecDeque::from([root.clone()]);
        let mut lines = Vec::new();

        while let Some(current) = queue.pop_front() {
            for entry in fs::read_dir(&current)
                .with_context(|| format!("failed to list directory {}", current.display()))?
            {
                let entry = entry?;
                let path = entry.path();
                let metadata = entry.metadata()?;
                let label = display_path(context.working_directory(), &path);
                if metadata.is_dir() {
                    lines.push(format!("dir  {}", label));
                    if recursive {
                        queue.push_back(path);
                    }
                } else {
                    lines.push(format!("file {}", label));
                }

                if lines.len() >= max_entries {
                    lines.push(format!("... truncated at {max_entries} entries"));
                    return Ok(LocalToolResult::text(lines.join("\n")));
                }
            }
        }

        if lines.is_empty() {
            lines.push("(no entries)".to_string());
        }

        Ok(LocalToolResult::text(lines.join("\n")))
    }
}
