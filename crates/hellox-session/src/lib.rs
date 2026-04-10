use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use hellox_agent::{StoredSessionSnapshot, StoredSessionUsageTotals};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub session_id: String,
    pub model: String,
    pub working_directory: String,
    pub updated_at: u64,
    pub message_count: usize,
    pub usage_by_model: BTreeMap<String, StoredSessionUsageTotals>,
}

pub fn list_sessions(root: &Path) -> Result<Vec<SessionSummary>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to list sessions in {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let snapshot = read_session_snapshot(&path)?;
        sessions.push(SessionSummary {
            session_id: snapshot.session_id,
            model: snapshot.model,
            working_directory: normalize_path(&snapshot.working_directory),
            updated_at: snapshot.updated_at,
            message_count: snapshot.messages.len(),
            usage_by_model: snapshot.usage_by_model,
        });
    }

    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    Ok(sessions)
}

pub fn load_session(root: &Path, session_id: &str) -> Result<StoredSessionSnapshot> {
    let path = root.join(format!("{session_id}.json"));
    read_session_snapshot(&path).with_context(|| {
        format!(
            "failed to load session `{session_id}` from {}",
            root.display()
        )
    })
}

pub fn format_session_list(sessions: &[SessionSummary]) -> String {
    if sessions.is_empty() {
        return "No persisted sessions found.".to_string();
    }

    let mut lines = Vec::with_capacity(sessions.len() + 1);
    lines.push("session_id\tmodel\tmessages\tupdated_at\tworking_directory".to_string());
    for session in sessions {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}",
            session.session_id,
            session.model,
            session.message_count,
            session.updated_at,
            session.working_directory
        ));
    }
    lines.join("\n")
}

pub fn format_session_detail(snapshot: &StoredSessionSnapshot) -> String {
    format!(
        "session_id: {}\nmodel: {}\npermission_mode: {}\noutput_style: {}\npersona: {}\nprompt_fragments: {}\nplan_mode: {}\nplan_steps: {}\nworking_directory: {}\nshell: {}\ncreated_at: {}\nupdated_at: {}\nmessages: {}\nrequests: {}\ninput_tokens: {}\noutput_tokens: {}",
        snapshot.session_id,
        snapshot.model,
        snapshot
            .permission_mode
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "(from current config)".to_string()),
        snapshot.output_style_name.as_deref().unwrap_or("(none)"),
        snapshot
            .persona
            .as_ref()
            .map(|persona| persona.name.as_str())
            .unwrap_or("(none)"),
        render_prompt_fragments(snapshot),
        snapshot.planning.active,
        snapshot.planning.plan.len(),
        normalize_path(&snapshot.working_directory),
        snapshot.shell_name,
        snapshot.created_at,
        snapshot.updated_at,
        snapshot.messages.len(),
        snapshot.total_requests(),
        snapshot.total_input_tokens(),
        snapshot.total_output_tokens()
    )
}

fn read_session_snapshot(path: &Path) -> Result<StoredSessionSnapshot> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read session file {}", path.display()))?;
    serde_json::from_str::<StoredSessionSnapshot>(&raw)
        .with_context(|| format!("failed to parse session file {}", path.display()))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn render_prompt_fragments(snapshot: &StoredSessionSnapshot) -> String {
    if snapshot.prompt_fragments.is_empty() {
        "(none)".to_string()
    } else {
        snapshot
            .prompt_fragments
            .iter()
            .map(|fragment| fragment.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_agent::{StoredSessionSnapshot, StoredSessionUsageTotals};

    use super::{format_session_detail, format_session_list, list_sessions, load_session};

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-session-cli-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_snapshot(root: &Path, session_id: &str, updated_at: u64, model: &str) {
        let snapshot = StoredSessionSnapshot {
            session_id: session_id.to_string(),
            model: model.to_string(),
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
            created_at: updated_at.saturating_sub(10),
            updated_at,
            agent_runtime: None,
            usage_by_model: BTreeMap::from([(
                model.to_string(),
                StoredSessionUsageTotals {
                    requests: 1,
                    input_tokens: 100,
                    output_tokens: 50,
                },
            )]),
            messages: vec![serde_json::from_value(serde_json::json!({
                "role": "user",
                "content": "hello"
            }))
            .expect("build message")],
        };
        let raw = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");
        fs::write(root.join(format!("{session_id}.json")), raw).expect("write snapshot");
    }

    #[test]
    fn list_sessions_returns_newest_first() {
        let root = temp_root();
        write_snapshot(&root, "older", 10, "opus");
        write_snapshot(&root, "newer", 20, "gpt-5");

        let sessions = list_sessions(&root).expect("list sessions");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "newer");
        assert_eq!(sessions[1].session_id, "older");
    }

    #[test]
    fn load_session_reads_snapshot() {
        let root = temp_root();
        write_snapshot(&root, "abc", 10, "opus");

        let snapshot = load_session(&root, "abc").expect("load session");
        assert_eq!(snapshot.session_id, "abc");
        assert_eq!(snapshot.model, "opus");
    }

    #[test]
    fn formatting_helpers_include_core_fields() {
        let root = temp_root();
        write_snapshot(&root, "abc", 10, "opus");
        let sessions = list_sessions(&root).expect("list sessions");
        let list = format_session_list(&sessions);
        assert!(list.contains("session_id"));
        assert!(list.contains("abc"));

        let detail = format_session_detail(&load_session(&root, "abc").expect("load session"));
        assert!(detail.contains("working_directory: D:/workspace"));
        assert!(detail.contains("permission_mode: accept_edits"));
        assert!(detail.contains("messages: 1"));
        assert!(detail.contains("input_tokens: 100"));
        assert!(detail.contains("output_tokens: 50"));
    }
}
