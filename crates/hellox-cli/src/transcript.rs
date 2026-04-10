use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use hellox_agent::{AgentSession, StoredSessionSnapshot};
use hellox_gateway_api::{
    ContentBlock, DocumentSource, ImageSource, MessageContent, MessageRole, ToolResultContent,
};

pub fn default_share_path(root: &Path, session_id: Option<&str>) -> PathBuf {
    let stem = session_id.unwrap_or("current-session");
    root.join(format!("{stem}-{}.md", unix_timestamp()))
}

pub fn export_session_markdown(session: &AgentSession, path: &Path) -> Result<()> {
    write_markdown(
        path,
        render_transcript_markdown(
            TranscriptMetadata {
                session_id: session.session_id().unwrap_or("(ephemeral)").to_string(),
                model: session.model().to_string(),
                permission_mode: session.permission_mode().to_string(),
                output_style: session.output_style_name().unwrap_or("(none)").to_string(),
                persona: session.persona_name().unwrap_or("(none)").to_string(),
                prompt_fragments: render_names(session.prompt_fragment_names()),
                working_directory: normalize_path(
                    &session.working_directory().display().to_string(),
                ),
                message_count: session.message_count(),
            },
            session.messages().iter().map(|message| TranscriptMessage {
                role: match message.role {
                    MessageRole::User => "User",
                    MessageRole::Assistant => "Assistant",
                },
                content: &message.content,
            }),
        ),
    )
}

pub fn export_stored_session_markdown(snapshot: &StoredSessionSnapshot, path: &Path) -> Result<()> {
    write_markdown(
        path,
        render_transcript_markdown(
            TranscriptMetadata {
                session_id: snapshot.session_id.clone(),
                model: snapshot.model.clone(),
                permission_mode: snapshot
                    .permission_mode
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "(from current config)".to_string()),
                output_style: snapshot
                    .output_style_name
                    .as_deref()
                    .unwrap_or("(none)")
                    .to_string(),
                persona: snapshot
                    .persona
                    .as_ref()
                    .map(|persona| persona.name.as_str())
                    .unwrap_or("(none)")
                    .to_string(),
                prompt_fragments: render_names(
                    &snapshot
                        .prompt_fragments
                        .iter()
                        .map(|fragment| fragment.name.clone())
                        .collect::<Vec<_>>(),
                ),
                working_directory: normalize_path(&snapshot.working_directory),
                message_count: snapshot.messages.len(),
            },
            snapshot.messages.iter().map(|message| TranscriptMessage {
                role: if message.role.eq_ignore_ascii_case("assistant") {
                    "Assistant"
                } else {
                    "User"
                },
                content: &message.content,
            }),
        ),
    )
}

struct TranscriptMetadata {
    session_id: String,
    model: String,
    permission_mode: String,
    output_style: String,
    persona: String,
    prompt_fragments: String,
    working_directory: String,
    message_count: usize,
}

struct TranscriptMessage<'a> {
    role: &'static str,
    content: &'a MessageContent,
}

fn write_markdown(path: &Path, markdown: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create share directory {}", parent.display()))?;
    }

    fs::write(path, markdown)
        .with_context(|| format!("failed to write shared transcript {}", path.display()))
}

fn render_transcript_markdown<'a>(
    metadata: TranscriptMetadata,
    messages: impl Iterator<Item = TranscriptMessage<'a>>,
) -> String {
    let messages = messages.collect::<Vec<_>>();
    let mut sections = vec![
        "# hellox transcript".to_string(),
        String::new(),
        format!("- session_id: {}", metadata.session_id),
        format!("- model: {}", metadata.model),
        format!("- permission_mode: {}", metadata.permission_mode),
        format!("- output_style: {}", metadata.output_style),
        format!("- persona: {}", metadata.persona),
        format!("- prompt_fragments: {}", metadata.prompt_fragments),
        format!("- working_directory: {}", metadata.working_directory),
        format!("- messages: {}", metadata.message_count),
        format!("- exported_at: {}", unix_timestamp()),
    ];

    if messages.is_empty() {
        sections.push(String::new());
        sections.push("_No transcript messages recorded yet._".to_string());
        return sections.join("\n");
    }

    for (index, message) in messages.iter().enumerate() {
        sections.push(String::new());
        sections.push(format!("## {} {}", index + 1, message.role));
        sections.push(String::new());
        sections.push(render_message_content(message.content));
    }

    sections.join("\n")
}

fn render_message_content(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Empty => "_Empty message_".to_string(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .map(render_content_block)
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

fn render_content_block(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text { text } => text.clone(),
        ContentBlock::Image { source } => {
            format!("_Image input_\n\n{}", render_image_source(source))
        }
        ContentBlock::Document {
            source,
            title,
            context,
            ..
        } => format!(
            "_Document input_\n\n{}",
            render_document_source(source, title.as_deref(), context.as_deref())
        ),
        ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {
            "_Thinking content omitted_".to_string()
        }
        ContentBlock::ToolUse { id, name, input } => format!(
            "_Tool use `{name}` ({id})_\n\n```json\n{}\n```",
            serde_json::to_string_pretty(input).unwrap_or_else(|_| "{}".to_string())
        ),
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => format!(
            "_Tool result for `{tool_use_id}`{}_\n\n{}",
            if *is_error { " (error)" } else { "" },
            render_tool_result_content(content)
        ),
    }
}

fn render_image_source(source: &ImageSource) -> String {
    match source {
        ImageSource::File { file_id } => format!("- source: file\n- file_id: {file_id}"),
        ImageSource::Url { url } => format!("- source: url\n- url: {url}"),
        ImageSource::Base64 { media_type, .. } => {
            format!("- source: base64\n- media_type: {media_type}")
        }
    }
}

fn render_document_source(
    source: &DocumentSource,
    title: Option<&str>,
    context: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    if let Some(title) = title.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("- title: {title}"));
    }
    if let Some(context) = context.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("- context: {context}"));
    }

    match source {
        DocumentSource::File { file_id } => {
            lines.push("- source: file".to_string());
            lines.push(format!("- file_id: {file_id}"));
        }
        DocumentSource::Url { url } => {
            lines.push("- source: url".to_string());
            lines.push(format!("- url: {url}"));
        }
        DocumentSource::Base64 { media_type, .. } => {
            lines.push("- source: base64".to_string());
            lines.push(format!("- media_type: {media_type}"));
        }
        DocumentSource::Text { media_type, data } => {
            lines.push("- source: text".to_string());
            lines.push(format!("- media_type: {media_type}"));
            lines.push(String::new());
            lines.push(data.clone());
        }
        DocumentSource::Content { content } => {
            lines.push("- source: content".to_string());
            lines.push(String::new());
            lines.push(
                content
                    .iter()
                    .map(render_content_block)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            );
        }
    }

    lines.join("\n")
}

fn render_tool_result_content(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(text) => text.clone(),
        ToolResultContent::Empty => "_Empty tool result_".to_string(),
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .map(render_content_block)
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn render_names(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    }
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
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_agent::{
        default_tool_registry, AgentOptions, AgentSession, GatewayClient, StoredSessionSnapshot,
    };

    use super::{default_share_path, export_session_markdown, export_stored_session_markdown};

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-transcript-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn session(root: &Path) -> AgentSession {
        AgentSession::create(
            GatewayClient::new("http://127.0.0.1:7821"),
            default_tool_registry(),
            root.join(".hellox").join("config.toml"),
            root.to_path_buf(),
            "powershell",
            AgentOptions::default(),
            hellox_config::PermissionMode::AcceptEdits,
            None,
            None,
            false,
            Some(String::from("share-me")),
        )
    }

    fn stored_snapshot() -> StoredSessionSnapshot {
        StoredSessionSnapshot {
            session_id: String::from("persisted-session"),
            model: String::from("opus"),
            permission_mode: Some(hellox_config::PermissionMode::AcceptEdits),
            output_style_name: None,
            output_style: None,
            persona: None,
            prompt_fragments: Vec::new(),
            config_path: None,
            planning: hellox_agent::PlanningState::default(),
            working_directory: String::from("D:\\workspace"),
            shell_name: String::from("powershell"),
            system_prompt: String::from("system"),
            created_at: 1,
            updated_at: 2,
            agent_runtime: None,
            usage_by_model: Default::default(),
            messages: vec![serde_json::from_value(serde_json::json!({
                "role": "user",
                "content": "hello"
            }))
            .expect("build message")],
        }
    }

    #[test]
    fn default_share_path_uses_session_id_and_markdown_extension() {
        let root = temp_root();
        let path = default_share_path(&root, Some("abc"));
        assert!(path.starts_with(&root));
        assert!(path.to_string_lossy().contains("abc-"));
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("md")
        );
    }

    #[test]
    fn export_session_markdown_writes_transcript() {
        let root = temp_root();
        let session = session(&root);
        let output = root.join("shares").join("share.md");

        export_session_markdown(&session, &output).expect("export transcript");

        let markdown = fs::read_to_string(&output).expect("read transcript");
        assert!(markdown.contains("# hellox transcript"));
        assert!(markdown.contains("- session_id: (ephemeral)"));
        assert!(markdown.contains("- permission_mode: accept_edits"));
        assert!(markdown.contains("_No transcript messages recorded yet._"));
    }

    #[test]
    fn export_stored_session_markdown_writes_persisted_session_id() {
        let root = temp_root();
        let output = root.join("shares").join("stored.md");

        export_stored_session_markdown(&stored_snapshot(), &output)
            .expect("export stored transcript");

        let markdown = fs::read_to_string(&output).expect("read stored transcript");
        assert!(markdown.contains("- session_id: persisted-session"));
        assert!(markdown.contains("- working_directory: D:/workspace"));
        assert!(markdown.contains("## 1 User"));
    }
}
