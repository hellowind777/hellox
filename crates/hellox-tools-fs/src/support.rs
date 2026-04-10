use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use regex::{Regex, RegexBuilder};
use serde_json::Value;

pub(crate) const DEFAULT_LIST_LIMIT: usize = 200;
pub(crate) const DEFAULT_MATCH_LIMIT: usize = 100;
const MAX_READ_CHARS: usize = 200_000;

pub(crate) fn collect_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut paths = Vec::new();

    while let Some(current) = queue.pop_front() {
        let metadata = fs::metadata(&current)
            .with_context(|| format!("failed to inspect {}", current.display()))?;
        if metadata.is_file() {
            paths.push(current);
            continue;
        }

        for entry in fs::read_dir(&current)
            .with_context(|| format!("failed to list directory {}", current.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            paths.push(path.clone());
            if entry.metadata()?.is_dir() {
                queue.push_back(path);
            }
        }
    }

    Ok(paths)
}

pub(crate) fn parse_include_patterns(value: Option<&Value>) -> Result<Vec<Regex>> {
    let mut patterns = Vec::new();
    match value {
        None => return Ok(patterns),
        Some(Value::String(pattern)) => patterns.push(compile_glob_pattern(pattern)?),
        Some(Value::Array(items)) => {
            for item in items {
                let pattern = item
                    .as_str()
                    .ok_or_else(|| anyhow!("include patterns must be strings"))?;
                patterns.push(compile_glob_pattern(pattern)?);
            }
        }
        Some(_) => return Err(anyhow!("include must be a string or string array")),
    }
    Ok(patterns)
}

pub(crate) fn matches_include(patterns: &[Regex], root: &Path, path: &Path) -> bool {
    patterns.is_empty()
        || patterns
            .iter()
            .any(|pattern| matches_glob(pattern, root, path))
}

pub(crate) fn matches_glob(pattern: &Regex, root: &Path, path: &Path) -> bool {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    pattern.is_match(&relative)
}

pub(crate) fn compile_glob_pattern(pattern: &str) -> Result<Regex> {
    RegexBuilder::new(&glob_to_regex(pattern))
        .case_insensitive(cfg!(windows))
        .build()
        .with_context(|| format!("invalid glob pattern `{pattern}`"))
}

pub(crate) fn normalize_glob(pattern: &str) -> String {
    pattern.replace('\\', "/")
}

pub(crate) fn read_text_file(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read file {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub(crate) fn truncate_for_output(mut text: String) -> String {
    if text.len() > MAX_READ_CHARS {
        text.truncate(MAX_READ_CHARS);
        text.push_str("\n... truncated ...");
    }
    text
}

pub(crate) fn read_searchable_text(path: &Path) -> Result<Option<String>> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read file {}", path.display()))?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

pub(crate) fn write_text_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("failed to write file {}", path.display()))?;
    Ok(())
}

pub(crate) fn render_match_list(matches: Vec<String>, max_results: usize) -> String {
    if matches.is_empty() {
        return "(no matches)".to_string();
    }

    let total = matches.len();
    let mut lines = matches.into_iter().take(max_results).collect::<Vec<_>>();
    if total > max_results {
        lines.push(format!("... truncated at {max_results} matches"));
    }
    lines.join("\n")
}

pub(crate) fn render_context_lines(
    lines: &[&str],
    index: usize,
    context_lines: usize,
) -> Vec<String> {
    let start = index.saturating_sub(context_lines);
    let end = usize::min(lines.len(), index + context_lines + 1);
    let mut rendered = Vec::new();

    for (offset, line) in lines[start..end].iter().enumerate() {
        let line_number = start + offset + 1;
        if line_number == index + 1 {
            continue;
        }
        rendered.push(format!("  {}:{}", line_number, line));
    }

    rendered
}

fn glob_to_regex(pattern: &str) -> String {
    let normalized = normalize_glob(pattern);
    let chars = normalized.chars().collect::<Vec<_>>();
    let mut regex = String::from("^");
    let mut index = 0usize;

    while index < chars.len() {
        match chars[index] {
            '*' if chars.get(index + 1) == Some(&'*') => {
                if chars.get(index + 2) == Some(&'/') {
                    regex.push_str("(?:.*/)?");
                    index += 3;
                } else {
                    regex.push_str(".*");
                    index += 2;
                }
            }
            '*' => {
                regex.push_str("[^/]*");
                index += 1;
            }
            '?' => {
                regex.push_str("[^/]");
                index += 1;
            }
            ch if ".+()[]{}^$|\\".contains(ch) => {
                regex.push('\\');
                regex.push(ch);
                index += 1;
            }
            ch => {
                regex.push(ch);
                index += 1;
            }
        }
    }

    regex.push('$');
    regex
}
