use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{HelloxConfig, PermissionMode};

use crate::memory::{list_memories, load_memory, MemoryScopeSelector};

use super::commands::{MemoryCommand, ReplCommand};
use super::format::help_text;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-memory-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session(root: PathBuf) -> AgentSession {
    AgentSession::create(
        GatewayClient::new("http://127.0.0.1:7821"),
        default_tool_registry(),
        root.join(".hellox").join("config.toml"),
        root,
        "powershell",
        AgentOptions::default(),
        PermissionMode::BypassPermissions,
        None,
        None,
        false,
        None,
    )
}

fn metadata(root: &PathBuf) -> ReplMetadata {
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: PathBuf::from("C:/Users/test/.hellox/config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

#[test]
fn parse_memory_commands() {
    assert_eq!(
        super::commands::parse_command("/memory"),
        Some(ReplCommand::Memory(MemoryCommand::Current))
    );
    assert_eq!(
        super::commands::parse_command("/memory panel"),
        Some(ReplCommand::Memory(MemoryCommand::Panel {
            archived: false,
            memory_id: None
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory panel session-abc"),
        Some(ReplCommand::Memory(MemoryCommand::Panel {
            archived: false,
            memory_id: Some(String::from("session-abc"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory panel --archived session-abc"),
        Some(ReplCommand::Memory(MemoryCommand::Panel {
            archived: true,
            memory_id: Some(String::from("session-abc"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory save preserve accepted decisions"),
        Some(ReplCommand::Memory(MemoryCommand::Save {
            instructions: Some(String::from("preserve accepted decisions"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory show session-abc"),
        Some(ReplCommand::Memory(MemoryCommand::Show {
            archived: false,
            memory_id: Some(String::from("session-abc"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory search accepted architecture"),
        Some(ReplCommand::Memory(MemoryCommand::Search {
            archived: false,
            query: Some(String::from("accepted architecture"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory clusters"),
        Some(ReplCommand::Memory(MemoryCommand::Clusters {
            archived: false,
            limit: 200,
            semantic: false,
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory clusters --archived --limit 50"),
        Some(ReplCommand::Memory(MemoryCommand::Clusters {
            archived: true,
            limit: 50,
            semantic: false,
        }))
    );
    assert_eq!(
        super::commands::parse_command("/memory clusters --semantic --limit 25"),
        Some(ReplCommand::Memory(MemoryCommand::Clusters {
            archived: false,
            limit: 25,
            semantic: true,
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/memory prune --scope project --older-than-days 14 --keep-latest 2 --apply"
        ),
        Some(ReplCommand::Memory(MemoryCommand::Prune {
            scope: MemoryScopeSelector::Project,
            older_than_days: 14,
            keep_latest: 2,
            apply: true,
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/memory archive --scope session --older-than-days 7 --keep-latest 1 --apply"
        ),
        Some(ReplCommand::Memory(MemoryCommand::Archive {
            scope: MemoryScopeSelector::Session,
            older_than_days: 7,
            keep_latest: 1,
            apply: true,
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/memory decay --scope session --older-than-days 90 --keep-latest 2 --max-summary-lines 12 --max-summary-chars 800 --apply"
        ),
        Some(ReplCommand::Memory(MemoryCommand::Decay {
            scope: MemoryScopeSelector::Session,
            older_than_days: 90,
            keep_latest: 2,
            max_summary_lines: 12,
            max_summary_chars: 800,
            apply: true,
        }))
    );
}

#[test]
fn help_text_lists_memory_commands() {
    let text = help_text();
    assert!(text.contains("/memory"));
    assert!(text.contains("/memory panel"));
    assert!(text.contains("/memory search [--archived] <query>"));
    assert!(text.contains("/memory clusters"));
    assert!(text.contains("/memory prune"));
    assert!(text.contains("/memory archive"));
    assert!(text.contains("/memory decay"));
    assert!(text.contains("/memory save [instructions]"));
}

#[test]
fn handle_memory_save_writes_and_lists_current_session_memory() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    let action = handle_repl_input(
        "/memory save preserve active implementation state",
        &mut session,
        &metadata,
    )
    .expect("memory save");
    assert_eq!(action, ReplAction::Continue);

    let entries = list_memories(&metadata.memory_root).expect("list memories");
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .any(|entry| entry.scope.as_str() == "session"));
    assert!(entries
        .iter()
        .any(|entry| entry.scope.as_str() == "project"));

    let session_memory_id = entries
        .iter()
        .find(|entry| entry.scope.as_str() == "session")
        .map(|entry| entry.memory_id.clone())
        .expect("session memory entry");
    let project_memory_id = entries
        .iter()
        .find(|entry| entry.scope.as_str() == "project")
        .map(|entry| entry.memory_id.clone())
        .expect("project memory entry");

    let session_memory =
        load_memory(&metadata.memory_root, &session_memory_id).expect("load session memory");
    assert!(session_memory.contains("# hellox memory"));
    assert!(session_memory.contains("preserve active implementation state"));

    let project_memory =
        load_memory(&metadata.memory_root, &project_memory_id).expect("load project memory");
    assert!(project_memory.contains("- scope: project"));
}

#[test]
fn compact_command_refreshes_memory_file() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    let action = handle_repl_input("/compact capture latest work", &mut session, &metadata)
        .expect("compact");
    assert_eq!(action, ReplAction::Continue);

    let entries = list_memories(&metadata.memory_root).expect("list memories");
    assert_eq!(entries.len(), 2);
}

#[test]
fn memory_panel_renders_selector_and_lens() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    handle_repl_input(
        "/memory save preserve active implementation state",
        &mut session,
        &metadata,
    )
    .expect("memory save");

    let entries = list_memories(&metadata.memory_root).expect("list memories");
    let memory_id = entries[0].memory_id.clone();

    let list = super::core_actions::handle_memory_command(
        MemoryCommand::Panel {
            archived: false,
            memory_id: None,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("memory panel list");
    assert!(list.contains("== Memory selector =="));
    assert!(list.contains("hellox memory panel"));

    let detail = super::core_actions::handle_memory_command(
        MemoryCommand::Panel {
            archived: false,
            memory_id: Some(memory_id.clone()),
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("memory panel detail");
    assert!(detail.contains("== Memory lens =="));
    assert!(detail.contains(&format!("show: `hellox memory show {memory_id}`")));
    assert!(detail.contains("markdown_lines"));
}

#[test]
fn memory_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let session_root = metadata.memory_root.join("sessions");
    let project_root = metadata.memory_root.join("projects");
    fs::create_dir_all(&session_root).expect("create session memory root");
    fs::create_dir_all(&project_root).expect("create project memory root");
    fs::write(session_root.join("session-alpha.md"), "# hellox memory").expect("write session");
    fs::write(project_root.join("project-beta.md"), "# hellox memory").expect("write project");

    let expected = list_memories(&metadata.memory_root)
        .expect("list memories")
        .into_iter()
        .take(20)
        .map(|entry| entry.memory_id)
        .collect::<Vec<_>>();
    assert!(!expected.is_empty());

    let driver = super::CliReplDriver::new();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("submit"),
                ReplAction::Submit(String::from("1"))
            );

            assert_eq!(
                driver
                    .handle_repl_input_async("/memory panel", &mut session, &metadata)
                    .await
                    .expect("open memory panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::MemoryPanelList {
                    archived,
                    memory_ids,
                }) => {
                    assert!(!archived);
                    assert_eq!(memory_ids, expected);
                }
                other => panic!("expected memory selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("select memory"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}

#[test]
fn memory_search_returns_ranked_hits_with_age_and_score() {
    let root = temp_dir();
    let session = session(root.clone());
    let metadata = metadata(&root);
    let session_root = metadata.memory_root.join("sessions");
    let project_root = metadata.memory_root.join("projects");
    fs::create_dir_all(&session_root).expect("create session memory root");
    fs::create_dir_all(&project_root).expect("create project memory root");
    fs::write(
        session_root.join("session-priority.md"),
        "# hellox memory\n\n## Summary\n\naccepted architecture remains active for the current implementation\n",
    )
    .expect("write session memory");
    fs::write(
        project_root.join("project-secondary.md"),
        "# hellox memory\n\nmetadata\narchitecture note\n",
    )
    .expect("write project memory");

    let text = super::core_actions::handle_memory_command(
        MemoryCommand::Search {
            archived: false,
            query: Some(String::from("accepted architecture")),
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("search memory");

    assert!(text.contains("memory_id\tscope\tage\tscore\tlocation\tpreview\tpath"));
    assert!(text.contains("session-priority"));
    assert!(text.contains("fresh"));
    assert!(text.contains("accepted architecture remains active"));
}

#[test]
fn memory_clusters_groups_entries_with_token_overlap() {
    let root = temp_dir();
    let session = session(root.clone());
    let metadata = metadata(&root);
    let session_root = metadata.memory_root.join("sessions");
    fs::create_dir_all(&session_root).expect("create session memory root");
    fs::write(
        session_root.join("session-a.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel remains pending\n\n## Pending Work\n\n- need to build workflow panel\n",
    )
    .expect("write session a");
    fs::write(
        session_root.join("session-b.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel wiring still missing\n\n## Pending Work\n\n- workflow panel pending work\n",
    )
    .expect("write session b");

    let text = super::core_actions::handle_memory_command(
        MemoryCommand::Clusters {
            archived: false,
            limit: 50,
            semantic: false,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("memory clusters");

    assert!(text.contains("Clustered"));
    assert!(text.contains("cluster_id"));
    assert!(text.contains("session-a"));
    assert!(text.contains("session-b"));
}

#[test]
fn memory_clusters_support_semantic_mode() {
    let root = temp_dir();
    let session = session(root.clone());
    let metadata = metadata(&root);
    let session_root = metadata.memory_root.join("sessions");
    fs::create_dir_all(&session_root).expect("create session memory root");
    fs::write(
        session_root.join("session-a.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel remains pending\n\n## Pending Work\n\n- need to build workflow panel\n",
    )
    .expect("write session a");
    fs::write(
        session_root.join("session-b.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel wiring still missing\n\n## Pending Work\n\n- workflow panel pending work\n",
    )
    .expect("write session b");

    let text = super::core_actions::handle_memory_command(
        MemoryCommand::Clusters {
            archived: false,
            limit: 50,
            semantic: true,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("memory clusters semantic");

    assert!(text.contains("mode: tfidf_cosine"));
    assert!(text.contains("session-a"));
    assert!(text.contains("session-b"));
}

#[test]
fn memory_prune_previews_and_applies_scope_filtered_retention() {
    let root = temp_dir();
    let session = session(root.clone());
    let metadata = metadata(&root);
    let session_root = metadata.memory_root.join("sessions");
    let project_root = metadata.memory_root.join("projects");
    fs::create_dir_all(&session_root).expect("create session memory root");
    fs::create_dir_all(&project_root).expect("create project memory root");
    fs::write(session_root.join("session-a-keep.md"), "# hellox memory").expect("write keep");
    fs::write(session_root.join("session-z-prune.md"), "# hellox memory").expect("write prune");
    fs::write(project_root.join("project-a-keep.md"), "# hellox memory").expect("write keep");
    fs::write(project_root.join("project-z-keep.md"), "# hellox memory").expect("write keep");

    let preview = super::core_actions::handle_memory_command(
        MemoryCommand::Prune {
            scope: MemoryScopeSelector::Session,
            older_than_days: 0,
            keep_latest: 1,
            apply: false,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("preview prune");
    assert!(preview.contains("Would prune 1 stale memory file(s)"));
    assert!(preview.contains("session-z-prune"));
    assert!(load_memory(&metadata.memory_root, "session-z-prune").is_ok());

    let applied = super::core_actions::handle_memory_command(
        MemoryCommand::Prune {
            scope: MemoryScopeSelector::Session,
            older_than_days: 0,
            keep_latest: 1,
            apply: true,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("apply prune");
    assert!(applied.contains("Pruned 1 stale memory file(s)"));
    assert!(load_memory(&metadata.memory_root, "session-z-prune").is_err());
    assert!(load_memory(&metadata.memory_root, "project-z-keep").is_ok());
}

#[test]
fn memory_archive_previews_and_applies_scope_filtered_retention() {
    let root = temp_dir();
    let session = session(root.clone());
    let metadata = metadata(&root);
    let session_root = metadata.memory_root.join("sessions");
    let project_root = metadata.memory_root.join("projects");
    fs::create_dir_all(&session_root).expect("create session memory root");
    fs::create_dir_all(&project_root).expect("create project memory root");
    fs::write(session_root.join("session-a-keep.md"), "# hellox memory").expect("write keep");
    fs::write(session_root.join("session-z-archive.md"), "# hellox memory").expect("write archive");
    fs::write(project_root.join("project-a-keep.md"), "# hellox memory").expect("write keep");
    fs::write(project_root.join("project-z-keep.md"), "# hellox memory").expect("write keep");

    let preview = super::core_actions::handle_memory_command(
        MemoryCommand::Archive {
            scope: MemoryScopeSelector::Session,
            older_than_days: 0,
            keep_latest: 1,
            apply: false,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("preview archive");
    assert!(preview.contains("Would archive 1 stale memory file(s)"));
    assert!(preview.contains("session-z-archive"));
    assert!(load_memory(&metadata.memory_root, "session-z-archive").is_ok());

    let applied = super::core_actions::handle_memory_command(
        MemoryCommand::Archive {
            scope: MemoryScopeSelector::Session,
            older_than_days: 0,
            keep_latest: 1,
            apply: true,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("apply archive");
    assert!(applied.contains("Archived 1 stale memory file(s)"));
    assert!(load_memory(&metadata.memory_root, "session-z-archive").is_ok());
    assert_eq!(
        list_memories(&metadata.memory_root)
            .expect("list memories")
            .len(),
        3
    );
    assert!(load_memory(&metadata.memory_root, "project-z-keep").is_ok());
}

#[test]
fn memory_decay_previews_and_applies_summary_truncation_for_archived_memories() {
    let root = temp_dir();
    let session = session(root.clone());
    let metadata = metadata(&root);
    let archive_root = metadata.memory_root.join("archive").join("sessions");
    fs::create_dir_all(&archive_root).expect("create archive root");
    fs::write(
        archive_root.join("session-a-keep.md"),
        "# hellox memory\n\n## Summary\n\nshort summary\n\n## Key Points\n\n- ok\n",
    )
    .expect("write keep");
    let long = (0..40)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        archive_root.join("session-z-decay.md"),
        format!("# hellox memory\n\n## Summary\n\n{long}\n\n## Key Points\n\n- ok\n"),
    )
    .expect("write decay");

    let preview = super::core_actions::handle_memory_command(
        MemoryCommand::Decay {
            scope: MemoryScopeSelector::Session,
            older_than_days: 0,
            keep_latest: 1,
            max_summary_lines: 2,
            max_summary_chars: 80,
            apply: false,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("preview decay");
    assert!(preview.contains("Would decay 1 stale archived memory file(s)"));
    assert!(preview.contains("session-z-decay"));

    let applied = super::core_actions::handle_memory_command(
        MemoryCommand::Decay {
            scope: MemoryScopeSelector::Session,
            older_than_days: 0,
            keep_latest: 1,
            max_summary_lines: 2,
            max_summary_chars: 80,
            apply: true,
        },
        &session,
        &metadata,
        crate::startup::AppLanguage::English,
    )
    .expect("apply decay");
    assert!(applied.contains("Decayed 1 stale archived memory file(s)"));

    let markdown =
        fs::read_to_string(archive_root.join("session-z-decay.md")).expect("read decayed");
    assert!(markdown.contains("## Summary"));
    assert!(markdown.contains("..."));
}
