use std::path::Path;

use anyhow::{Context, Result};
use hellox_gateway_api::ContentBlock;
use serde_json::Value;

use crate::support::truncate_for_output;

pub(super) const DEFAULT_NOTEBOOK_CELL_LIMIT: usize = 24;
const MAX_NOTEBOOK_CELL_LIMIT: usize = 100;

pub(super) fn read_notebook_blocks(
    path: &Path,
    bytes: &[u8],
    offset: usize,
    limit: usize,
) -> Result<Vec<ContentBlock>> {
    let limit = usize::min(limit, MAX_NOTEBOOK_CELL_LIMIT);
    let offset = usize::max(1, offset);

    let notebook = serde_json::from_slice::<Value>(bytes)
        .with_context(|| format!("failed to parse notebook {}", path.display()))?;
    let cells = notebook
        .get("cells")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let kernelspec = notebook
        .get("metadata")
        .and_then(|value| value.get("kernelspec"))
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");
    let language = notebook
        .get("metadata")
        .and_then(|value| value.get("language_info"))
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");

    let mut code_cells = 0usize;
    let mut markdown_cells = 0usize;
    for cell in &cells {
        match cell.get("cell_type").and_then(Value::as_str).unwrap_or("") {
            "code" => code_cells += 1,
            "markdown" => markdown_cells += 1,
            _ => {}
        }
    }

    let mut lines = vec![
        format!("file: {}", path.display().to_string().replace('\\', "/")),
        "type: application/x-ipynb+json".to_string(),
        format!("size_bytes: {}", bytes.len()),
        format!("cells: {}", cells.len()),
        format!("code_cells: {code_cells}"),
        format!("markdown_cells: {markdown_cells}"),
        format!("language: {language}"),
        format!("kernelspec: {kernelspec}"),
        format!("offset: {offset}"),
        format!("limit: {limit}"),
    ];

    if let Some(first_markdown) = cells.iter().find(|cell| {
        cell.get("cell_type")
            .and_then(Value::as_str)
            .map(|value| value == "markdown")
            .unwrap_or(false)
    }) {
        let preview = join_notebook_source(first_markdown.get("source"));
        let preview = truncate_notebook_preview(&preview);
        if !preview.is_empty() {
            lines.push(format!("preview: {preview}"));
        }
    }

    let mut blocks = vec![ContentBlock::Text {
        text: lines.join("\n"),
    }];

    if cells.is_empty() {
        return Ok(blocks);
    }
    if offset > cells.len() {
        blocks.push(ContentBlock::Text {
            text: "(offset beyond end-of-notebook)".to_string(),
        });
        return Ok(blocks);
    }

    let start_index = offset - 1;
    let end_index = usize::min(cells.len(), start_index.saturating_add(limit));
    for (relative_index, cell) in cells[start_index..end_index].iter().enumerate() {
        let cell_number = start_index + relative_index + 1;
        let cell_type = cell
            .get("cell_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let source = join_notebook_source(cell.get("source"));
        let rendered = if source.trim().is_empty() {
            format!("[cell {cell_number}] {cell_type}\n\n(empty)")
        } else {
            format!("[cell {cell_number}] {cell_type}\n\n{source}")
        };
        blocks.push(ContentBlock::Text {
            text: truncate_for_output(rendered),
        });
    }

    Ok(blocks)
}

fn join_notebook_source(source: Option<&Value>) -> String {
    match source {
        Some(Value::Array(lines)) => lines
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(""),
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

fn truncate_notebook_preview(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 80 {
        return collapsed;
    }

    let truncated = collapsed.chars().take(77).collect::<String>();
    format!("{truncated}...")
}
