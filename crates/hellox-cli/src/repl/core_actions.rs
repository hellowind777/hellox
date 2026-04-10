use std::path::{Path, PathBuf};

use anyhow::Result;
use hellox_agent::{AgentSession, CompactMode, OutputStylePrompt};
use hellox_config::{load_or_default, PermissionMode};

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
use crate::output_styles::{
    discover_output_styles, format_output_style_detail, format_output_style_list, load_output_style,
};
use crate::repl::format::{
    permissions_text, resume_help_text, session_detail_text, session_list_text, session_text,
};
use crate::repl::ReplMetadata;
use crate::session_panel::render_session_panel;
use crate::sessions::load_session;
use crate::settings_commands::model_command_text;
use crate::style_panels::render_output_style_panel;
use crate::transcript::{
    default_share_path, export_session_markdown, export_stored_session_markdown,
};

use super::commands::{MemoryCommand, ModelCommand, OutputStyleCommand, SessionCommand};

pub(super) enum ResumeAction {
    Continue(String),
    Resume(String),
}

pub(super) fn handle_memory_command(
    command: MemoryCommand,
    session: &AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    match command {
        MemoryCommand::Current => {
            match load_memory(&metadata.memory_root, &current_memory_id(session)) {
                Ok(memory) => Ok(memory),
                Err(_) => match load_memory(
                    &metadata.memory_root,
                    &current_project_memory_id(session),
                ) {
                    Ok(memory) => Ok(memory),
                    Err(_) => Ok(
                        "No captured session or project memory found for the current workspace."
                            .to_string(),
                    ),
                },
            }
        }
        MemoryCommand::Panel {
            archived,
            memory_id,
        } => Ok(
            match render_memory_panel(&metadata.memory_root, archived, memory_id.as_deref()) {
                Ok(panel) => panel,
                Err(error) => format!("Unable to render memory panel: {error}"),
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
        } => Ok("Usage: /memory show <memory-id>".to_string()),
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
                Err(error) => Ok(format!("Unable to load memory `{memory_id}`: {error}")),
            }
        }
        MemoryCommand::Search {
            archived: _,
            query: None,
        } => Ok("Usage: /memory search <query>".to_string()),
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
            Ok(format!(
                "Captured layered memory using {} mode. {}",
                compact_mode_label(result.mode),
                memory_result_targets(&result)
            ))
        }
    }
}

pub(super) fn handle_session_command(
    command: SessionCommand,
    session: &AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    match command {
        SessionCommand::Current => Ok(session_text(session)),
        SessionCommand::Panel { session_id } => Ok(
            match render_session_panel(&metadata.sessions_root, session_id.as_deref()) {
                Ok(panel) => panel,
                Err(error) => format!("Unable to render session panel: {error}"),
            },
        ),
        SessionCommand::List => Ok(session_list_text(metadata)),
        SessionCommand::Show { session_id: None } => {
            Ok("Usage: /session show <session-id>".to_string())
        }
        SessionCommand::Show {
            session_id: Some(session_id),
        } => Ok(session_detail_text(metadata, &session_id)),
        SessionCommand::Share {
            session_id: None, ..
        } => Ok("Usage: /session share <session-id> [path]".to_string()),
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
                Ok(format!(
                    "Shared transcript written to `{}`.",
                    format_path(&destination)
                ))
            }
            Err(error) => Ok(format!("Unable to share `{session_id}`: {error}")),
        },
    }
}

pub(super) fn handle_output_style_command(
    command: OutputStyleCommand,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    let config = runtime_config(metadata);

    match command {
        OutputStyleCommand::Panel { style_name } => Ok(
            match render_output_style_panel(
                &metadata.config_path,
                session.working_directory(),
                config.output_style.default.as_deref(),
                session.output_style_name(),
                style_name.as_deref(),
            ) {
                Ok(panel) => panel,
                Err(error) => format!("Unable to render output style panel: {error}"),
            },
        ),
        OutputStyleCommand::Current | OutputStyleCommand::List => {
            match discover_output_styles(session.working_directory()) {
                Ok(styles) => Ok(format_output_style_overview(
                    &styles,
                    session,
                    config.output_style.default.as_deref(),
                )),
                Err(error) => Ok(format!("Unable to inspect output styles: {error}")),
            }
        }
        OutputStyleCommand::Show { style_name } => {
            let Some(style_name) = style_name
                .or_else(|| session.output_style_name().map(ToString::to_string))
                .or_else(|| config.output_style.default.clone())
            else {
                return Ok("Usage: /output-style show <name>".to_string());
            };

            match load_output_style(&style_name, session.working_directory()) {
                Ok(style) => Ok(format_output_style_detail(
                    &style,
                    config.output_style.default.as_deref() == Some(style.name.as_str()),
                    session.output_style_name() == Some(style.name.as_str()),
                )),
                Err(error) => Ok(format!(
                    "Unable to load output style `{style_name}`: {error}"
                )),
            }
        }
        OutputStyleCommand::Use { style_name: None } => {
            Ok("Usage: /output-style use <name>".to_string())
        }
        OutputStyleCommand::Use {
            style_name: Some(style_name),
        } => match load_output_style(&style_name, session.working_directory()) {
            Ok(style) => {
                session.set_output_style(Some(OutputStylePrompt {
                    name: style.name.clone(),
                    prompt: style.prompt,
                }))?;
                Ok(format!(
                    "Active output style set to `{}` for the current session.",
                    style.name
                ))
            }
            Err(error) => Ok(format!(
                "Unable to load output style `{style_name}`: {error}"
            )),
        },
        OutputStyleCommand::Clear => match session.output_style_name() {
            Some(active_style) => {
                let active_style = active_style.to_string();
                session.set_output_style(None)?;
                Ok(format!(
                    "Cleared active output style `{active_style}` for the current session."
                ))
            }
            None => Ok("No active output style is set for the current session.".to_string()),
        },
        OutputStyleCommand::Help => Ok(output_style_help_text().to_string()),
    }
}

pub(super) fn handle_permissions_command(
    value: Option<String>,
    session: &mut AgentSession,
) -> Result<String> {
    match value {
        Some(value) => match value.parse::<PermissionMode>() {
            Ok(mode) => {
                session.set_permission_mode(mode.clone())?;
                Ok(format!("Permission mode set to `{mode}`."))
            }
            Err(error) => Ok(error),
        },
        None => Ok(permissions_text(session)),
    }
}

pub(super) fn handle_resume_command(
    session_id: Option<String>,
    metadata: &ReplMetadata,
) -> Result<ResumeAction> {
    match session_id {
        None => Ok(ResumeAction::Continue(resume_help_text(metadata))),
        Some(session_id) => match load_session(&metadata.sessions_root, &session_id) {
            Ok(_) => Ok(ResumeAction::Resume(session_id)),
            Err(error) => Ok(ResumeAction::Continue(format!(
                "Unable to resume `{session_id}`: {error}"
            ))),
        },
    }
}

pub(super) fn handle_share_command(
    path: Option<String>,
    session: &AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    let destination = resolve_share_path(
        path.as_deref(),
        session.working_directory(),
        &metadata.shares_root,
        session.session_id(),
    );
    export_session_markdown(session, &destination)?;
    Ok(format!(
        "Shared transcript written to `{}`.",
        format_path(&destination)
    ))
}

pub(super) fn handle_compact_command(
    instructions: Option<String>,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
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
        Ok("No conversation history to compact.".to_string())
    } else {
        Ok(format!(
            "Compacted current session in {} mode: {} -> {} message(s). {}",
            compact_mode_label(result.mode),
            result.original_message_count,
            result.retained_message_count,
            memory_result_targets(&memory_result)
        ))
    }
}

pub(super) fn handle_rewind_command(session: &mut AgentSession) -> Result<String> {
    let removed = session.rewind_last_turn()?;
    if removed == 0 {
        Ok("No conversation turn to rewind.".to_string())
    } else {
        Ok(format!(
            "Rewound the most recent turn ({removed} message(s) removed)."
        ))
    }
}

pub(super) fn handle_model_command(
    command: ModelCommand,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    match command {
        ModelCommand::Current => Ok(format!("Current model: `{}`", session.model())),
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
                    Err(error) => format!("Unable to render model panel: {error}"),
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
            Ok(format!("Model set to `{model}`."))
        }
        ModelCommand::Use { value: None } => Ok("Usage: /model use <name>".to_string()),
        ModelCommand::Default {
            profile_name: Some(profile_name),
        } => render_model_command(crate::cli_types::ModelCommands::SetDefault {
            profile_name,
            config: Some(metadata.config_path.clone()),
        }),
        ModelCommand::Default { profile_name: None } => {
            Ok("Usage: /model default <name>".to_string())
        }
        ModelCommand::Help => Ok(model_help_text().to_string()),
    }
}

fn model_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  /model                 Show the current session model\n",
        "  /model panel [name]    Show a model dashboard or inspect one profile\n",
        "  /model list            List configured model profiles\n",
        "  /model show [name]     Show the current or named model profile\n",
        "  /model use <name>      Switch the current session model\n",
        "  /model default <name>  Persist the default model profile"
    )
}

fn render_model_command(command: crate::cli_types::ModelCommands) -> Result<String> {
    match model_command_text(command) {
        Ok(text) => Ok(text),
        Err(error) => Ok(error.to_string()),
    }
}

fn resolve_share_path(
    value: Option<&str>,
    working_directory: &Path,
    shares_root: &Path,
    session_id: Option<&str>,
) -> PathBuf {
    match value {
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                working_directory.join(path)
            }
        }
        None => default_share_path(shares_root, session_id),
    }
}

fn runtime_config(metadata: &ReplMetadata) -> hellox_config::HelloxConfig {
    load_or_default(Some(metadata.config_path.clone())).unwrap_or_else(|_| metadata.config.clone())
}

fn format_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn compact_mode_label(mode: CompactMode) -> &'static str {
    match mode {
        CompactMode::Micro => "microcompact",
        CompactMode::Full => "compact",
    }
}

fn format_output_style_overview(
    styles: &[crate::output_styles::OutputStyleDefinition],
    session: &AgentSession,
    default_style: Option<&str>,
) -> String {
    format!(
        "active_output_style: {}\ndefault_output_style: {}\nworkspace_root: {}\n\n{}",
        session.output_style_name().unwrap_or("(none)"),
        default_style.unwrap_or("(none)"),
        format_path(session.working_directory()),
        format_output_style_list(styles, default_style)
    )
}

fn output_style_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  /output-style              Show active, default, and discovered output styles\n",
        "  /output-style panel [name] Show an output-style dashboard or inspect one style\n",
        "  /output-style list         List discovered output styles\n",
        "  /output-style show <name>  Show a style prompt\n",
        "  /output-style use <name>   Apply a style to the current session\n",
        "  /output-style clear        Clear the active session style"
    )
}
