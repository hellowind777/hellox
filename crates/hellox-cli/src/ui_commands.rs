use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli_ui_types::{BriefCommands, ToolsCommands};

pub(crate) fn handle_brief_command(command: BriefCommands) -> Result<()> {
    println!("{}", brief_command_text(command)?);
    Ok(())
}

pub(crate) fn handle_tools_command(command: ToolsCommands) -> Result<()> {
    println!("{}", tools_command_text(command)?);
    Ok(())
}

pub(crate) fn brief_command_text(command: BriefCommands) -> Result<String> {
    match command {
        BriefCommands::Show { cwd } => {
            let root = workspace_root(cwd)?;
            match load_brief(&root) {
                Ok(record) => Ok(format_brief(&root, &record)),
                Err(_) => Ok(format!(
                    "No brief file found at `{}`.",
                    normalize_path(&brief_file_path(&root))
                )),
            }
        }
        BriefCommands::Set {
            message,
            attachments,
            status,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let record = BriefRecord {
                message,
                attachments: attachments
                    .into_iter()
                    .map(|path| normalize_attachment(&root, &path))
                    .collect::<Result<Vec<_>>>()?,
                status: status
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                updated_at: unix_timestamp(),
            };
            let path = brief_file_path(&root);
            write_brief(&path, &record)?;
            Ok(format_brief(&root, &record))
        }
        BriefCommands::Clear { cwd } => {
            let root = workspace_root(cwd)?;
            let path = brief_file_path(&root);
            if !path.exists() {
                return Ok(format!(
                    "No brief file found at `{}`.",
                    normalize_path(&path)
                ));
            }
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove brief file {}", path.display()))?;
            Ok(format!("Removed brief file `{}`.", normalize_path(&path)))
        }
    }
}

pub(crate) fn tools_command_text(command: ToolsCommands) -> Result<String> {
    match command {
        ToolsCommands::Search { query, limit } => Ok(render_tool_search(&query, limit)),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BriefRecord {
    message: String,
    #[serde(default)]
    attachments: Vec<BriefAttachment>,
    #[serde(default)]
    status: Option<String>,
    updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BriefAttachment {
    path: String,
    #[serde(default)]
    label: Option<String>,
}

fn load_brief(root: &Path) -> Result<BriefRecord> {
    let path = brief_file_path(root);
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read brief file {}", path.display()))?;
    serde_json::from_str::<BriefRecord>(&raw)
        .with_context(|| format!("failed to parse brief file {}", path.display()))
}

fn write_brief(path: &Path, record: &BriefRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create brief directory {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(record).context("failed to serialize brief")?;
    fs::write(path, format!("{raw}\n"))
        .with_context(|| format!("failed to write brief file {}", path.display()))
}

fn format_brief(root: &Path, record: &BriefRecord) -> String {
    let mut lines = vec![
        format!("brief_path: {}", normalize_path(&brief_file_path(root))),
        format!("message: {}", record.message),
        format!("status: {}", record.status.as_deref().unwrap_or("(none)")),
        format!("updated_at: {}", record.updated_at),
    ];
    if record.attachments.is_empty() {
        lines.push("attachments: (none)".to_string());
    } else {
        lines.push("attachments:".to_string());
        for attachment in &record.attachments {
            lines.push(format!(
                "- {}{}",
                attachment.path,
                attachment
                    .label
                    .as_deref()
                    .map(|label| format!(" ({label})"))
                    .unwrap_or_default()
            ));
        }
    }
    lines.join("\n")
}

fn render_tool_search(query: &str, limit: usize) -> String {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return "Tool search query cannot be empty.".to_string();
    }

    let matches = hellox_agent::default_tool_registry()
        .definitions()
        .into_iter()
        .filter(|tool| {
            tool.name.to_ascii_lowercase().contains(&normalized_query)
                || tool
                    .description
                    .as_ref()
                    .is_some_and(|text| text.to_ascii_lowercase().contains(&normalized_query))
        })
        .take(limit)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return format!("No local tools matched `{query}`.");
    }

    let mut lines = vec![
        format!("query: {query}"),
        format!("matches: {}", matches.len()),
        "name\tdescription".to_string(),
    ];
    for tool in matches {
        lines.push(format!(
            "{}\t{}",
            tool.name,
            tool.description.unwrap_or_default()
        ));
    }
    lines.join("\n")
}

fn brief_file_path(root: &Path) -> PathBuf {
    root.join(".hellox").join("brief.json")
}

fn normalize_attachment(root: &Path, raw: &str) -> Result<BriefAttachment> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("brief attachment paths cannot be empty");
    }
    let resolved = PathBuf::from(trimmed);
    let path = if resolved.is_absolute() {
        resolved
    } else {
        root.join(resolved)
    };
    Ok(BriefAttachment {
        path: path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/"),
        label: None,
    })
}

fn workspace_root(value: Option<PathBuf>) -> Result<PathBuf> {
    Ok(match value {
        Some(path) => path,
        None => std::env::current_dir()?,
    })
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{brief_command_text, tools_command_text, BriefCommands, ToolsCommands};

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-ui-commands-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn tools_search_lists_matching_local_tools() {
        let text = tools_command_text(ToolsCommands::Search {
            query: "mcp".to_string(),
            limit: 5,
        })
        .expect("tool search");
        assert!(text.contains("query: mcp"));
        assert!(text.contains("mcp"));
    }

    #[test]
    fn brief_show_and_clear_roundtrip() {
        let root = temp_dir();
        let brief_root = root.join(".hellox");
        fs::create_dir_all(&brief_root).expect("create brief dir");
        fs::write(
            brief_root.join("brief.json"),
            r#"{
  "message": "Ship the current release review flow.",
  "attachments": [{ "path": "notes/review.md", "label": "review" }],
  "status": "in_progress",
  "updated_at": 42
}"#,
        )
        .expect("write brief");

        let shown = brief_command_text(BriefCommands::Show {
            cwd: Some(root.clone()),
        })
        .expect("show brief");
        assert!(shown.contains("Ship the current release review flow."));
        assert!(shown.contains("notes/review.md (review)"));

        let cleared = brief_command_text(BriefCommands::Clear {
            cwd: Some(root.clone()),
        })
        .expect("clear brief");
        assert!(cleared.contains("Removed brief file"));
        assert!(!brief_root.join("brief.json").exists());
    }

    #[test]
    fn brief_set_writes_normalized_attachment_paths() {
        let root = temp_dir();
        let shown = brief_command_text(BriefCommands::Set {
            message: "Need review on the release checklist.".to_string(),
            attachments: vec!["notes/review.md".to_string()],
            status: Some("in_progress".to_string()),
            cwd: Some(root.clone()),
        })
        .expect("set brief");

        assert!(shown.contains("Need review on the release checklist."));
        assert!(shown.contains("notes/review.md"));
        let stored =
            fs::read_to_string(root.join(".hellox").join("brief.json")).expect("read brief");
        assert!(stored.contains("\"status\": \"in_progress\""));
    }
}
