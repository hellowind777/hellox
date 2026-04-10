use serde_json::{json, Value};
use similar::TextDiff;

pub(super) struct PatchResult {
    pub(super) hunks: Vec<Value>,
    pub(super) unified_diff: String,
}

pub(super) fn build_patch(old: &str, new: &str, header: &str) -> PatchResult {
    let header = header.replace('\\', "/");
    let diff = TextDiff::from_lines(old, new);
    let unified = diff
        .unified_diff()
        .context_radius(3)
        .header(&header, &header)
        .to_string();

    PatchResult {
        hunks: parse_unified_diff_hunks(&unified),
        unified_diff: unified,
    }
}

fn parse_unified_diff_hunks(unified: &str) -> Vec<Value> {
    let mut hunks = Vec::new();
    let mut current_header: Option<(usize, usize, usize, usize)> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in unified.lines() {
        if !line.starts_with("@@") {
            if current_header.is_some() {
                current_lines.push(line.to_string());
            }
            continue;
        }

        if let Some((old_start, old_lines, new_start, new_lines)) = current_header.take() {
            hunks.push(json!({
                "old_start": old_start,
                "old_lines": old_lines,
                "new_start": new_start,
                "new_lines": new_lines,
                "lines": current_lines,
            }));
            current_lines = Vec::new();
        }

        if let Some((old_start, old_len, new_start, new_len)) = parse_hunk_header(line) {
            current_header = Some((old_start, old_len, new_start, new_len));
        }
    }

    if let Some((old_start, old_lines, new_start, new_lines)) = current_header.take() {
        hunks.push(json!({
            "old_start": old_start,
            "old_lines": old_lines,
            "new_start": new_start,
            "new_lines": new_lines,
            "lines": current_lines,
        }));
    }

    hunks
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    // Expected format: @@ -old_start,old_len +new_start,new_len @@
    let mut trimmed = line.trim();
    trimmed = trimmed.strip_prefix("@@")?.trim();
    trimmed = trimmed.trim_end_matches("@@").trim();
    let mut parts = trimmed.split_whitespace();
    let old_part = parts.next()?;
    let new_part = parts.next()?;

    let (old_start, old_len) = parse_range(old_part)?;
    let (new_start, new_len) = parse_range(new_part)?;
    Some((old_start, old_len, new_start, new_len))
}

fn parse_range(part: &str) -> Option<(usize, usize)> {
    let part = part.strip_prefix('-').or_else(|| part.strip_prefix('+'))?;
    let mut items = part.split(',');
    let start = items.next()?.parse::<usize>().ok()?;
    let len = items
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);
    Some((start, len))
}
