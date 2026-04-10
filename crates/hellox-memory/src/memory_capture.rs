use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use hellox_agent::{compact_messages, AgentSession, CompactResult, StoredSessionSnapshot};
use hellox_gateway_api::{Message, MessageRole};

use crate::memory_extract::{
    extract_memory_sections, merge_memory_sections, parse_memory_sections, render_memory_sections,
    MemorySectionLimits,
};
use crate::{
    compact_mode_label, current_memory_id, current_project_memory_id, derive_project_memory_id,
    derive_session_memory_id, memory_path_for_scope, normalize_path, sanitize_instructions,
    unix_timestamp, MemoryCaptureResult, MemoryMetadata, MemoryScope,
};

const PROJECT_SECTION_LIMITS: MemorySectionLimits = MemorySectionLimits {
    key_points: 6,
    pending_work: 6,
    risks: 5,
    recent_artifacts: 8,
};

pub fn capture_memory_from_session(
    session: &AgentSession,
    root: &Path,
    instructions: Option<&str>,
) -> Result<MemoryCaptureResult> {
    let mut messages = session.messages().to_vec();
    let result = compact_messages(&mut messages, instructions);
    write_memory_from_session_summary(
        session,
        root,
        &result,
        instructions,
        Some(session.messages()),
    )
}

pub fn write_memory_from_session_summary(
    session: &AgentSession,
    root: &Path,
    result: &CompactResult,
    instructions: Option<&str>,
    transcript: Option<&[Message]>,
) -> Result<MemoryCaptureResult> {
    let session_memory_id = current_memory_id(session);
    let project_memory_id = current_project_memory_id(session);
    let session_path = memory_path_for_scope(root, MemoryScope::Session, &session_memory_id);
    let project_path = memory_path_for_scope(root, MemoryScope::Project, &project_memory_id);
    let instructions = sanitize_instructions(instructions);
    let fresh_sections = extract_memory_sections(&result.summary, transcript);

    write_memory_file(
        &session_path,
        render_memory_markdown(
            MemoryMetadata {
                scope: MemoryScope::Session,
                memory_id: session_memory_id.clone(),
                source_session_id: session.session_id().map(ToString::to_string),
                model: session.model().to_string(),
                permission_mode: session.permission_mode().to_string(),
                working_directory: normalize_path(
                    &session.working_directory().display().to_string(),
                ),
                source_message_count: result.original_message_count,
                mode: result.mode,
                instructions: instructions.clone(),
            },
            &result.summary,
            &fresh_sections,
        ),
    )?;
    write_memory_file(
        &project_path,
        render_memory_markdown(
            MemoryMetadata {
                scope: MemoryScope::Project,
                memory_id: project_memory_id.clone(),
                source_session_id: session.session_id().map(ToString::to_string),
                model: session.model().to_string(),
                permission_mode: session.permission_mode().to_string(),
                working_directory: normalize_path(
                    &session.working_directory().display().to_string(),
                ),
                source_message_count: result.original_message_count,
                mode: result.mode,
                instructions,
            },
            &build_project_summary(&project_path, &result.summary, &fresh_sections)?,
            &build_project_sections(&project_path, fresh_sections.clone())?,
        ),
    )?;

    Ok(MemoryCaptureResult {
        memory_id: session_memory_id,
        path: session_path,
        project_memory_id: Some(project_memory_id),
        project_path: Some(project_path),
        mode: result.mode,
        source_message_count: result.original_message_count,
    })
}

pub fn capture_memory_from_snapshot(
    snapshot: &StoredSessionSnapshot,
    root: &Path,
    instructions: Option<&str>,
) -> Result<MemoryCaptureResult> {
    let transcript = restore_messages(snapshot);
    let mut messages = transcript.clone();
    let result = compact_messages(&mut messages, instructions);
    write_memory_from_snapshot_summary(snapshot, root, &result, instructions, Some(&transcript))
}

pub fn write_memory_from_snapshot_summary(
    snapshot: &StoredSessionSnapshot,
    root: &Path,
    result: &CompactResult,
    instructions: Option<&str>,
    transcript: Option<&[Message]>,
) -> Result<MemoryCaptureResult> {
    let working_directory = Path::new(&snapshot.working_directory);
    let session_memory_id =
        derive_session_memory_id(Some(snapshot.session_id.as_str()), working_directory);
    let project_memory_id = derive_project_memory_id(working_directory);
    let session_path = memory_path_for_scope(root, MemoryScope::Session, &session_memory_id);
    let project_path = memory_path_for_scope(root, MemoryScope::Project, &project_memory_id);
    let instructions = sanitize_instructions(instructions);
    let fresh_sections = extract_memory_sections(&result.summary, transcript);

    write_memory_file(
        &session_path,
        render_memory_markdown(
            MemoryMetadata {
                scope: MemoryScope::Session,
                memory_id: session_memory_id.clone(),
                source_session_id: Some(snapshot.session_id.clone()),
                model: snapshot.model.clone(),
                permission_mode: snapshot
                    .permission_mode
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "(from current config)".to_string()),
                working_directory: normalize_path(&snapshot.working_directory),
                source_message_count: result.original_message_count,
                mode: result.mode,
                instructions: instructions.clone(),
            },
            &result.summary,
            &fresh_sections,
        ),
    )?;
    write_memory_file(
        &project_path,
        render_memory_markdown(
            MemoryMetadata {
                scope: MemoryScope::Project,
                memory_id: project_memory_id.clone(),
                source_session_id: Some(snapshot.session_id.clone()),
                model: snapshot.model.clone(),
                permission_mode: snapshot
                    .permission_mode
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "(from current config)".to_string()),
                working_directory: normalize_path(&snapshot.working_directory),
                source_message_count: result.original_message_count,
                mode: result.mode,
                instructions,
            },
            &build_project_summary(&project_path, &result.summary, &fresh_sections)?,
            &build_project_sections(&project_path, fresh_sections.clone())?,
        ),
    )?;

    Ok(MemoryCaptureResult {
        memory_id: session_memory_id,
        path: session_path,
        project_memory_id: Some(project_memory_id),
        project_path: Some(project_path),
        mode: result.mode,
        source_message_count: result.original_message_count,
    })
}

fn restore_messages(snapshot: &StoredSessionSnapshot) -> Vec<Message> {
    snapshot
        .messages
        .iter()
        .map(|message| Message {
            role: if message.role.eq_ignore_ascii_case("assistant") {
                MessageRole::Assistant
            } else {
                MessageRole::User
            },
            content: message.content.clone(),
        })
        .collect()
}

fn render_memory_markdown(
    metadata: MemoryMetadata,
    summary: &str,
    sections: &crate::memory_extract::ExtractedMemorySections,
) -> String {
    let mut lines = vec![
        "# hellox memory".to_string(),
        String::new(),
        format!("- scope: {}", metadata.scope.as_str()),
        format!("- memory_id: {}", metadata.memory_id),
        format!(
            "- source_session_id: {}",
            metadata
                .source_session_id
                .as_deref()
                .unwrap_or("(ephemeral)")
        ),
        format!("- model: {}", metadata.model),
        format!("- permission_mode: {}", metadata.permission_mode),
        format!("- working_directory: {}", metadata.working_directory),
        format!("- source_messages: {}", metadata.source_message_count),
        format!("- compact_mode: {}", compact_mode_label(metadata.mode)),
        format!("- updated_at: {}", unix_timestamp()),
    ];

    if let Some(instructions) = metadata.instructions {
        lines.push(format!("- instructions: {instructions}"));
    }

    lines.push(String::new());
    lines.push("## Summary".to_string());
    lines.push(String::new());
    lines.push(summary.to_string());
    lines.extend(render_memory_sections(sections));
    lines.join("\n")
}

fn build_project_summary(
    project_path: &Path,
    current_summary: &str,
    fresh_sections: &crate::memory_extract::ExtractedMemorySections,
) -> Result<String> {
    let existing_markdown = read_existing_memory(project_path)?;
    let existing_sections = existing_markdown
        .as_deref()
        .map(parse_memory_sections)
        .unwrap_or_default();
    let merged = merge_memory_sections(
        existing_sections,
        fresh_sections.clone(),
        PROJECT_SECTION_LIMITS,
    );

    let mut lines = vec![
        "Project memory rolls up accepted decisions, active work, risks, and artifacts observed across recent sessions."
            .to_string(),
        format!("Latest session summary: {}", first_non_empty_line(current_summary)),
    ];

    if !merged.pending_work.is_empty() {
        lines.push(format!(
            "Active pending work: {}",
            merged.pending_work.join(" | ")
        ));
    }
    if !merged.risks.is_empty() {
        lines.push(format!("Known risks: {}", merged.risks.join(" | ")));
    }
    if !merged.recent_artifacts.is_empty() {
        lines.push(format!(
            "Recent artifacts: {}",
            merged.recent_artifacts.join(" | ")
        ));
    }

    Ok(lines.join("\n"))
}

fn build_project_sections(
    project_path: &Path,
    fresh_sections: crate::memory_extract::ExtractedMemorySections,
) -> Result<crate::memory_extract::ExtractedMemorySections> {
    let existing_markdown = read_existing_memory(project_path)?;
    let existing_sections = existing_markdown
        .as_deref()
        .map(parse_memory_sections)
        .unwrap_or_default();
    Ok(merge_memory_sections(
        existing_sections,
        fresh_sections,
        PROJECT_SECTION_LIMITS,
    ))
}

fn read_existing_memory(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path).with_context(|| {
        format!("failed to read existing memory file {}", path.display())
    })?))
}

fn first_non_empty_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "(empty summary)".to_string())
}

fn write_memory_file(path: &Path, markdown: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create memory directory {}", parent.display()))?;
    }

    fs::write(path, markdown)
        .with_context(|| format!("failed to write memory file {}", path.display()))
}
