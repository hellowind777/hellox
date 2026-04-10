use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Map, Value};

use crate::support::write_text_file;
use crate::FsToolContext;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult};

pub struct NotebookEditTool;

#[async_trait]
impl<C> LocalTool<C> for NotebookEditTool
where
    C: FsToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "NotebookEdit".to_string(),
            description: Some("Edit or insert cells in a local Jupyter notebook.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "notebook_path": { "type": "string", "description": "Path to the .ipynb file." },
                    "new_source": { "type": "string", "description": "Replacement or inserted cell source." },
                    "cell_number": { "type": "integer", "minimum": 1, "description": "1-based target cell number." },
                    "cell_type": {
                        "type": "string",
                        "enum": ["code", "markdown", "raw"],
                        "description": "Cell type for inserted cells, or when replacing and changing the target type."
                    },
                    "edit_mode": {
                        "type": "string",
                        "enum": ["replace", "insert_before", "insert_after", "append"],
                        "description": "How to apply the change. Defaults to replace when cell_number is set, otherwise append."
                    }
                },
                "required": ["notebook_path", "new_source"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let raw_path = required_string(&input, "notebook_path")?;
        let new_source = required_string(&input, "new_source")?;
        let path = context.resolve_path(raw_path);
        ensure_notebook_path(&path)?;

        let edit_mode = input
            .get("edit_mode")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                if input.get("cell_number").is_some() {
                    "replace"
                } else {
                    "append"
                }
            });
        let cell_number = parse_cell_number(input.get("cell_number"))?;
        let requested_type = input.get("cell_type").and_then(Value::as_str);

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read notebook {}", path.display()))?;
        let mut notebook = serde_json::from_str::<Value>(&raw)
            .with_context(|| format!("failed to parse notebook {}", path.display()))?;
        let cells = notebook
            .get_mut("cells")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| anyhow!("notebook `{}` is missing a `cells` array", path.display()))?;

        let result = match edit_mode {
            "replace" => replace_cell(cells, cell_number, requested_type, new_source)?,
            "insert_before" => insert_cell(cells, cell_number, requested_type, new_source, true)?,
            "insert_after" => insert_cell(cells, cell_number, requested_type, new_source, false)?,
            "append" => append_cell(cells, requested_type, new_source)?,
            _ => {
                return Err(anyhow!(
                    "unsupported edit_mode `{edit_mode}`; expected replace, insert_before, insert_after, or append"
                ));
            }
        };
        let total_cells = cells.len();

        context.ensure_write_allowed(&path).await?;
        let serialized =
            serde_json::to_string_pretty(&notebook).context("failed to serialize notebook")?;
        write_text_file(&path, &format!("{serialized}\n"))?;

        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "notebook_path": path.display().to_string().replace('\\', "/"),
                "edit_mode": edit_mode,
                "cell_number": result.cell_number,
                "cell_type": result.cell_type,
                "total_cells": total_cells,
            }),
        )?))
    }
}

struct NotebookEditResult {
    cell_number: usize,
    cell_type: String,
}

fn ensure_notebook_path(path: &Path) -> Result<()> {
    if path.extension().and_then(|value| value.to_str()) != Some("ipynb") {
        return Err(anyhow!(
            "NotebookEdit only supports `.ipynb` files, got {}",
            path.display()
        ));
    }
    Ok(())
}

fn parse_cell_number(value: Option<&Value>) -> Result<Option<usize>> {
    match value {
        None => Ok(None),
        Some(Value::Number(number)) => match number.as_u64() {
            Some(0) => Err(anyhow!("cell_number must be greater than zero")),
            Some(number) => Ok(Some(number as usize)),
            None => Err(anyhow!("cell_number must be a positive integer")),
        },
        Some(_) => Err(anyhow!("cell_number must be a positive integer")),
    }
}

fn replace_cell(
    cells: &mut [Value],
    cell_number: Option<usize>,
    requested_type: Option<&str>,
    new_source: &str,
) -> Result<NotebookEditResult> {
    let cell_number = cell_number.ok_or_else(|| anyhow!("replace requires `cell_number`"))?;
    let index = checked_cell_index(cells.len(), cell_number)?;
    let existing_type = cells[index]
        .get("cell_type")
        .and_then(Value::as_str)
        .unwrap_or("code");
    let cell_type = validate_cell_type(requested_type.unwrap_or(existing_type))?.to_string();
    let metadata = cells[index]
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    cells[index] = build_cell(&cell_type, new_source, metadata);

    Ok(NotebookEditResult {
        cell_number,
        cell_type,
    })
}

fn insert_cell(
    cells: &mut Vec<Value>,
    cell_number: Option<usize>,
    requested_type: Option<&str>,
    new_source: &str,
    before: bool,
) -> Result<NotebookEditResult> {
    let cell_number =
        cell_number.ok_or_else(|| anyhow!("insert_before/insert_after require `cell_number`"))?;
    let index = checked_cell_index(cells.len(), cell_number)?;
    let insert_at = if before { index } else { index + 1 };
    let cell_type = validate_cell_type(requested_type.unwrap_or("code"))?.to_string();
    cells.insert(
        insert_at,
        build_cell(&cell_type, new_source, Value::Object(Map::new())),
    );

    Ok(NotebookEditResult {
        cell_number: insert_at + 1,
        cell_type,
    })
}

fn append_cell(
    cells: &mut Vec<Value>,
    requested_type: Option<&str>,
    new_source: &str,
) -> Result<NotebookEditResult> {
    let cell_type = validate_cell_type(requested_type.unwrap_or("code"))?.to_string();
    cells.push(build_cell(
        &cell_type,
        new_source,
        Value::Object(Map::new()),
    ));

    Ok(NotebookEditResult {
        cell_number: cells.len(),
        cell_type,
    })
}

fn checked_cell_index(total_cells: usize, cell_number: usize) -> Result<usize> {
    if total_cells == 0 {
        return Err(anyhow!("notebook does not contain any cells"));
    }
    if cell_number > total_cells {
        return Err(anyhow!(
            "cell_number {cell_number} is out of range for notebook with {total_cells} cells"
        ));
    }
    Ok(cell_number - 1)
}

fn validate_cell_type(cell_type: &str) -> Result<&str> {
    match cell_type {
        "code" | "markdown" | "raw" => Ok(cell_type),
        _ => Err(anyhow!(
            "unsupported cell_type `{cell_type}`; expected code, markdown, or raw"
        )),
    }
}

fn build_cell(cell_type: &str, source: &str, metadata: Value) -> Value {
    let source = Value::Array(source_to_lines(source));
    let mut cell = Map::new();
    cell.insert(
        "cell_type".to_string(),
        Value::String(cell_type.to_string()),
    );
    cell.insert("metadata".to_string(), metadata);
    cell.insert("source".to_string(), source);

    if cell_type == "code" {
        cell.insert("execution_count".to_string(), Value::Null);
        cell.insert("outputs".to_string(), Value::Array(Vec::new()));
    }

    Value::Object(cell)
}

fn source_to_lines(source: &str) -> Vec<Value> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut lines = source
        .split_inclusive('\n')
        .map(|line| Value::String(line.to_string()))
        .collect::<Vec<_>>();
    if !source.ends_with('\n') {
        if let Some(last) = lines.last_mut() {
            *last = Value::String(
                source
                    .rsplit_once('\n')
                    .map_or(source, |(_, tail)| tail)
                    .to_string(),
            );
        }
        if !source.contains('\n') {
            lines[0] = Value::String(source.to_string());
        }
    }
    lines
}
