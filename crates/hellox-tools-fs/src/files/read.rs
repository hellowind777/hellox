mod image;
mod notebook;
mod pdf;
mod text;

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_gateway_api::ToolResultContent;
use serde_json::{json, Value};

use crate::metadata::{sniff_image, sniff_pdf};
use crate::FsToolContext;
use hellox_tool_runtime::{LocalTool, LocalToolResult};

use image::read_image_blocks;
use notebook::{read_notebook_blocks, DEFAULT_NOTEBOOK_CELL_LIMIT};
use pdf::{read_pdf_blocks, DEFAULT_PDF_PAGE_LIMIT};
use text::{read_text_blocks, DEFAULT_TEXT_READ_LIMIT};

pub struct ReadFileTool;

#[async_trait]
impl<C> LocalTool<C> for ReadFileTool
where
    C: FsToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "Read".to_string(),
            description: Some(
                "Read a local file from disk. Supports text (line ranges), images, PDFs, and Jupyter notebooks."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "File path to read." },
                    "path": { "type": "string", "description": "Alias for `file_path`." },
                    "offset": { "type": "integer", "minimum": 1, "description": "1-based start line/page/cell offset. Defaults to 1." },
                    "limit": { "type": "integer", "minimum": 1, "description": "Maximum lines/pages/cells to return. Text defaults to 2000 lines." }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let raw_path = input
            .get("file_path")
            .and_then(Value::as_str)
            .or_else(|| input.get("path").and_then(Value::as_str))
            .ok_or_else(|| anyhow!("missing required string field `file_path` (or `path`)"))?;
        let offset = parse_optional_positive_usize(input.get("offset"), "offset")?.unwrap_or(1);
        let limit = parse_optional_positive_usize(input.get("limit"), "limit")?;

        let path = context.resolve_path(raw_path);
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read file {}", path.display()))?;

        let blocks = if is_notebook_path(&path) {
            read_notebook_blocks(
                &path,
                &bytes,
                offset,
                limit.unwrap_or(DEFAULT_NOTEBOOK_CELL_LIMIT),
            )?
        } else if let Some(pdf) = sniff_pdf(&bytes) {
            read_pdf_blocks(
                &path,
                &bytes,
                pdf,
                offset,
                limit.unwrap_or(DEFAULT_PDF_PAGE_LIMIT),
            )?
        } else if let Some(image) = sniff_image(&bytes) {
            read_image_blocks(&path, &bytes, image)?
        } else {
            read_text_blocks(
                &path,
                &bytes,
                offset,
                limit.unwrap_or(DEFAULT_TEXT_READ_LIMIT),
            )?
        };

        Ok(LocalToolResult {
            content: ToolResultContent::Blocks(blocks),
            is_error: false,
        })
    }
}

fn parse_optional_nonnegative_usize(value: Option<&Value>, name: &str) -> Result<Option<usize>> {
    match value {
        None => Ok(None),
        Some(Value::Number(number)) => {
            if let Some(value) = number.as_u64() {
                Ok(Some(value as usize))
            } else if let Some(value) = number.as_i64() {
                if value < 0 {
                    Err(anyhow!("{name} must be >= 0"))
                } else {
                    Ok(Some(value as usize))
                }
            } else {
                Err(anyhow!("{name} must be an integer"))
            }
        }
        Some(_) => Err(anyhow!("{name} must be an integer")),
    }
}

fn parse_optional_positive_usize(value: Option<&Value>, name: &str) -> Result<Option<usize>> {
    let Some(value) = parse_optional_nonnegative_usize(value, name)? else {
        return Ok(None);
    };
    if value == 0 {
        return Err(anyhow!("{name} must be >= 1"));
    }
    Ok(Some(value))
}

fn is_notebook_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("ipynb"))
        .unwrap_or(false)
}
