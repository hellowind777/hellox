mod memory_archive;
mod memory_capture;
mod memory_cluster;
mod memory_decay;
mod memory_extract;
mod memory_query;
mod memory_retention;
mod memory_store;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{AgentSession, CompactMode};

pub use memory_archive::{archive_memories, format_memory_archive_report, MemoryArchiveOptions};
pub use memory_capture::{
    capture_memory_from_session, capture_memory_from_snapshot, write_memory_from_session_summary,
    write_memory_from_snapshot_summary,
};
pub use memory_cluster::{cluster_memories, format_memory_cluster_report, MemoryClusterOptions};
pub use memory_decay::{decay_archived_memories, format_memory_decay_report, MemoryDecayOptions};
pub use memory_query::{
    format_memory_search_results, relative_age_text, search_archived_memories_ranked,
    search_memories_ranked,
};
pub use memory_retention::{
    format_memory_prune_report, prune_memories, MemoryPruneOptions, MemoryScopeSelector,
};
pub use memory_store::{
    format_memory_list, list_archived_memories, list_memories, load_archived_memory, load_memory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    Session,
    Project,
}

impl MemoryScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Project => "project",
        }
    }

    fn directory_name(&self) -> &'static str {
        match self {
            Self::Session => "sessions",
            Self::Project => "projects",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    pub memory_id: String,
    pub scope: MemoryScope,
    pub updated_at: u64,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCaptureResult {
    pub memory_id: String,
    pub path: PathBuf,
    pub project_memory_id: Option<String>,
    pub project_path: Option<PathBuf>,
    pub mode: CompactMode,
    pub source_message_count: usize,
}

pub fn session_memory_root(root: &Path) -> PathBuf {
    memory_scope_root(root, MemoryScope::Session)
}

pub fn project_memory_root(root: &Path) -> PathBuf {
    memory_scope_root(root, MemoryScope::Project)
}

pub fn current_memory_id(session: &AgentSession) -> String {
    derive_session_memory_id(session.session_id(), session.working_directory())
}

pub fn current_project_memory_id(session: &AgentSession) -> String {
    derive_project_memory_id(session.working_directory())
}

pub(crate) struct MemoryMetadata {
    pub(crate) scope: MemoryScope,
    pub(crate) memory_id: String,
    pub(crate) source_session_id: Option<String>,
    pub(crate) model: String,
    pub(crate) permission_mode: String,
    pub(crate) working_directory: String,
    pub(crate) source_message_count: usize,
    pub(crate) mode: CompactMode,
    pub(crate) instructions: Option<String>,
}

pub(crate) fn derive_session_memory_id(
    session_id: Option<&str>,
    working_directory: &Path,
) -> String {
    match session_id {
        Some(session_id) => format!("session-{session_id}"),
        None => format!("workspace-{:016x}", workspace_hash(working_directory)),
    }
}

pub(crate) fn derive_project_memory_id(working_directory: &Path) -> String {
    format!("project-{:016x}", workspace_hash(working_directory))
}

pub(crate) fn memory_path_for_scope(root: &Path, scope: MemoryScope, memory_id: &str) -> PathBuf {
    match scope {
        MemoryScope::Session => session_memory_root(root),
        MemoryScope::Project => project_memory_root(root),
    }
    .join(format!("{memory_id}.md"))
}

pub(crate) fn memory_archive_path_for_scope(
    root: &Path,
    scope: MemoryScope,
    memory_id: &str,
) -> PathBuf {
    root.join("archive")
        .join(scope.directory_name())
        .join(format!("{memory_id}.md"))
}

pub(crate) fn memory_archive_scope_root(root: &Path, scope: MemoryScope) -> PathBuf {
    root.join("archive").join(scope.directory_name())
}

pub(crate) fn memory_scope_root(root: &Path, scope: MemoryScope) -> PathBuf {
    root.join(scope.directory_name())
}

pub(crate) fn sanitize_instructions(instructions: Option<&str>) -> Option<String> {
    instructions
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn compact_mode_label(mode: CompactMode) -> &'static str {
    match mode {
        CompactMode::Micro => "microcompact",
        CompactMode::Full => "compact",
    }
}

pub fn memory_result_targets(result: &MemoryCaptureResult) -> String {
    match (&result.project_memory_id, &result.project_path) {
        (Some(project_memory_id), Some(project_path)) => format!(
            "Session memory `{}` updated at `{}`. Project memory `{}` updated at `{}`.",
            result.memory_id,
            normalize_path(&result.path.display().to_string()),
            project_memory_id,
            normalize_path(&project_path.display().to_string())
        ),
        _ => format!(
            "Memory `{}` updated at `{}`.",
            result.memory_id,
            normalize_path(&result.path.display().to_string())
        ),
    }
}

fn workspace_hash(working_directory: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    normalize_path(&working_directory.display().to_string()).hash(&mut hasher);
    hasher.finish()
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
    use hellox_config::PermissionMode;

    use super::{
        capture_memory_from_session, capture_memory_from_snapshot, format_memory_list,
        list_memories, load_memory, MemoryScope,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-memory-{suffix}"));
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
            PermissionMode::AcceptEdits,
            None,
            None,
            false,
            Some(String::from("memory-session")),
        )
    }

    fn snapshot() -> StoredSessionSnapshot {
        StoredSessionSnapshot {
            session_id: String::from("stored-session"),
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
                "content": "remember the accepted architecture"
            }))
            .expect("message")],
        }
    }

    fn snapshot_with(
        session_id: &str,
        working_directory: &str,
        messages: Vec<serde_json::Value>,
    ) -> StoredSessionSnapshot {
        StoredSessionSnapshot {
            session_id: session_id.to_string(),
            model: String::from("opus"),
            permission_mode: Some(PermissionMode::AcceptEdits),
            output_style_name: None,
            output_style: None,
            persona: None,
            prompt_fragments: Vec::new(),
            config_path: None,
            planning: hellox_agent::PlanningState::default(),
            working_directory: working_directory.to_string(),
            shell_name: String::from("powershell"),
            system_prompt: String::from("system"),
            created_at: 1,
            updated_at: 2,
            agent_runtime: None,
            usage_by_model: Default::default(),
            messages: messages
                .into_iter()
                .map(|value| serde_json::from_value(value).expect("message"))
                .collect(),
        }
    }

    #[test]
    fn capture_memory_from_session_writes_session_and_project_markdown() {
        let root = temp_root();
        let session = session(&root);
        let result =
            capture_memory_from_session(&session, &root, Some("preserve current architecture"))
                .expect("capture memory");

        let markdown = fs::read_to_string(&result.path).expect("read session memory");
        assert!(markdown.contains("# hellox memory"));
        assert!(markdown.contains("- scope: session"));
        assert!(result.memory_id.starts_with("workspace-"));
        assert!(result
            .project_memory_id
            .as_deref()
            .is_some_and(|value| value.starts_with("project-")));
        assert!(markdown.contains("## Summary"));

        let project_path = result.project_path.expect("project path");
        let project_markdown = fs::read_to_string(project_path).expect("read project memory");
        assert!(project_markdown.contains("- scope: project"));
    }

    #[test]
    fn capture_memory_from_snapshot_supports_layered_list_and_show() {
        let root = temp_root();
        let result = capture_memory_from_snapshot(&snapshot(), &root, None).expect("capture");

        let entries = list_memories(&root).expect("list memories");
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.scope == MemoryScope::Session));
        assert!(entries
            .iter()
            .any(|entry| entry.scope == MemoryScope::Project));

        let list = format_memory_list(&entries);
        assert!(list.contains("scope"));
        assert!(list.contains(&result.memory_id));

        let markdown = load_memory(&root, &result.memory_id).expect("load session memory");
        assert!(markdown.contains("- source_session_id: stored-session"));
        assert!(markdown.contains("- working_directory: D:/workspace"));
        assert!(markdown.contains("## Key Points"));

        let project_memory_id = result.project_memory_id.expect("project memory id");
        let project_markdown = load_memory(&root, &project_memory_id).expect("load project memory");
        assert!(project_markdown.contains("- scope: project"));
    }

    #[test]
    fn project_memory_merges_recent_sections_across_sessions() {
        let root = temp_root();
        let working_directory = "D:\\workspace";
        let first = snapshot_with(
            "session-one",
            working_directory,
            vec![
                serde_json::json!({
                    "role": "user",
                    "content": "Accepted architecture is local-first Rust CLI."
                }),
                serde_json::json!({
                    "role": "assistant",
                    "content": "Still need to build the workflow panel."
                }),
            ],
        );
        let second = snapshot_with(
            "session-two",
            working_directory,
            vec![
                serde_json::json!({
                    "role": "user",
                    "content": "Risk: tmux host validation is still pending."
                }),
                serde_json::json!({
                    "role": "assistant",
                    "content": "Updated crates/hellox-cli/src/main.rs to support richer memory extraction."
                }),
            ],
        );

        let first_result =
            capture_memory_from_snapshot(&first, &root, None).expect("first capture");
        let second_result =
            capture_memory_from_snapshot(&second, &root, None).expect("second capture");

        let first_project_id = first_result.project_memory_id.expect("first project id");
        let second_project_id = second_result.project_memory_id.expect("second project id");
        assert_eq!(first_project_id, second_project_id);

        let project_markdown = load_memory(&root, &first_project_id).expect("load project memory");
        assert!(project_markdown.contains("Project memory rolls up accepted decisions"));
        assert!(project_markdown.contains("Accepted architecture is local-first Rust CLI"));
        assert!(project_markdown.contains("Still need to build the workflow panel"));
        assert!(project_markdown.contains("tmux host validation is still pending"));
        assert!(project_markdown.contains("crates/hellox-cli/src/main.rs"));
    }
}
