use std::path::Path;

use anyhow::Result;
use hellox_agent::{compact_messages, AgentSession};

use crate::memory::{
    current_memory_id, load_memory, memory_result_targets, write_memory_from_session_summary,
    MemoryCaptureResult,
};

pub(crate) const AUTO_MEMORY_REFRESH_MIN_MESSAGES: usize = 8;
const AUTO_MEMORY_REFRESH_DELTA: usize = 6;
const AUTO_MEMORY_REFRESH_INSTRUCTIONS: &str =
    "Refresh accepted decisions, active implementation context, pending work, and recent outcomes without compacting the session.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutoMemoryRefreshOutcome {
    pub(crate) previous_source_messages: Option<usize>,
    pub(crate) refreshed_source_messages: usize,
    pub(crate) memory: MemoryCaptureResult,
}

pub(crate) fn maybe_auto_refresh_session_memory(
    session: &AgentSession,
    memory_root: &Path,
) -> Result<Option<AutoMemoryRefreshOutcome>> {
    let current_message_count = session.message_count();
    if current_message_count < AUTO_MEMORY_REFRESH_MIN_MESSAGES {
        return Ok(None);
    }

    let previous_source_messages = read_previous_source_messages(session, memory_root);
    if let Some(previous_source_messages) = previous_source_messages {
        let baseline = if previous_source_messages > current_message_count {
            1
        } else {
            previous_source_messages
        };
        if current_message_count < baseline.saturating_add(AUTO_MEMORY_REFRESH_DELTA) {
            return Ok(None);
        }
    }

    let transcript = session.messages().to_vec();
    let mut messages = transcript.clone();
    let compact = compact_messages(&mut messages, Some(AUTO_MEMORY_REFRESH_INSTRUCTIONS));
    let memory = write_memory_from_session_summary(
        session,
        memory_root,
        &compact,
        Some(AUTO_MEMORY_REFRESH_INSTRUCTIONS),
        Some(transcript.as_slice()),
    )?;

    Ok(Some(AutoMemoryRefreshOutcome {
        previous_source_messages,
        refreshed_source_messages: compact.original_message_count,
        memory,
    }))
}

pub(crate) fn format_auto_memory_refresh_notice(outcome: &AutoMemoryRefreshOutcome) -> String {
    let previous = outcome
        .previous_source_messages
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string());
    format!(
        "Auto-refreshed layered memory at {} message(s) without compacting the session (previous source_messages: {}). {}",
        outcome.refreshed_source_messages,
        previous,
        memory_result_targets(&outcome.memory)
    )
}

fn read_previous_source_messages(session: &AgentSession, memory_root: &Path) -> Option<usize> {
    let memory_id = current_memory_id(session);
    let markdown = load_memory(memory_root, &memory_id).ok()?;
    markdown.lines().find_map(parse_source_messages)
}

fn parse_source_messages(line: &str) -> Option<usize> {
    line.strip_prefix("- source_messages: ")?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_agent::{
        default_tool_registry, AgentOptions, AgentSession, GatewayClient, StoredSession,
        StoredSessionMessage, StoredSessionSnapshot,
    };
    use hellox_config::PermissionMode;
    use serde_json::json;

    use super::{
        format_auto_memory_refresh_notice, maybe_auto_refresh_session_memory,
        parse_source_messages, AUTO_MEMORY_REFRESH_MIN_MESSAGES,
    };
    use crate::memory::load_memory;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-auto-memory-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn restorable_session(root: &Path, message_count: usize) -> AgentSession {
        let messages = (0..message_count)
            .map(|index| {
                let role = if index % 2 == 0 { "user" } else { "assistant" };
                serde_json::from_value::<StoredSessionMessage>(json!({
                    "role": role,
                    "content": format!("message {index}")
                }))
                .expect("message")
            })
            .collect();
        let stored = StoredSession {
            session_id: String::from("auto-memory-session"),
            path: root.join("auto-memory-session.json"),
            snapshot: StoredSessionSnapshot {
                session_id: String::from("auto-memory-session"),
                model: String::from("opus"),
                permission_mode: Some(PermissionMode::AcceptEdits),
                output_style_name: None,
                output_style: None,
                persona: None,
                prompt_fragments: Vec::new(),
                config_path: None,
                planning: hellox_agent::PlanningState::default(),
                working_directory: root.display().to_string(),
                shell_name: String::from("powershell"),
                system_prompt: String::from("system"),
                created_at: 1,
                updated_at: 2,
                agent_runtime: None,
                usage_by_model: Default::default(),
                messages,
            },
        };

        AgentSession::restore(
            GatewayClient::new("http://127.0.0.1:7821"),
            default_tool_registry(),
            AgentOptions::default(),
            PermissionMode::Default,
            None,
            None,
            stored,
        )
    }

    #[test]
    fn skips_auto_memory_refresh_below_threshold() {
        let root = temp_dir();
        let memory_root = root.join("memory");
        let session = restorable_session(&root, AUTO_MEMORY_REFRESH_MIN_MESSAGES - 1);

        let result =
            maybe_auto_refresh_session_memory(&session, &memory_root).expect("auto refresh");

        assert!(result.is_none());
        assert!(!memory_root.exists());
    }

    #[test]
    fn auto_memory_refresh_writes_layered_memory_without_compacting_session() {
        let root = temp_dir();
        let memory_root = root.join("memory");
        let session = restorable_session(&root, AUTO_MEMORY_REFRESH_MIN_MESSAGES);

        let outcome = maybe_auto_refresh_session_memory(&session, &memory_root)
            .expect("auto refresh")
            .expect("should refresh");

        assert_eq!(session.message_count(), AUTO_MEMORY_REFRESH_MIN_MESSAGES);
        assert_eq!(
            outcome.refreshed_source_messages,
            AUTO_MEMORY_REFRESH_MIN_MESSAGES
        );

        let session_memory =
            load_memory(&memory_root, &outcome.memory.memory_id).expect("load session memory");
        assert!(session_memory.contains("Refresh accepted decisions"));
        let notice = format_auto_memory_refresh_notice(&outcome);
        assert!(notice.contains("Auto-refreshed layered memory"));
        assert!(notice.contains("previous source_messages: none"));
    }

    #[test]
    fn skips_refresh_when_existing_memory_is_recent_enough() {
        let root = temp_dir();
        let memory_root = root.join("memory");
        let session = restorable_session(&root, AUTO_MEMORY_REFRESH_MIN_MESSAGES);

        let first = maybe_auto_refresh_session_memory(&session, &memory_root)
            .expect("first refresh")
            .expect("should refresh");
        assert_eq!(
            first.refreshed_source_messages,
            AUTO_MEMORY_REFRESH_MIN_MESSAGES
        );

        let second =
            maybe_auto_refresh_session_memory(&session, &memory_root).expect("second refresh");
        assert!(second.is_none());
    }

    #[test]
    fn parse_source_messages_extracts_metadata_value() {
        assert_eq!(parse_source_messages("- source_messages: 12"), Some(12));
        assert_eq!(parse_source_messages("- source_messages:"), None);
    }
}
