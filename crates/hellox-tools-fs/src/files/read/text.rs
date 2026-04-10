use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_gateway_api::ContentBlock;

use crate::support::truncate_for_output;

pub(super) const DEFAULT_TEXT_READ_LIMIT: usize = 2000;
const MAX_TEXT_READ_LINES: usize = 2000;

pub(super) fn read_text_blocks(
    path: &Path,
    bytes: &[u8],
    offset: usize,
    limit: usize,
) -> Result<Vec<ContentBlock>> {
    if bytes.contains(&0) {
        return Err(anyhow!(
            "Read only supports UTF-8 text files, images, PDFs, and notebooks; `{}` looks like a binary file",
            path.display()
        ));
    }

    let text = String::from_utf8_lossy(bytes).into_owned();
    let lines = text.lines().collect::<Vec<_>>();
    let total_lines = lines.len();
    let limit = usize::min(limit, MAX_TEXT_READ_LINES);

    let mut rendered = Vec::new();
    rendered.push(format!(
        "file: {}",
        path.display().to_string().replace('\\', "/")
    ));
    rendered.push("type: text/plain".to_string());
    rendered.push(format!("size_bytes: {}", bytes.len()));
    rendered.push(format!("offset: {offset}"));
    rendered.push(format!("limit: {limit}"));

    if total_lines == 0 {
        rendered.push("lines: 0".to_string());
        return Ok(vec![ContentBlock::Text {
            text: rendered.join("\n"),
        }]);
    }

    if offset > total_lines {
        rendered.push(format!("lines: {total_lines}"));
        rendered.push(String::new());
        rendered.push("(offset beyond end-of-file)".to_string());
        return Ok(vec![ContentBlock::Text {
            text: rendered.join("\n"),
        }]);
    }

    let start_index = offset.saturating_sub(1);
    let end_index = usize::min(total_lines, start_index.saturating_add(limit));
    rendered.push(format!(
        "lines: {}-{} of {}",
        start_index + 1,
        end_index,
        total_lines
    ));
    rendered.push(String::new());

    for (relative_index, line) in lines[start_index..end_index].iter().enumerate() {
        let line_number = start_index + relative_index + 1;
        rendered.push(format!("{line_number}: {line}"));
    }

    if end_index < total_lines {
        rendered.push(format!("... truncated at {limit} lines"));
    }

    Ok(vec![ContentBlock::Text {
        text: truncate_for_output(rendered.join("\n")),
    }])
}
