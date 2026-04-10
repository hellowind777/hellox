use std::collections::{HashMap, HashSet};

use hellox_gateway_api::{ContentBlock, Message, MessageContent, ToolResultContent};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ExtractedMemorySections {
    pub(super) key_points: Vec<String>,
    pub(super) pending_work: Vec<String>,
    pub(super) risks: Vec<String>,
    pub(super) recent_artifacts: Vec<String>,
}

#[allow(dead_code)]
pub(super) fn render_extracted_memory_sections(
    summary: &str,
    transcript: Option<&[Message]>,
) -> Vec<String> {
    let sections = extract_memory_sections(summary, transcript);
    render_memory_sections(&sections)
}

pub(super) fn render_memory_sections(sections: &ExtractedMemorySections) -> Vec<String> {
    let mut lines = Vec::new();
    append_section(&mut lines, "Key Points", &sections.key_points);
    append_section(&mut lines, "Pending Work", &sections.pending_work);
    append_section(&mut lines, "Risks", &sections.risks);
    append_section(&mut lines, "Recent Artifacts", &sections.recent_artifacts);
    lines
}

pub(super) fn parse_memory_sections(markdown: &str) -> ExtractedMemorySections {
    let mut sections = ExtractedMemorySections::default();
    let mut current = MemorySection::Other;

    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(next) = memory_section_for_heading(trimmed) {
            current = next;
            continue;
        }
        if !trimmed.starts_with("- ") {
            continue;
        }

        let item = normalize_fragment(trimmed.trim_start_matches("- "));
        if item.is_empty() {
            continue;
        }

        match current {
            MemorySection::KeyPoints => sections.key_points.push(item),
            MemorySection::PendingWork => sections.pending_work.push(item),
            MemorySection::Risks => sections.risks.push(item),
            MemorySection::RecentArtifacts => sections.recent_artifacts.push(item),
            MemorySection::Other | MemorySection::Summary => {}
        }
    }

    sections
}

pub(super) fn merge_memory_sections(
    existing: ExtractedMemorySections,
    fresh: ExtractedMemorySections,
    limits: MemorySectionLimits,
) -> ExtractedMemorySections {
    ExtractedMemorySections {
        key_points: merge_section(&existing.key_points, &fresh.key_points, limits.key_points),
        pending_work: merge_section(
            &existing.pending_work,
            &fresh.pending_work,
            limits.pending_work,
        ),
        risks: merge_section(&existing.risks, &fresh.risks, limits.risks),
        recent_artifacts: merge_section(
            &existing.recent_artifacts,
            &fresh.recent_artifacts,
            limits.recent_artifacts,
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MemorySectionLimits {
    pub(super) key_points: usize,
    pub(super) pending_work: usize,
    pub(super) risks: usize,
    pub(super) recent_artifacts: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemorySection {
    Other,
    Summary,
    KeyPoints,
    PendingWork,
    Risks,
    RecentArtifacts,
}

fn memory_section_for_heading(line: &str) -> Option<MemorySection> {
    match line {
        "## Summary" => Some(MemorySection::Summary),
        "## Key Points" => Some(MemorySection::KeyPoints),
        "## Pending Work" => Some(MemorySection::PendingWork),
        "## Risks" => Some(MemorySection::Risks),
        "## Recent Artifacts" => Some(MemorySection::RecentArtifacts),
        _ => None,
    }
}

fn merge_section(existing: &[String], fresh: &[String], limit: usize) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }

    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for item in fresh.iter().chain(existing.iter()) {
        let normalized = normalize_fragment(item);
        if normalized.is_empty() || is_noise_fragment(&normalized) {
            continue;
        }

        let key = normalized.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }

        merged.push(normalized);
        if merged.len() >= limit {
            break;
        }
    }

    merged
}

pub(super) fn extract_memory_sections(
    summary: &str,
    transcript: Option<&[Message]>,
) -> ExtractedMemorySections {
    let mut fragments = candidate_fragments(summary);
    if let Some(transcript) = transcript {
        fragments.extend(candidate_fragments_from_transcript(transcript));
    }
    let mut seen = HashSet::new();
    let mut ranked = Vec::new();

    for fragment in fragments {
        let normalized = normalize_fragment(&fragment);
        if normalized.is_empty() || is_noise_fragment(&normalized) {
            continue;
        }

        let dedupe_key = normalized.to_ascii_lowercase();
        if !seen.insert(dedupe_key) {
            continue;
        }

        ranked.push((importance_score(&normalized), normalized));
    }

    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.len().cmp(&right.1.len()))
            .then_with(|| left.1.cmp(&right.1))
    });

    let ranked_fragments = ranked
        .into_iter()
        .map(|(_, fragment)| fragment)
        .collect::<Vec<_>>();
    ExtractedMemorySections {
        key_points: ranked_fragments.iter().take(3).cloned().collect(),
        pending_work: select_category(&ranked_fragments, looks_pending_work, 3),
        risks: select_category(&ranked_fragments, looks_risky, 3),
        recent_artifacts: select_category(&ranked_fragments, looks_like_artifact, 4),
    }
}

fn candidate_fragments_from_transcript(messages: &[Message]) -> Vec<String> {
    let start = messages.len().saturating_sub(24);
    let mut fragments = Vec::new();
    let mut tool_calls: HashMap<String, String> = HashMap::new();

    for message in &messages[start..] {
        fragments.extend(candidate_fragments(&extract_text_fragment(
            &message.content,
        )));

        if let MessageContent::Blocks(blocks) = &message.content {
            fragments.extend(candidate_fragments_from_blocks(blocks, &mut tool_calls));
        }
    }

    fragments
}

fn candidate_fragments_from_blocks(
    blocks: &[ContentBlock],
    tool_calls: &mut HashMap<String, String>,
) -> Vec<String> {
    let mut fragments = Vec::new();

    for block in blocks {
        match block {
            ContentBlock::Text { text } => {
                fragments.extend(candidate_fragments(&truncate_fragment(text, 800)));
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.insert(id.clone(), name.clone());
                fragments.push(format!("Tool call: `{name}`"));
                fragments.extend(extract_tool_inputs(name, input));
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let tool_name = tool_calls.get(tool_use_id).map(String::as_str);
                if *is_error {
                    fragments.push(match tool_name {
                        Some(tool_name) => {
                            format!("Tool `{tool_name}` result `{tool_use_id}` returned an error")
                        }
                        None => format!("Tool result `{tool_use_id}` returned an error"),
                    });
                }
                fragments.extend(extract_tool_result_fragments(tool_name, content, *is_error));
            }
            _ => {}
        }
    }

    fragments
}

fn extract_tool_inputs(name: &str, input: &Value) -> Vec<String> {
    let Some(object) = input.as_object() else {
        return Vec::new();
    };

    let mut fragments = Vec::new();
    let path = object
        .get("file_path")
        .and_then(|value| value.as_str())
        .or_else(|| object.get("path").and_then(|value| value.as_str()));
    if let Some(path) = path {
        fragments.push(format!(
            "Tool `{name}` path: `{}`",
            normalize_fragment(path)
        ));
    }
    if let Some(pattern) = object.get("pattern").and_then(|value| value.as_str()) {
        fragments.push(format!(
            "Tool `{name}` pattern: `{}`",
            normalize_fragment(pattern)
        ));
    }
    if let Some(command) = object.get("command").and_then(|value| value.as_str()) {
        if let Some(snippet) = sanitize_command_snippet(command) {
            fragments.push(format!(
                "Tool `{name}` command: `{}`",
                normalize_fragment(&snippet)
            ));
        }
    }
    if let Some(query) = object.get("query").and_then(|value| value.as_str()) {
        fragments.push(format!(
            "Tool `{name}` query: `{}`",
            normalize_fragment(query)
        ));
    }
    fragments
}

fn sanitize_command_snippet(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Avoid storing obvious secrets in memory (tokens, passwords, keys).
    let lower = trimmed.to_ascii_lowercase();
    if [
        "api_key",
        "apikey",
        "access_key",
        "secret",
        "token",
        "password",
        "authorization:",
        "bearer ",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return None;
    }

    Some(truncate_fragment(trimmed, 240))
}

fn extract_tool_result_fragments(
    tool_name: Option<&str>,
    content: &ToolResultContent,
    is_error: bool,
) -> Vec<String> {
    let Some(tool_name) = tool_name else {
        return Vec::new();
    };

    // File tools often return full file contents; rely on tool inputs (path/pattern)
    // rather than trying to mine large outputs for memory.
    if matches!(
        tool_name,
        "Read" | "Write" | "Edit" | "read_file" | "write_file" | "edit_file"
    ) {
        return Vec::new();
    }

    let text = extract_tool_result_text(content);
    if text.trim().is_empty() {
        return Vec::new();
    }

    let mut fragments = Vec::new();
    let mut seen_paths = HashSet::new();

    for line in text.lines().take(80) {
        let normalized = normalize_fragment(line);
        if normalized.is_empty() || is_noise_tool_output_line(&normalized) {
            continue;
        }

        if is_error || looks_risky(&normalized) || normalized.to_ascii_lowercase().contains("error")
        {
            fragments.push(format!("Tool `{tool_name}` output: {normalized}"));
        }

        for path in extract_artifact_tokens(&normalized) {
            let key = path.to_ascii_lowercase();
            if seen_paths.insert(key) {
                fragments.push(format!("`{path}`"));
            }
        }
    }

    fragments
}

fn extract_tool_result_text(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(text) => truncate_fragment(text, 1_600),
        ToolResultContent::Blocks(blocks) => {
            let mut combined = String::new();
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(text);
                }
            }
            truncate_fragment(&combined, 1_600)
        }
        ToolResultContent::Empty => String::new(),
    }
}

fn is_noise_tool_output_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("exit_code:")
        || lower == "stdout:"
        || lower == "stderr:"
        || lower.starts_with("directory:")
        || lower.starts_with("mode ")
        || lower.starts_with("----")
}

fn extract_artifact_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in line.split_whitespace() {
        let trimmed = raw.trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}'
            )
        });
        if trimmed.is_empty() {
            continue;
        }
        if looks_like_path_token(trimmed) {
            tokens.push(trimmed.replace('\\', "/"));
        }
    }
    tokens
}

fn looks_like_path_token(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    if token.contains('/') || token.contains('\\') {
        return true;
    }
    [
        ".rs", ".md", ".json", ".toml", ".yaml", ".yml", ".ts", ".tsx", ".js", ".txt",
    ]
    .iter()
    .any(|suffix| lower.contains(suffix))
}

fn extract_text_fragment(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => truncate_fragment(text, 800),
        MessageContent::Blocks(blocks) => {
            let mut combined = String::new();
            for block in blocks {
                if let ContentBlock::Text { text } = block {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(text);
                }
            }
            truncate_fragment(&combined, 800)
        }
        MessageContent::Empty => String::new(),
    }
}

fn truncate_fragment(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    value.chars().take(max_chars).collect::<String>()
}

fn append_section(lines: &mut Vec<String>, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }

    lines.push(String::new());
    lines.push(format!("## {title}"));
    lines.push(String::new());
    for item in items {
        lines.push(format!("- {item}"));
    }
}

fn select_category(
    ranked_fragments: &[String],
    predicate: fn(&str) -> bool,
    limit: usize,
) -> Vec<String> {
    ranked_fragments
        .iter()
        .filter(|fragment| predicate(fragment))
        .take(limit)
        .cloned()
        .collect()
}

fn candidate_fragments(summary: &str) -> Vec<String> {
    let mut fragments = Vec::new();
    for raw_line in summary.lines() {
        let trimmed = normalize_fragment(
            raw_line
                .trim_start_matches("- ")
                .trim_start_matches("* ")
                .trim_start_matches(char::is_whitespace),
        );
        if trimmed.is_empty() {
            continue;
        }

        for fragment in split_sentence_like_fragments(&trimmed) {
            let normalized = normalize_fragment(fragment);
            if !normalized.is_empty() {
                fragments.push(normalized);
            }
        }
    }
    fragments
}

fn split_sentence_like_fragments(input: &str) -> Vec<&str> {
    let mut fragments = Vec::new();
    let mut start = 0usize;
    let chars = input.char_indices().collect::<Vec<_>>();

    for (index, (offset, ch)) in chars.iter().enumerate() {
        let is_boundary = matches!(ch, '!' | '?' | ';')
            || (*ch == '.'
                && chars
                    .get(index + 1)
                    .map(|(_, next)| next.is_whitespace())
                    .unwrap_or(true));

        if !is_boundary {
            continue;
        }

        let end = *offset;
        if start < end {
            fragments.push(input[start..end].trim());
        }
        start = end + ch.len_utf8();
    }

    if start < input.len() {
        fragments.push(input[start..].trim());
    }

    fragments
}

fn normalize_fragment(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn importance_score(fragment: &str) -> usize {
    let word_count = fragment.split_whitespace().count();
    let mut score = 1;

    if (4..=28).contains(&word_count) {
        score += 2;
    }
    if looks_actionable(fragment) {
        score += 3;
    }
    if looks_risky(fragment) {
        score += 3;
    }
    if looks_like_artifact(fragment) {
        score += 2;
    }
    if looks_decision_like(fragment) {
        score += 2;
    }

    score
}

fn looks_actionable(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    [
        "next",
        "pending",
        "remaining",
        "still need",
        "need to",
        "follow up",
        "left to",
        "todo",
        "plan to",
        "continue",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn looks_pending_work(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    [
        "pending",
        "remaining",
        "still need",
        "need to",
        "next",
        "follow up",
        "left to",
        "todo",
        "plan to",
        "continue",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn looks_risky(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    [
        "error",
        "panic",
        "risk",
        "warning",
        "blocked",
        "blocker",
        "limitation",
        "cannot",
        "can't",
        "failed",
        "failure",
        "flaky",
        "host validation",
        "still missing",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn looks_decision_like(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    [
        "accepted",
        "decided",
        "decision",
        "current architecture",
        "keep",
        "using",
        "use ",
        "adopt",
        "chosen",
        "settled",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn looks_like_artifact(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    fragment.contains('`')
        || fragment.contains('/')
        || [
            ".rs", ".md", ".json", ".toml", ".yaml", ".yml", ".ts", ".tsx", ".js",
        ]
        .iter()
        .any(|suffix| lower.contains(suffix))
}

fn is_noise_fragment(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    lower.contains("conversation summary generated by hellox /compact")
        || lower.contains("compaction instructions:")
        || lower == "summary"
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use hellox_gateway_api::{Message, MessageContent, MessageRole};

    use super::render_extracted_memory_sections;

    fn shell_tool_name() -> &'static str {
        if cfg!(windows) {
            "PowerShell"
        } else {
            "Bash"
        }
    }

    #[test]
    fn renders_structured_sections_from_actionable_summary() {
        let lines = render_extracted_memory_sections(
            "Accepted architecture is Rust CLI. Still need to build the workflow panel. \
             Risk: tmux host validation remains pending. Updated `crates/hellox-cli/src/main.rs` \
             and docs/HELLOX_LOCAL_FEATURE_AUDIT.md.",
            None,
        );
        let rendered = lines.join("\n");

        assert!(rendered.contains("## Key Points"));
        assert!(rendered.contains("Accepted architecture is Rust CLI"));
        assert!(rendered.contains("## Pending Work"));
        assert!(rendered.contains("Still need to build the workflow panel"));
        assert!(rendered.contains("## Risks"));
        assert!(rendered.contains("Risk: tmux host validation remains pending"));
        assert!(rendered.contains("## Recent Artifacts"));
        assert!(rendered.contains("crates/hellox-cli/src/main"));
    }

    #[test]
    fn ignores_compact_boilerplate_noise() {
        let lines = render_extracted_memory_sections(
            "Conversation summary generated by hellox /compact. \
             Compaction instructions: Preserve active work. Accepted implementation uses Rust.",
            None,
        );
        let rendered = lines.join("\n");

        assert!(!rendered.contains("Conversation summary generated by hellox /compact"));
        assert!(rendered.contains("Accepted implementation uses Rust"));
    }

    #[test]
    fn includes_tool_paths_from_transcript_tool_use_blocks() {
        let transcript = vec![Message {
            role: MessageRole::Assistant,
            content: MessageContent::Blocks(vec![hellox_gateway_api::ContentBlock::ToolUse {
                id: String::from("tool-1"),
                name: String::from("Read"),
                input: json!({
                    "file_path": "crates/hellox-cli/src/main.rs"
                }),
            }]),
        }];

        let lines = render_extracted_memory_sections("", Some(&transcript));
        let rendered = lines.join("\n");

        assert!(rendered.contains("crates/hellox-cli/src/main.rs"));
        assert!(rendered.contains("## Recent Artifacts"));
    }

    #[test]
    fn extracts_paths_and_errors_from_tool_results() {
        let transcript = vec![Message {
            role: MessageRole::Assistant,
            content: MessageContent::Blocks(vec![
                hellox_gateway_api::ContentBlock::ToolUse {
                    id: String::from("tool-1"),
                    name: String::from(shell_tool_name()),
                    input: json!({
                        "command": "cargo test -p hellox-cli"
                    }),
                },
                hellox_gateway_api::ContentBlock::ToolResult {
                    tool_use_id: String::from("tool-1"),
                    is_error: true,
                    content: hellox_gateway_api::ToolResultContent::Text(
                        "exit_code: 1\nstdout:\nCompiling hellox-cli\nstderr:\nerror: failed to read file crates/hellox-cli/src/main.rs".to_string(),
                    ),
                },
            ]),
        }];

        let lines = render_extracted_memory_sections("", Some(&transcript));
        let rendered = lines.join("\n");

        assert!(rendered.contains("## Risks"));
        assert!(rendered.contains(&format!(
            "Tool `{}` output: error: failed to read file",
            shell_tool_name()
        )));
        assert!(rendered.contains("## Recent Artifacts"));
        assert!(rendered.contains("crates/hellox-cli/src/main.rs"));
    }

    #[test]
    fn sentence_splitter_preserves_file_extensions() {
        let lines = render_extracted_memory_sections(
            "Updated crates/hellox-cli/src/main.rs. Still need to wire the workflow panel.",
            None,
        );
        let rendered = lines.join("\n");

        assert!(rendered.contains("crates/hellox-cli/src/main.rs"));
        assert!(rendered.contains("Still need to wire the workflow panel"));
    }
}
