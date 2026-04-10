use std::path::Path;

use anyhow::Result;
use hellox_agent::{AgentSession, CompactMode};

use crate::memory::{
    memory_result_targets, write_memory_from_session_summary, MemoryCaptureResult,
};

pub(crate) const AUTO_COMPACT_MESSAGE_THRESHOLD: usize = 24;
const AUTO_COMPACT_INSTRUCTIONS: &str =
    "Preserve the active task, accepted decisions, pending work, and relevant tool outcomes.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutoCompactOutcome {
    pub(crate) compact_mode: CompactMode,
    pub(crate) original_message_count: usize,
    pub(crate) retained_message_count: usize,
    pub(crate) memory: MemoryCaptureResult,
}

pub(crate) fn maybe_auto_compact_session(
    session: &mut AgentSession,
    memory_root: &Path,
) -> Result<Option<AutoCompactOutcome>> {
    if session.message_count() < AUTO_COMPACT_MESSAGE_THRESHOLD {
        return Ok(None);
    }

    let compact = session.compact(Some(AUTO_COMPACT_INSTRUCTIONS))?;
    let memory = write_memory_from_session_summary(
        session,
        memory_root,
        &compact,
        Some(AUTO_COMPACT_INSTRUCTIONS),
        None,
    )?;

    Ok(Some(AutoCompactOutcome {
        compact_mode: compact.mode,
        original_message_count: compact.original_message_count,
        retained_message_count: compact.retained_message_count,
        memory,
    }))
}

pub(crate) fn format_auto_compact_notice(outcome: &AutoCompactOutcome) -> String {
    format!(
        "Auto-compacted current session in {} mode after {} message(s); retained {} summary message(s). {}",
        compact_mode_label(outcome.compact_mode),
        outcome.original_message_count,
        outcome.retained_message_count,
        memory_result_targets(&outcome.memory)
    )
}

fn compact_mode_label(mode: CompactMode) -> &'static str {
    match mode {
        CompactMode::Micro => "microcompact",
        CompactMode::Full => "compact",
    }
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
        format_auto_compact_notice, maybe_auto_compact_session, AUTO_COMPACT_MESSAGE_THRESHOLD,
    };
    use crate::memory::load_memory;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-auto-compact-{suffix}"));
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
            session_id: String::from("auto-compact-session"),
            path: root.join("auto-compact-session.json"),
            snapshot: StoredSessionSnapshot {
                session_id: String::from("auto-compact-session"),
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
    fn skips_auto_compact_below_threshold() {
        let root = temp_dir();
        let memory_root = root.join("memory");
        let mut session = restorable_session(&root, AUTO_COMPACT_MESSAGE_THRESHOLD - 1);

        let result = maybe_auto_compact_session(&mut session, &memory_root).expect("auto compact");

        assert!(result.is_none());
        assert_eq!(session.message_count(), AUTO_COMPACT_MESSAGE_THRESHOLD - 1);
        assert!(!memory_root.exists());
    }

    #[test]
    fn auto_compact_rewrites_session_and_refreshes_layered_memory() {
        let root = temp_dir();
        let memory_root = root.join("memory");
        let mut session = restorable_session(&root, AUTO_COMPACT_MESSAGE_THRESHOLD);

        let outcome = maybe_auto_compact_session(&mut session, &memory_root)
            .expect("auto compact")
            .expect("should compact");

        assert_eq!(session.message_count(), 1);
        assert_eq!(
            outcome.original_message_count,
            AUTO_COMPACT_MESSAGE_THRESHOLD
        );
        assert_eq!(outcome.retained_message_count, 1);

        let summary = match &session.messages()[0].content {
            hellox_gateway_api::MessageContent::Text(text) => text,
            other => panic!("expected summary text, got {other:?}"),
        };
        assert!(summary.contains("Conversation summary generated by hellox /compact."));
        assert!(summary.contains(
            "Compaction instructions: Preserve the active task, accepted decisions, pending work, and relevant tool outcomes."
        ));

        let session_memory =
            load_memory(&memory_root, &outcome.memory.memory_id).expect("load session memory");
        assert!(session_memory.contains("- scope: session"));
        assert!(session_memory.contains("Preserve the active task"));

        let project_memory_id = outcome
            .memory
            .project_memory_id
            .clone()
            .expect("project memory id");
        let project_memory =
            load_memory(&memory_root, &project_memory_id).expect("load project memory");
        assert!(project_memory.contains("- scope: project"));

        let notice = format_auto_compact_notice(&outcome);
        assert!(notice.contains("Auto-compacted current session in compact mode"));
        assert!(notice.contains("Session memory"));
        assert!(notice.contains("Project memory"));
    }
}
