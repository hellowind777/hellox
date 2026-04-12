use anyhow::Result;
use hellox_agent::AgentSession;
use hellox_config::PermissionMode;

use crate::memory::{
    archive_memories, capture_memory_from_session, cluster_memories, current_memory_id,
    current_project_memory_id, decay_archived_memories, format_memory_archive_report,
    format_memory_cluster_report, format_memory_decay_report, format_memory_list,
    format_memory_prune_report, format_memory_search_results, list_archived_memories,
    list_memories, load_archived_memory, load_memory, memory_result_targets, prune_memories,
    search_archived_memories_ranked, search_memories_ranked, write_memory_from_session_summary,
    MemoryArchiveOptions, MemoryClusterOptions, MemoryDecayOptions, MemoryPruneOptions,
};
use crate::memory_panel::render_memory_panel;
use crate::model_panel::render_model_panel;
use crate::repl::core_copy::{
    captured_memory_text, compacted_session_text, current_model_text, model_help_text,
    model_set_text, no_history_to_compact_text, no_turn_to_rewind_text, no_workspace_memory_text,
    rewound_turn_text, shared_transcript_written_text, unable_to_load_memory_text,
    unable_to_render_memory_panel_text, unable_to_render_model_panel_text,
    unable_to_render_session_panel_text, unable_to_share_session_text, usage_text,
};
use crate::repl::core_paths::{
    compact_mode_label, format_path, resolve_share_path, runtime_config,
};
use crate::repl::format::{
    permissions_text, resume_help_text, session_detail_text, session_list_text, session_text,
};
use crate::repl::ReplMetadata;
use crate::session_panel::render_session_panel;
use crate::sessions::load_session;
use crate::settings_commands::model_command_text;
use crate::startup::AppLanguage;
use crate::transcript::{export_session_markdown, export_stored_session_markdown};

use super::commands::{MemoryCommand, ModelCommand, SessionCommand};

pub(super) enum ResumeAction {
    Continue(String),
    Resume(String),
}

pub(super) fn handle_memory_command(
    command: MemoryCommand,
    session: &AgentSession,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> Result<String> {
    match command {
        MemoryCommand::Current => {
            match load_memory(&metadata.memory_root, &current_memory_id(session)) {
                Ok(memory) => Ok(memory),
                Err(_) => {
                    match load_memory(&metadata.memory_root, &current_project_memory_id(session)) {
                        Ok(memory) => Ok(memory),
                        Err(_) => Ok(no_workspace_memory_text(language)),
                    }
                }
            }
        }
        MemoryCommand::Panel {
            archived,
            memory_id,
        } => Ok(
            match render_memory_panel(&metadata.memory_root, archived, memory_id.as_deref()) {
                Ok(panel) => panel,
                Err(error) => unable_to_render_memory_panel_text(language, &error),
            },
        ),
        MemoryCommand::List { archived } => {
            let entries = if archived {
                list_archived_memories(&metadata.memory_root)?
            } else {
                list_memories(&metadata.memory_root)?
            };
            Ok(format_memory_list(&entries))
        }
        MemoryCommand::Show {
            archived: _,
            memory_id: None,
        } => Ok(usage_text(language, "/memory show <memory-id>")),
        MemoryCommand::Show {
            archived,
            memory_id: Some(memory_id),
        } => {
            let markdown = if archived {
                load_archived_memory(&metadata.memory_root, &memory_id)
            } else {
                load_memory(&metadata.memory_root, &memory_id)
            };

            match markdown {
                Ok(memory) => Ok(memory),
                Err(error) => Ok(unable_to_load_memory_text(language, &memory_id, &error)),
            }
        }
        MemoryCommand::Search {
            archived: _,
            query: None,
        } => Ok(usage_text(language, "/memory search <query>")),
        MemoryCommand::Search {
            archived,
            query: Some(query),
        } => {
            let hits = if archived {
                search_archived_memories_ranked(&metadata.memory_root, &query, 20)?
            } else {
                search_memories_ranked(&metadata.memory_root, &query, 20)?
            };
            Ok(format_memory_search_results(&query, &hits))
        }
        MemoryCommand::Clusters {
            archived,
            limit,
            semantic,
        } => {
            let report = cluster_memories(
                &metadata.memory_root,
                &MemoryClusterOptions {
                    archived,
                    limit,
                    semantic,
                    ..MemoryClusterOptions::default()
                },
            )?;
            Ok(format_memory_cluster_report(&report))
        }
        MemoryCommand::Prune {
            scope,
            older_than_days,
            keep_latest,
            apply,
        } => {
            let report = prune_memories(
                &metadata.memory_root,
                &MemoryPruneOptions {
                    scope,
                    older_than_days,
                    keep_latest,
                    apply,
                },
            )?;
            Ok(format_memory_prune_report(&report))
        }
        MemoryCommand::Archive {
            scope,
            older_than_days,
            keep_latest,
            apply,
        } => {
            let report = archive_memories(
                &metadata.memory_root,
                &MemoryArchiveOptions {
                    scope,
                    older_than_days,
                    keep_latest,
                    apply,
                },
            )?;
            Ok(format_memory_archive_report(&report))
        }
        MemoryCommand::Decay {
            scope,
            older_than_days,
            keep_latest,
            max_summary_lines,
            max_summary_chars,
            apply,
        } => {
            let report = decay_archived_memories(
                &metadata.memory_root,
                &MemoryDecayOptions {
                    scope,
                    older_than_days,
                    keep_latest,
                    max_summary_lines,
                    max_summary_chars,
                    apply,
                },
            )?;
            Ok(format_memory_decay_report(&report))
        }
        MemoryCommand::Save { instructions } => {
            let result = capture_memory_from_session(
                session,
                &metadata.memory_root,
                instructions.as_deref(),
            )?;
            Ok(captured_memory_text(
                language,
                compact_mode_label(result.mode, language),
                &memory_result_targets(&result),
            ))
        }
    }
}

pub(super) fn handle_session_command(
    command: SessionCommand,
    session: &AgentSession,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> Result<String> {
    match command {
        SessionCommand::Current => Ok(session_text(session, language)),
        SessionCommand::Panel { session_id } => Ok(
            match render_session_panel(&metadata.sessions_root, session_id.as_deref()) {
                Ok(panel) => panel,
                Err(error) => unable_to_render_session_panel_text(language, &error),
            },
        ),
        SessionCommand::List => Ok(session_list_text(metadata, language)),
        SessionCommand::Show { session_id: None } => {
            Ok(usage_text(language, "/session show <session-id>"))
        }
        SessionCommand::Show {
            session_id: Some(session_id),
        } => Ok(session_detail_text(metadata, &session_id, language)),
        SessionCommand::Share {
            session_id: None, ..
        } => Ok(usage_text(language, "/session share <session-id> [path]")),
        SessionCommand::Share {
            session_id: Some(session_id),
            path,
        } => match load_session(&metadata.sessions_root, &session_id) {
            Ok(snapshot) => {
                let destination = resolve_share_path(
                    path.as_deref(),
                    session.working_directory(),
                    &metadata.shares_root,
                    Some(snapshot.session_id.as_str()),
                );
                export_stored_session_markdown(&snapshot, &destination)?;
                Ok(shared_transcript_written_text(
                    language,
                    &format_path(&destination),
                ))
            }
            Err(error) => Ok(unable_to_share_session_text(language, &session_id, &error)),
        },
    }
}

pub(super) fn handle_permissions_command(
    value: Option<String>,
    session: &mut AgentSession,
    language: AppLanguage,
) -> Result<String> {
    match value {
        Some(value) => match value.parse::<PermissionMode>() {
            Ok(mode) => {
                session.set_permission_mode(mode.clone())?;
                Ok(match language {
                    AppLanguage::English => format!("Permission mode set to `{mode}`."),
                    AppLanguage::SimplifiedChinese => format!("权限模式已切换为 `{mode}`。"),
                })
            }
            Err(error) => Ok(error),
        },
        None => Ok(permissions_text(session, language)),
    }
}

pub(super) fn handle_resume_command(
    session_id: Option<String>,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> Result<ResumeAction> {
    match session_id {
        None => Ok(ResumeAction::Continue(resume_help_text(metadata, language))),
        Some(session_id) => match load_session(&metadata.sessions_root, &session_id) {
            Ok(_) => Ok(ResumeAction::Resume(session_id)),
            Err(error) => Ok(ResumeAction::Continue(format!(
                "{}: {error}",
                match language {
                    AppLanguage::English => format!("Unable to resume `{session_id}`"),
                    AppLanguage::SimplifiedChinese => format!("无法恢复会话 `{session_id}`"),
                }
            ))),
        },
    }
}

pub(super) fn handle_share_command(
    path: Option<String>,
    session: &AgentSession,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> Result<String> {
    let destination = resolve_share_path(
        path.as_deref(),
        session.working_directory(),
        &metadata.shares_root,
        session.session_id(),
    );
    export_session_markdown(session, &destination)?;
    Ok(shared_transcript_written_text(
        language,
        &format_path(&destination),
    ))
}

pub(super) fn handle_compact_command(
    instructions: Option<String>,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> Result<String> {
    let result = session.compact(instructions.as_deref())?;
    let memory_result = write_memory_from_session_summary(
        session,
        &metadata.memory_root,
        &result,
        instructions.as_deref(),
        None,
    )?;

    if result.original_message_count == 0 {
        Ok(no_history_to_compact_text(language))
    } else {
        Ok(compacted_session_text(
            language,
            compact_mode_label(result.mode, language),
            result.original_message_count,
            result.retained_message_count,
            &memory_result_targets(&memory_result),
        ))
    }
}

pub(super) fn handle_rewind_command(
    session: &mut AgentSession,
    language: AppLanguage,
) -> Result<String> {
    let removed = session.rewind_last_turn()?;
    if removed == 0 {
        Ok(no_turn_to_rewind_text(language))
    } else {
        Ok(rewound_turn_text(language, removed))
    }
}

pub(super) fn handle_model_command(
    command: ModelCommand,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> Result<String> {
    match command {
        ModelCommand::Current => Ok(current_model_text(language, session.model())),
        ModelCommand::Panel { profile_name } => {
            let config = runtime_config(metadata);
            Ok(
                match render_model_panel(
                    &metadata.config_path,
                    &config,
                    profile_name.as_deref(),
                    Some(session.model()),
                ) {
                    Ok(panel) => panel,
                    Err(error) => unable_to_render_model_panel_text(language, &error),
                },
            )
        }
        ModelCommand::List => render_model_command(crate::cli_types::ModelCommands::List {
            config: Some(metadata.config_path.clone()),
        }),
        ModelCommand::Show { profile_name } => {
            render_model_command(crate::cli_types::ModelCommands::Show {
                profile_name,
                config: Some(metadata.config_path.clone()),
            })
        }
        ModelCommand::Use { value: Some(model) } => {
            session.set_model(model.clone())?;
            Ok(model_set_text(language, &model))
        }
        ModelCommand::Use { value: None } => Ok(usage_text(language, "/model use <name>")),
        ModelCommand::Default {
            profile_name: Some(profile_name),
        } => render_model_command(crate::cli_types::ModelCommands::SetDefault {
            profile_name,
            config: Some(metadata.config_path.clone()),
        }),
        ModelCommand::Default { profile_name: None } => {
            Ok(usage_text(language, "/model default <name>"))
        }
        ModelCommand::Help => Ok(model_help_text(language).to_string()),
    }
}

fn render_model_command(command: crate::cli_types::ModelCommands) -> Result<String> {
    match model_command_text(command) {
        Ok(text) => Ok(text),
        Err(error) => Ok(error.to_string()),
    }
}
