use std::path::Path;

use anyhow::Result;
use hellox_agent::AgentSession;
use hellox_gateway_api::{extract_text, MessageContent};

use crate::memory::search_memories_ranked;
use crate::sessions::{list_sessions, load_session};

pub const DEFAULT_SEARCH_LIMIT: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub source_kind: &'static str,
    pub source_id: String,
    pub location: String,
    pub preview: String,
}

pub fn search_current_session(session: &AgentSession, query: &str, limit: usize) -> Vec<SearchHit> {
    search_message_contents(
        "transcript",
        session.session_id().unwrap_or("(ephemeral)"),
        session
            .messages()
            .iter()
            .enumerate()
            .map(|(index, message)| (index + 1, &message.content)),
        query,
        limit,
    )
}

pub fn search_sessions(root: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let mut hits = Vec::new();
    for summary in list_sessions(root)? {
        if hits.len() >= limit {
            break;
        }

        let snapshot = load_session(root, &summary.session_id)?;
        hits.extend(search_message_contents(
            "session",
            &snapshot.session_id,
            snapshot
                .messages
                .iter()
                .enumerate()
                .map(|(index, message)| (index + 1, &message.content)),
            query,
            limit.saturating_sub(hits.len()),
        ));
    }
    Ok(hits)
}

pub fn search_memories(root: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    Ok(search_memories_ranked(root, query, limit)?
        .into_iter()
        .map(|hit| SearchHit {
            source_kind: "memory",
            source_id: hit.memory_id,
            location: format!("{} ({}, score {})", hit.location, hit.age, hit.score),
            preview: hit.preview,
        })
        .collect())
}

pub fn merge_search_hits(limit: usize, groups: Vec<Vec<SearchHit>>) -> Vec<SearchHit> {
    groups.into_iter().flatten().take(limit).collect()
}

pub fn format_search_results(query: &str, hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return format!("No search hits for `{query}`.");
    }

    let mut lines = Vec::with_capacity(hits.len() + 1);
    lines.push("source\tsource_id\tlocation\tpreview".to_string());
    for hit in hits {
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            hit.source_kind, hit.source_id, hit.location, hit.preview
        ));
    }
    lines.join("\n")
}

fn search_message_contents<'a>(
    source_kind: &'static str,
    source_id: &str,
    items: impl Iterator<Item = (usize, &'a MessageContent)>,
    query: &str,
    limit: usize,
) -> Vec<SearchHit> {
    let query_lower = query.to_ascii_lowercase();
    let mut hits = Vec::new();

    for (index, content) in items {
        if hits.len() >= limit {
            break;
        }

        let text = extract_text(content);
        for line in text.lines() {
            if hits.len() >= limit {
                break;
            }
            if line.to_ascii_lowercase().contains(&query_lower) {
                hits.push(SearchHit {
                    source_kind,
                    source_id: source_id.to_string(),
                    location: format!("message {}", index),
                    preview: collapse_preview(line),
                });
                break;
            }
        }
    }

    hits
}

fn collapse_preview(line: &str) -> String {
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 120 {
        return collapsed;
    }

    let truncated = collapsed.chars().take(117).collect::<String>();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_agent::{
        default_tool_registry, AgentOptions, AgentSession, GatewayClient, StoredSession,
        StoredSessionSnapshot,
    };
    use hellox_config::PermissionMode;
    use serde_json::json;

    use super::{
        format_search_results, merge_search_hits, search_current_session, search_memories,
        search_sessions,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-search-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn restored_session(root: &Path, session_id: &str, messages: &[(&str, &str)]) -> AgentSession {
        let stored = StoredSession {
            session_id: session_id.to_string(),
            path: root.join(format!("{session_id}.json")),
            snapshot: StoredSessionSnapshot {
                session_id: session_id.to_string(),
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
                messages: messages
                    .iter()
                    .map(|(role, content)| {
                        serde_json::from_value(json!({
                            "role": role,
                            "content": content,
                        }))
                        .expect("message")
                    })
                    .collect(),
            },
        };
        AgentSession::restore(
            GatewayClient::new("http://127.0.0.1:7821"),
            default_tool_registry(),
            AgentOptions::default(),
            PermissionMode::AcceptEdits,
            None,
            None,
            stored,
        )
    }

    fn write_snapshot(root: &Path, session_id: &str, content: &str) {
        let snapshot = StoredSessionSnapshot {
            session_id: session_id.to_string(),
            model: String::from("opus"),
            permission_mode: Some(PermissionMode::AcceptEdits),
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
                "content": content
            }))
            .expect("message")],
        };
        fs::create_dir_all(root).expect("create sessions");
        fs::write(
            root.join(format!("{session_id}.json")),
            serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
        )
        .expect("write snapshot");
    }

    #[test]
    fn search_current_session_returns_transcript_hits() {
        let root = temp_root();
        let session = restored_session(
            &root,
            "search-session",
            &[
                ("user", "capture accepted architecture"),
                (
                    "assistant",
                    "summary keeps the accepted architecture active",
                ),
            ],
        );
        let hits = search_current_session(&session, "summary", 10);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].source_kind, "transcript");
        assert_eq!(hits[0].source_id, "search-session");
        assert_eq!(hits[0].location, "message 2");
    }

    #[test]
    fn search_sessions_and_memories_return_hits() {
        let root = temp_root();
        let sessions_root = root.join("sessions");
        let memory_root = root.join("memory").join("sessions");
        write_snapshot(&sessions_root, "abc", "find accepted architecture");
        fs::create_dir_all(&memory_root).expect("create memories");
        fs::write(
            memory_root.join("session-abc.md"),
            "# hellox memory\n\naccepted architecture",
        )
        .expect("write memory");

        let session_hits =
            search_sessions(&sessions_root, "architecture", 10).expect("session hits");
        let memory_hits =
            search_memories(&root.join("memory"), "architecture", 10).expect("memory hits");
        let merged = merge_search_hits(10, vec![session_hits, memory_hits]);
        let rendered = format_search_results("architecture", &merged);

        assert!(rendered.contains("session"));
        assert!(rendered.contains("memory"));
    }
}
