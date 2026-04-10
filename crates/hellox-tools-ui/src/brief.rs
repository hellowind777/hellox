use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::UiToolContext;
use hellox_tool_runtime::{display_path, required_string, LocalTool, LocalToolResult};

pub struct BriefTool;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BriefRecord {
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<BriefAttachment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BriefAttachment {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum BriefAttachmentInput {
    Path(String),
    Detailed {
        path: String,
        #[serde(default)]
        label: Option<String>,
    },
}

#[async_trait]
impl<C> LocalTool<C> for BriefTool
where
    C: UiToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "SendUserMessage".to_string(),
            description: Some(
                "Store a structured local brief for the user and return the normalized message."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" },
                    "attachments": {
                        "type": "array",
                        "items": {
                            "oneOf": [
                                { "type": "string" },
                                {
                                    "type": "object",
                                    "properties": {
                                        "path": { "type": "string" },
                                        "label": { "type": "string" }
                                    },
                                    "required": ["path"]
                                }
                            ]
                        }
                    },
                    "status": { "type": "string" }
                },
                "required": ["message"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let message = required_string(&input, "message")?.trim().to_string();
        if message.is_empty() {
            return Err(anyhow!("brief message cannot be empty"));
        }

        let attachments = parse_attachments(input.get("attachments"), context.working_directory())?;
        let status = input
            .get("status")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let record = BriefRecord {
            message,
            attachments,
            status: status.clone(),
            updated_at: unix_timestamp(),
        };
        let path = brief_file_path(context.working_directory());

        context.ensure_write_allowed(&path).await?;
        write_brief(&path, &record)?;

        Ok(LocalToolResult::text(
            serde_json::to_string_pretty(&json!({
                "message": record.message,
                "attachments": record.attachments,
                "status": status,
                "path": path.display().to_string().replace('\\', "/"),
            }))
            .context("failed to serialize brief result")?,
        ))
    }
}

fn parse_attachments(value: Option<&Value>, root: &Path) -> Result<Vec<BriefAttachment>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let inputs = serde_json::from_value::<Vec<BriefAttachmentInput>>(value.clone())
        .context("failed to parse attachments")?;
    inputs
        .into_iter()
        .map(|item| match item {
            BriefAttachmentInput::Path(path) => normalize_attachment(root, &path, None),
            BriefAttachmentInput::Detailed { path, label } => {
                normalize_attachment(root, &path, label)
            }
        })
        .collect()
}

fn normalize_attachment(
    root: &Path,
    raw_path: &str,
    label: Option<String>,
) -> Result<BriefAttachment> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("attachment paths cannot be empty"));
    }

    let resolved = PathBuf::from(trimmed);
    let path = if resolved.is_absolute() {
        resolved
    } else {
        root.join(resolved)
    };
    let normalized = if path.starts_with(root) {
        display_path(root, &path)
    } else {
        path.display().to_string().replace('\\', "/")
    };

    Ok(BriefAttachment {
        path: normalized,
        label: label
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    })
}

fn brief_file_path(root: &Path) -> PathBuf {
    root.join(".hellox").join("brief.json")
}

pub fn load_brief(root: &Path) -> Result<Option<BriefRecord>> {
    let path = brief_file_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read brief file {}", path.display()))?;
    let record = serde_json::from_str::<BriefRecord>(&raw)
        .with_context(|| format!("failed to parse brief file {}", path.display()))?;
    Ok(Some(record))
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

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
