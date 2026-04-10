use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use hellox_gateway_api::{
    ContentBlock, DocumentSource, ImageSource, Message, MessageContent, MessageRole,
};
use hellox_tools_ui::{load_brief, BriefRecord};

use super::AgentSession;

pub(super) fn workspace_brief_section(record: &BriefRecord) -> String {
    let message = record.message.trim();
    if message.is_empty() {
        return String::new();
    }

    let mut lines = vec!["# Workspace brief".to_string(), message.to_string()];

    if let Some(status) = record
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(String::new());
        lines.push(format!("Status: {status}"));
    }

    if record.attachments.is_empty() {
        return lines.join("\n");
    }

    lines.push(String::new());
    lines.push("Attachments:".to_string());
    for attachment in &record.attachments {
        let label = attachment
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!(" ({value})"))
            .unwrap_or_default();
        lines.push(format!("- {}{label}", attachment.path));
    }

    lines.push(String::new());
    lines.push("Use `Read` to inspect attachments as needed.".to_string());

    lines.join("\n")
}

impl AgentSession {
    pub(super) async fn maybe_inject_brief_attachments(&mut self) -> Result<()> {
        if has_brief_attachment_message(&self.messages) {
            return Ok(());
        }

        let record = match load_brief(&self.context.working_directory) {
            Ok(Some(record)) => record,
            Ok(None) => return Ok(()),
            Err(error) => {
                eprintln!("Warning: failed to load workspace brief attachments: {error}");
                return Ok(());
            }
        };

        if record.attachments.is_empty() {
            return Ok(());
        }

        let mut blocks = vec![ContentBlock::Text {
            text: "Workspace brief attachments:".to_string(),
        }];
        let mut seen_paths: HashSet<String> = HashSet::new();
        let mut failures = Vec::new();

        for attachment in &record.attachments {
            let raw_path = attachment.path.trim();
            if raw_path.is_empty() {
                continue;
            }
            if !seen_paths.insert(raw_path.to_string()) {
                continue;
            }

            let resolved = resolve_brief_attachment_path(&self.context.working_directory, raw_path);
            if !resolved.exists() {
                failures.push(format!(
                    "attachment `{raw_path}` does not exist at {}",
                    resolved.display()
                ));
                continue;
            }
            if resolved.is_dir() {
                failures.push(format!(
                    "attachment `{raw_path}` resolves to a directory at {}",
                    resolved.display()
                ));
                continue;
            }

            let mime_type = infer_brief_attachment_mime_type(&resolved);
            let uploaded = match self
                .client
                .upload_file_path(&resolved, Some("user_data"), Some(mime_type))
                .await
            {
                Ok(meta) => meta,
                Err(error) => {
                    failures.push(format!("failed to upload attachment `{raw_path}`: {error}"));
                    continue;
                }
            };

            let label = attachment
                .label
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            let label_suffix = label
                .as_deref()
                .map(|value| format!(" ({value})"))
                .unwrap_or_default();

            blocks.push(ContentBlock::Text {
                text: format!("Attachment: {raw_path}{label_suffix}"),
            });

            if mime_type.starts_with("image/") {
                blocks.push(ContentBlock::Image {
                    source: ImageSource::File {
                        file_id: uploaded.id,
                    },
                });
            } else {
                let title = label.or_else(|| {
                    resolved
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                });
                blocks.push(ContentBlock::Document {
                    source: DocumentSource::File {
                        file_id: uploaded.id,
                    },
                    title,
                    context: Some(raw_path.to_string()),
                    citations: None,
                });
            }
        }

        if blocks.len() == 1 {
            if failures.is_empty() {
                return Ok(());
            }
            return Err(anyhow!(
                "brief attachments were configured, but none could be uploaded:\n- {}",
                failures.join("\n- ")
            ));
        }

        if !failures.is_empty() {
            eprintln!(
                "Warning: some brief attachments were skipped:\n- {}",
                failures.join("\n- ")
            );
        }

        self.messages.push(Message {
            role: MessageRole::User,
            content: MessageContent::Blocks(blocks),
        });
        self.persist()?;
        Ok(())
    }
}

fn has_brief_attachment_message(messages: &[Message]) -> bool {
    messages.iter().any(|message| {
        if !matches!(message.role, MessageRole::User) {
            return false;
        }
        let MessageContent::Blocks(blocks) = &message.content else {
            return false;
        };
        let Some(ContentBlock::Text { text }) = blocks.first() else {
            return false;
        };
        text == "Workspace brief attachments:"
    })
}

fn resolve_brief_attachment_path(root: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    }
}

fn infer_brief_attachment_mime_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "text/plain",
    }
}
