use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use axum::extract::Multipart;
use axum::Json;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use hellox_gateway_api::{
    AnthropicCompatRequest, ContentBlock, DocumentSource, ImageSource, MessageContent,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::error::GatewayHttpError;

pub(crate) async fn files_upload(
    mut multipart: Multipart,
) -> Result<Json<Value>, GatewayHttpError> {
    let file_id = format!("file_{}", Uuid::new_v4().simple());
    let mut purpose: Option<String> = None;
    let mut stored_dir: Option<PathBuf> = None;
    let mut filename: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let mut bytes: u64 = 0;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| GatewayHttpError::bad_request(err.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "purpose" {
            let text = field
                .text()
                .await
                .map_err(|err| GatewayHttpError::bad_request(err.to_string()))?;
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                purpose = Some(trimmed.to_string());
            }
            continue;
        }

        if name != "file" {
            continue;
        }
        if stored_dir.is_some() {
            return Err(GatewayHttpError::bad_request(
                "Only one `file` field is supported.".to_string(),
            ));
        }

        let original = field.file_name().unwrap_or("upload.bin");
        let safe = sanitize_filename(original);
        filename = Some(safe);
        mime_type = field.content_type().map(|item| item.to_string());

        let dir = gateway_files_root().join(&file_id);
        tokio::fs::create_dir_all(&dir).await.map_err(|err| {
            GatewayHttpError::internal(format!("failed to create upload dir: {err}"))
        })?;

        let content_path = dir.join("content");
        let mut output = tokio::fs::File::create(&content_path)
            .await
            .map_err(|err| {
                GatewayHttpError::internal(format!("failed to create uploaded file: {err}"))
            })?;

        let mut field = field;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|err| GatewayHttpError::bad_request(err.to_string()))?
        {
            bytes = bytes.saturating_add(chunk.len() as u64);
            output.write_all(&chunk).await.map_err(|err| {
                GatewayHttpError::internal(format!("failed to write uploaded file: {err}"))
            })?;
        }
        output.flush().await.map_err(|err| {
            GatewayHttpError::internal(format!("failed to flush uploaded file: {err}"))
        })?;

        stored_dir = Some(dir);
    }

    let Some(dir) = stored_dir else {
        return Err(GatewayHttpError::bad_request(
            "Missing multipart field `file`.".to_string(),
        ));
    };

    let meta = StoredGatewayFileMeta {
        id: file_id,
        purpose,
        filename: filename.unwrap_or_else(|| "upload.bin".to_string()),
        mime_type: mime_type.clone(),
        content_type: mime_type,
        size_bytes: bytes,
        bytes,
        created_at: unix_timestamp(),
        downloadable: false,
    };

    let meta_path = dir.join("meta.json");
    let raw = serde_json::to_vec_pretty(&meta).map_err(|err| {
        GatewayHttpError::internal(format!("failed to serialize file meta: {err}"))
    })?;
    tokio::fs::write(&meta_path, raw)
        .await
        .map_err(|err| GatewayHttpError::internal(format!("failed to persist file meta: {err}")))?;

    Ok(Json(serde_json::to_value(&meta).map_err(|err| {
        GatewayHttpError::internal(format!("failed to render file meta response: {err}"))
    })?))
}

pub(crate) fn materialize_local_file_references(
    request: AnthropicCompatRequest,
    allow_unresolved_file_source: bool,
) -> Result<AnthropicCompatRequest, GatewayHttpError> {
    materialize_local_file_references_in(
        &gateway_files_root(),
        request,
        allow_unresolved_file_source,
    )
}

fn materialize_local_file_references_in(
    files_root: &Path,
    request: AnthropicCompatRequest,
    allow_unresolved_file_source: bool,
) -> Result<AnthropicCompatRequest, GatewayHttpError> {
    let system = match request.system {
        Some(hellox_gateway_api::SystemPrompt::Blocks(blocks)) => {
            Some(hellox_gateway_api::SystemPrompt::Blocks(
                materialize_blocks(files_root, blocks, allow_unresolved_file_source)?,
            ))
        }
        other => other,
    };

    let mut messages = Vec::new();
    for mut message in request.messages {
        message.content =
            materialize_message_content(files_root, message.content, allow_unresolved_file_source)?;
        messages.push(message);
    }

    Ok(AnthropicCompatRequest {
        system,
        messages,
        ..request
    })
}

#[derive(Debug, Clone)]
pub(crate) struct StoredGatewayFile {
    pub meta: StoredGatewayFileMeta,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredGatewayFileMeta {
    pub id: String,
    #[serde(default)]
    pub purpose: Option<String>,
    pub filename: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub downloadable: bool,
}

impl StoredGatewayFileMeta {
    fn effective_mime_type(&self) -> Option<&str> {
        self.mime_type
            .as_deref()
            .or(self.content_type.as_deref())
            .filter(|value| !value.trim().is_empty())
    }
}

fn materialize_message_content(
    files_root: &Path,
    content: MessageContent,
    allow_unresolved_file_source: bool,
) -> Result<MessageContent, GatewayHttpError> {
    match content {
        MessageContent::Blocks(blocks) => Ok(MessageContent::Blocks(materialize_blocks(
            files_root,
            blocks,
            allow_unresolved_file_source,
        )?)),
        other => Ok(other),
    }
}

fn materialize_blocks(
    files_root: &Path,
    blocks: Vec<ContentBlock>,
    allow_unresolved_file_source: bool,
) -> Result<Vec<ContentBlock>, GatewayHttpError> {
    let mut output = Vec::new();
    for block in blocks {
        output.push(materialize_block(
            files_root,
            block,
            allow_unresolved_file_source,
        )?);
    }
    Ok(output)
}

fn materialize_block(
    files_root: &Path,
    block: ContentBlock,
    allow_unresolved_file_source: bool,
) -> Result<ContentBlock, GatewayHttpError> {
    match block {
        ContentBlock::Image { source } => Ok(ContentBlock::Image {
            source: materialize_image_source(files_root, source, allow_unresolved_file_source)?,
        }),
        ContentBlock::Document {
            source,
            title,
            context,
            citations,
        } => Ok(ContentBlock::Document {
            source: materialize_document_source(files_root, source, allow_unresolved_file_source)?,
            title,
            context,
            citations,
        }),
        ContentBlock::ToolResult {
            tool_use_id,
            content: hellox_gateway_api::ToolResultContent::Blocks(blocks),
            is_error,
        } => Ok(ContentBlock::ToolResult {
            tool_use_id,
            content: hellox_gateway_api::ToolResultContent::Blocks(materialize_blocks(
                files_root,
                blocks,
                allow_unresolved_file_source,
            )?),
            is_error,
        }),
        other => Ok(other),
    }
}

fn materialize_image_source(
    files_root: &Path,
    source: ImageSource,
    allow_unresolved_file_source: bool,
) -> Result<ImageSource, GatewayHttpError> {
    let ImageSource::File { file_id } = source else {
        return Ok(source);
    };

    let Some(stored) = load_gateway_file_from_root(files_root, &file_id)
        .map_err(|err| GatewayHttpError::internal(err.to_string()))?
    else {
        return if allow_unresolved_file_source {
            Ok(ImageSource::File { file_id })
        } else {
            Err(GatewayHttpError::bad_request(format!(
                "Unknown local gateway file_id `{file_id}`."
            )))
        };
    };

    let mime_type = stored
        .meta
        .effective_mime_type()
        .unwrap_or("application/octet-stream");
    if !mime_type.starts_with("image/") {
        return Err(GatewayHttpError::bad_request(format!(
            "File `{file_id}` is `{mime_type}` and cannot be used as an image block."
        )));
    }

    Ok(ImageSource::Base64 {
        media_type: mime_type.to_string(),
        data: BASE64_STANDARD.encode(stored.bytes),
    })
}

fn materialize_document_source(
    files_root: &Path,
    source: DocumentSource,
    allow_unresolved_file_source: bool,
) -> Result<DocumentSource, GatewayHttpError> {
    let DocumentSource::File { file_id } = source else {
        return Ok(source);
    };

    let Some(stored) = load_gateway_file_from_root(files_root, &file_id)
        .map_err(|err| GatewayHttpError::internal(err.to_string()))?
    else {
        return if allow_unresolved_file_source {
            Ok(DocumentSource::File { file_id })
        } else {
            Err(GatewayHttpError::bad_request(format!(
                "Unknown local gateway file_id `{file_id}`."
            )))
        };
    };

    let mime_type = stored
        .meta
        .effective_mime_type()
        .unwrap_or("application/octet-stream");
    match mime_type {
        "application/pdf" => Ok(DocumentSource::Base64 {
            media_type: mime_type.to_string(),
            data: BASE64_STANDARD.encode(stored.bytes),
        }),
        "text/plain" => {
            let text = String::from_utf8(stored.bytes).map_err(|_| {
                GatewayHttpError::bad_request(format!(
                    "File `{file_id}` is not valid UTF-8 text/plain content."
                ))
            })?;
            Ok(DocumentSource::Text {
                media_type: mime_type.to_string(),
                data: text,
            })
        }
        other => Err(GatewayHttpError::bad_request(format!(
            "File `{file_id}` is `{other}`. Document blocks currently support local `application/pdf` and `text/plain` uploads."
        ))),
    }
}

fn load_gateway_file_from_root(
    files_root: &Path,
    file_id: &str,
) -> Result<Option<StoredGatewayFile>> {
    let dir = files_root.join(file_id);
    if !dir.exists() {
        return Ok(None);
    }

    let meta_path = dir.join("meta.json");
    let content_path = dir.join("content");
    let raw = std::fs::read(&meta_path).with_context(|| {
        format!(
            "failed to read gateway file metadata {}",
            meta_path.display()
        )
    })?;
    let meta = serde_json::from_slice::<StoredGatewayFileMeta>(&raw).with_context(|| {
        format!(
            "failed to parse gateway file metadata {}",
            meta_path.display()
        )
    })?;
    let bytes = std::fs::read(&content_path).with_context(|| {
        format!(
            "failed to read gateway file content {}",
            content_path.display()
        )
    })?;

    Ok(Some(StoredGatewayFile { meta, bytes }))
}

fn gateway_files_root() -> PathBuf {
    gateway_files_root_from(&hellox_config::config_root())
}

fn gateway_files_root_from(config_root: &Path) -> PathBuf {
    config_root.join("gateway").join("files")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sanitize_filename(filename: &str) -> String {
    let trimmed = filename.trim();
    let candidate = Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upload.bin");

    let mut sanitized = String::with_capacity(candidate.len());
    for ch in candidate.chars() {
        if matches!(
            ch,
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'
        ) {
            sanitized.push('_');
        } else {
            sanitized.push(ch);
        }
    }

    if sanitized.trim().is_empty() {
        "upload.bin".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_gateway_api::{
        AnthropicCompatRequest, ContentBlock, DocumentSource, ImageSource, Message, MessageContent,
        MessageRole,
    };

    use super::{
        gateway_files_root_from, materialize_local_file_references_in, StoredGatewayFileMeta,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-gateway-files-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_local_file(root: &PathBuf, file_id: &str, mime_type: &str, body: &[u8]) {
        let files_root = gateway_files_root_from(root);
        let file_root = files_root.join(file_id);
        fs::create_dir_all(&file_root).expect("create file root");
        fs::write(file_root.join("content"), body).expect("write content");
        let meta = StoredGatewayFileMeta {
            id: file_id.to_string(),
            purpose: Some("user_data".to_string()),
            filename: "sample".to_string(),
            mime_type: Some(mime_type.to_string()),
            content_type: Some(mime_type.to_string()),
            size_bytes: body.len() as u64,
            bytes: body.len() as u64,
            created_at: 1,
            downloadable: false,
        };
        fs::write(
            file_root.join("meta.json"),
            serde_json::to_vec_pretty(&meta).expect("serialize meta"),
        )
        .expect("write meta");
    }

    #[test]
    fn materializes_text_document_file_reference() {
        let root = temp_root();
        write_local_file(
            &root,
            "file_local_text",
            "text/plain",
            b"hello from document",
        );

        let request = AnthropicCompatRequest {
            model: "sonnet".to_string(),
            system: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Blocks(vec![ContentBlock::Document {
                    source: DocumentSource::File {
                        file_id: "file_local_text".to_string(),
                    },
                    title: Some("Notes".to_string()),
                    context: None,
                    citations: None,
                }]),
            }],
            tools: Vec::new(),
            tool_choice: None,
            max_tokens: Some(256),
            temperature: None,
            top_p: None,
            metadata: None,
            thinking: None,
            stream: Some(false),
        };

        let materialized =
            materialize_local_file_references_in(&gateway_files_root_from(&root), request, false)
                .expect("materialize request");

        match &materialized.messages[0].content {
            MessageContent::Blocks(blocks) => match &blocks[0] {
                ContentBlock::Document {
                    source: DocumentSource::Text { media_type, data },
                    title,
                    ..
                } => {
                    assert_eq!(media_type, "text/plain");
                    assert_eq!(data, "hello from document");
                    assert_eq!(title.as_deref(), Some("Notes"));
                }
                other => panic!("unexpected block: {other:?}"),
            },
            other => panic!("unexpected content: {other:?}"),
        }
    }

    #[test]
    fn materializes_image_file_reference_to_base64() {
        let root = temp_root();
        write_local_file(&root, "file_local_image", "image/png", b"\x89PNG\r\n\x1a\n");

        let request = AnthropicCompatRequest {
            model: "sonnet".to_string(),
            system: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Blocks(vec![ContentBlock::Image {
                    source: ImageSource::File {
                        file_id: "file_local_image".to_string(),
                    },
                }]),
            }],
            tools: Vec::new(),
            tool_choice: None,
            max_tokens: Some(256),
            temperature: None,
            top_p: None,
            metadata: None,
            thinking: None,
            stream: Some(false),
        };

        let materialized =
            materialize_local_file_references_in(&gateway_files_root_from(&root), request, false)
                .expect("materialize request");

        match &materialized.messages[0].content {
            MessageContent::Blocks(blocks) => match &blocks[0] {
                ContentBlock::Image {
                    source: ImageSource::Base64 { media_type, data },
                } => {
                    assert_eq!(media_type, "image/png");
                    assert!(!data.is_empty());
                }
                other => panic!("unexpected block: {other:?}"),
            },
            other => panic!("unexpected content: {other:?}"),
        }
    }
}
