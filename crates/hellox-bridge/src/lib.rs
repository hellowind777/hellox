use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result};
use hellox_agent::StoredSessionSnapshot;
use hellox_config::{default_config_path, load_or_default, plugins_root, sessions_root};
use hellox_gateway_api::{
    ContentBlock, DocumentSource, ImageSource, MessageContent, ToolResultContent,
};
use serde::{Deserialize, Serialize};

mod protocol;

#[cfg(test)]
mod tests;

pub use protocol::{run_stdio_bridge, BridgeResponse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeRuntimePaths {
    pub config_path: PathBuf,
    pub sessions_root: PathBuf,
    pub plugins_root: PathBuf,
}

impl BridgeRuntimePaths {
    pub fn new(config_path: PathBuf, sessions_root: PathBuf, plugins_root: PathBuf) -> Self {
        Self {
            config_path,
            sessions_root,
            plugins_root,
        }
    }
}

impl Default for BridgeRuntimePaths {
    fn default() -> Self {
        Self::new(default_config_path(), sessions_root(), plugins_root())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeStatus {
    pub config_path: String,
    pub sessions_root: String,
    pub plugins_root: String,
    pub persisted_sessions: usize,
    pub configured_mcp_servers: usize,
    pub enabled_mcp_servers: usize,
    pub installed_plugins: usize,
    pub enabled_plugins: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeSessionSummary {
    pub session_id: String,
    pub model: String,
    pub permission_mode: String,
    pub working_directory: String,
    pub updated_at: u64,
    pub message_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeTranscriptEntry {
    pub role: String,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeSessionDetail {
    pub summary: BridgeSessionSummary,
    pub shell_name: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub system_prompt_preview: String,
    pub transcript: Vec<BridgeTranscriptEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdeStatus {
    pub bridge: BridgeStatus,
    pub latest_session: Option<BridgeSessionSummary>,
}

pub fn inspect_bridge_status(paths: &BridgeRuntimePaths) -> Result<BridgeStatus> {
    let config = load_or_default(Some(paths.config_path.clone()))?;
    let sessions = list_bridge_sessions(paths)?;
    let enabled_mcp_servers = config
        .mcp
        .servers
        .values()
        .filter(|server| server.enabled)
        .count();
    let enabled_plugins = config
        .plugins
        .installed
        .values()
        .filter(|plugin| plugin.enabled)
        .count();

    Ok(BridgeStatus {
        config_path: normalize_path(&paths.config_path),
        sessions_root: normalize_path(&paths.sessions_root),
        plugins_root: normalize_path(&paths.plugins_root),
        persisted_sessions: sessions.len(),
        configured_mcp_servers: config.mcp.servers.len(),
        enabled_mcp_servers,
        installed_plugins: config.plugins.installed.len(),
        enabled_plugins,
    })
}

pub fn inspect_ide_status(paths: &BridgeRuntimePaths) -> Result<IdeStatus> {
    let sessions = list_bridge_sessions(paths)?;
    Ok(IdeStatus {
        bridge: inspect_bridge_status(paths)?,
        latest_session: sessions.into_iter().next(),
    })
}

pub fn list_bridge_sessions(paths: &BridgeRuntimePaths) -> Result<Vec<BridgeSessionSummary>> {
    if !paths.sessions_root.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in fs::read_dir(&paths.sessions_root).with_context(|| {
        format!(
            "failed to list sessions in {}",
            paths.sessions_root.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let snapshot = read_session_snapshot(&path)?;
        sessions.push(build_session_summary(&snapshot));
    }

    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    Ok(sessions)
}

pub fn load_bridge_session(
    paths: &BridgeRuntimePaths,
    session_id: &str,
) -> Result<BridgeSessionDetail> {
    let snapshot = read_session_snapshot(&paths.sessions_root.join(format!("{session_id}.json")))
        .with_context(|| {
        format!(
            "failed to load bridge session `{session_id}` from {}",
            paths.sessions_root.display()
        )
    })?;

    Ok(BridgeSessionDetail {
        summary: build_session_summary(&snapshot),
        shell_name: snapshot.shell_name.clone(),
        created_at: snapshot.created_at,
        updated_at: snapshot.updated_at,
        system_prompt_preview: truncate_preview(&snapshot.system_prompt, 160),
        transcript: snapshot
            .messages
            .iter()
            .map(|message| BridgeTranscriptEntry {
                role: message.role.clone(),
                preview: message_preview(&message.content),
            })
            .collect(),
    })
}

pub fn format_bridge_status(status: &BridgeStatus) -> String {
    format!(
        "mode: local_bridge\nconfig_path: {}\nsessions_root: {}\nplugins_root: {}\npersisted_sessions: {}\nconfigured_mcp_servers: {}\nenabled_mcp_servers: {}\ninstalled_plugins: {}\nenabled_plugins: {}",
        status.config_path,
        status.sessions_root,
        status.plugins_root,
        status.persisted_sessions,
        status.configured_mcp_servers,
        status.enabled_mcp_servers,
        status.installed_plugins,
        status.enabled_plugins
    )
}

pub fn format_ide_status(status: &IdeStatus) -> String {
    let mut lines = vec![format_bridge_status(&status.bridge)];
    match &status.latest_session {
        Some(session) => lines.push(format!(
            "latest_session: {} ({}, {} messages)",
            session.session_id, session.model, session.message_count
        )),
        None => lines.push("latest_session: (none)".to_string()),
    }
    lines.join("\n")
}

pub fn format_bridge_session_list(sessions: &[BridgeSessionSummary]) -> String {
    if sessions.is_empty() {
        return "No persisted bridge sessions found.".to_string();
    }

    let mut lines = vec![
        "session_id\tmodel\tpermission_mode\tmessages\tupdated_at\tworking_directory".to_string(),
    ];
    for session in sessions {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            session.session_id,
            session.model,
            session.permission_mode,
            session.message_count,
            session.updated_at,
            session.working_directory
        ));
    }
    lines.join("\n")
}

pub fn format_bridge_session_detail(detail: &BridgeSessionDetail) -> String {
    let transcript = if detail.transcript.is_empty() {
        "(empty)".to_string()
    } else {
        detail
            .transcript
            .iter()
            .map(|entry| format!("- {}: {}", entry.role, entry.preview))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "session_id: {}\nmodel: {}\npermission_mode: {}\nworking_directory: {}\nshell: {}\ncreated_at: {}\nupdated_at: {}\nmessages: {}\nsystem_prompt_preview: {}\ntranscript:\n{}",
        detail.summary.session_id,
        detail.summary.model,
        detail.summary.permission_mode,
        detail.summary.working_directory,
        detail.shell_name,
        detail.created_at,
        detail.updated_at,
        detail.summary.message_count,
        detail.system_prompt_preview,
        transcript
    )
}

fn build_session_summary(snapshot: &StoredSessionSnapshot) -> BridgeSessionSummary {
    BridgeSessionSummary {
        session_id: snapshot.session_id.clone(),
        model: snapshot.model.clone(),
        permission_mode: snapshot
            .permission_mode
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "(from current config)".to_string()),
        working_directory: normalize_path(Path::new(&snapshot.working_directory)),
        updated_at: snapshot.updated_at,
        message_count: snapshot.messages.len(),
    }
}

fn read_session_snapshot(path: &Path) -> Result<StoredSessionSnapshot> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read session file {}", path.display()))?;
    serde_json::from_str::<StoredSessionSnapshot>(&raw)
        .with_context(|| format!("failed to parse session file {}", path.display()))
}

fn message_preview(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => truncate_preview(text, 160),
        MessageContent::Blocks(blocks) => truncate_preview(
            &blocks
                .iter()
                .map(block_preview)
                .collect::<Vec<_>>()
                .join(" | "),
            160,
        ),
        MessageContent::Empty => "(empty)".to_string(),
    }
}

fn block_preview(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text { text } => sanitize_inline(text),
        ContentBlock::Image { source } => format!("image: {}", image_source_preview(source)),
        ContentBlock::Document {
            source,
            title,
            context,
            ..
        } => format!(
            "document: {}",
            document_source_preview(source, title.as_deref(), context.as_deref())
        ),
        ContentBlock::Thinking { thinking, .. } => {
            format!("thinking: {}", truncate_preview(thinking, 64))
        }
        ContentBlock::RedactedThinking { .. } => "redacted_thinking".to_string(),
        ContentBlock::ToolUse { name, .. } => format!("tool_use: {name}"),
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            let state = if *is_error { "error" } else { "ok" };
            format!("tool_result ({state}): {}", tool_result_preview(content))
        }
    }
}

fn image_source_preview(source: &ImageSource) -> String {
    match source {
        ImageSource::File { file_id } => format!("file `{file_id}`"),
        ImageSource::Url { url } => format!("url {}", truncate_preview(url, 64)),
        ImageSource::Base64 { media_type, .. } => format!("base64 {media_type}"),
    }
}

fn document_source_preview(
    source: &DocumentSource,
    title: Option<&str>,
    context: Option<&str>,
) -> String {
    let label = title.or(context).unwrap_or("document");
    match source {
        DocumentSource::File { file_id } => format!("{label} (file `{file_id}`)"),
        DocumentSource::Url { url } => format!("{label} ({})", truncate_preview(url, 64)),
        DocumentSource::Base64 { media_type, .. } => format!("{label} ({media_type})"),
        DocumentSource::Text { data, .. } => format!("{label}: {}", truncate_preview(data, 64)),
        DocumentSource::Content { content } => {
            format!(
                "{label}: {}",
                truncate_preview(&hellox_gateway_api::flatten_text_blocks(content), 64)
            )
        }
    }
}

fn tool_result_preview(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(text) => truncate_preview(text, 64),
        ToolResultContent::Blocks(blocks) => truncate_preview(
            &blocks
                .iter()
                .map(block_preview)
                .collect::<Vec<_>>()
                .join(" | "),
            64,
        ),
        ToolResultContent::Empty => "(empty)".to_string(),
    }
}

fn truncate_preview(value: &str, max_len: usize) -> String {
    let cleaned = sanitize_inline(value);
    if cleaned.chars().count() <= max_len {
        cleaned
    } else {
        let shortened = cleaned
            .chars()
            .take(max_len.saturating_sub(1))
            .collect::<String>();
        format!("{shortened}...")
    }
}

fn sanitize_inline(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
