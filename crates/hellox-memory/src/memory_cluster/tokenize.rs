use std::collections::HashMap;

use crate::memory_extract::parse_memory_sections;

pub(super) fn summary_first_line(markdown: &str) -> String {
    let lines = markdown.lines().collect::<Vec<_>>();
    let Some(summary_heading_index) = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("## Summary"))
    else {
        return fallback_preview(markdown);
    };

    for line in lines.iter().skip(summary_heading_index + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("## ") {
            break;
        }
        return collapse_preview(trimmed);
    }

    fallback_preview(markdown)
}

pub(super) fn token_counts(
    markdown: &str,
    preview: &str,
    max_tokens: usize,
) -> HashMap<String, u32> {
    let sections = parse_memory_sections(markdown);
    let mut input = String::new();
    input.push_str(preview);
    for fragment in sections
        .key_points
        .iter()
        .chain(sections.pending_work.iter())
        .chain(sections.risks.iter())
        .chain(sections.recent_artifacts.iter())
    {
        input.push('\n');
        input.push_str(fragment);
    }

    let mut counts: HashMap<String, u32> = HashMap::new();
    for token in tokenize(&input) {
        if token.len() < 3 {
            continue;
        }
        if is_stopword(&token) {
            continue;
        }
        if let Some(count) = counts.get_mut(&token) {
            *count = count.saturating_add(1);
            continue;
        }
        if counts.len() >= max_tokens {
            continue;
        }
        counts.insert(token, 1);
    }
    counts
}

fn fallback_preview(markdown: &str) -> String {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') || trimmed.starts_with("- ") {
            continue;
        }
        return collapse_preview(trimmed);
    }
    "(empty)".to_string()
}

fn collapse_preview(line: &str) -> String {
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 96 {
        return collapsed;
    }
    let truncated = collapsed.chars().take(93).collect::<String>();
    format!("{truncated}...")
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in text.to_ascii_lowercase().split_whitespace() {
        let trimmed = raw.trim_matches(|ch: char| {
            !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.'))
        });
        if trimmed.is_empty() {
            continue;
        }
        tokens.push(trimmed.to_string());
    }
    tokens
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "the"
            | "and"
            | "for"
            | "with"
            | "from"
            | "that"
            | "this"
            | "into"
            | "over"
            | "under"
            | "your"
            | "you"
            | "are"
            | "was"
            | "were"
            | "have"
            | "has"
            | "had"
            | "will"
            | "can"
            | "cannot"
            | "can't"
            | "still"
            | "need"
            | "needs"
            | "needed"
            | "memory"
            | "hellox"
    )
}
