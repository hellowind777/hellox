use std::path::Path;

use hellox_agent::AgentSession;
use hellox_config::{default_config_path, load_or_default, HelloxConfig};

use crate::diagnostics::{
    cost_text as workspace_cost_text, doctor_text as workspace_doctor_text, gather_workspace_stats,
    stats_text as workspace_stats_text, status_text as workspace_status_text,
    usage_text as workspace_usage_text,
};
use crate::repl::format_copy::*;
use crate::repl::help_copy::{core_workflow_lines, local_integration_lines, remote_capable_lines};
use crate::repl::ReplMetadata;
use crate::search::{
    format_search_results, merge_search_hits, search_current_session, search_memories,
    search_sessions,
};
use crate::sessions::{format_session_detail, format_session_list, list_sessions, load_session};
use crate::startup::AppLanguage;
use crate::workflows::list_invocable_workflows;

#[cfg(test)]
pub(super) fn help_text() -> String {
    render_help_text(None, AppLanguage::English)
}

pub(super) fn help_text_for_workdir(root: &Path, language: AppLanguage) -> String {
    match list_invocable_workflows(root) {
        Ok(workflows) if !workflows.is_empty() => render_help_text(
            Some(
                &workflows
                    .iter()
                    .map(|workflow| workflow.name.clone())
                    .collect::<Vec<_>>(),
            ),
            language,
        ),
        Ok(_) => render_help_text(None, language),
        Err(error) => {
            let mut text = render_help_text(None, language);
            text.push_str(&format!(
                "\n{}: {error}",
                workflow_discovery_error_label(language)
            ));
            text
        }
    }
}

fn render_help_text(workflow_names: Option<&[String]>, language: AppLanguage) -> String {
    let mut lines = vec![
        help_title(language).to_string(),
        help_core_local_workflow_title(language).to_string(),
    ];
    lines.extend(
        core_workflow_lines(language)
            .iter()
            .map(|line| (*line).to_string()),
    );
    lines.push(help_local_integration_title(language).to_string());
    lines.extend(
        local_integration_lines(language)
            .iter()
            .map(|line| (*line).to_string()),
    );
    lines.push(help_remote_commands_title(language).to_string());
    lines.extend(
        remote_capable_lines(language)
            .iter()
            .map(|line| (*line).to_string()),
    );

    if let Some(workflow_names) = workflow_names {
        lines.push(project_workflow_commands_title(language).to_string());
        for workflow_name in workflow_names {
            lines.push(format!(
                "{}",
                project_workflow_command_line(language, workflow_name)
            ));
        }
    }

    lines.join("\n")
}

pub(super) fn session_text(session: &AgentSession, language: AppLanguage) -> String {
    let planning = session.planning_state();
    match language {
        AppLanguage::English => format!(
            "session_id: {}\nmodel: {}\npermission_mode: {}\noutput_style: {}\npersona: {}\nprompt_fragments: {}\nplan_mode: {}\nplan_steps: {}\nworking_directory: {}\nmessages: {}\nmax_turns: {}",
            session.session_id().unwrap_or("(ephemeral)"),
            session.model(),
            session.permission_mode(),
            session.output_style_name().unwrap_or("(none)"),
            session.persona_name().unwrap_or("(none)"),
            render_names(session.prompt_fragment_names(), language),
            planning.active,
            planning.plan.len(),
            format_path(session.working_directory()),
            session.message_count(),
            session.max_turns()
        ),
        AppLanguage::SimplifiedChinese => format!(
            "会话 ID：{}\n模型：{}\n权限模式：{}\n输出风格：{}\n人设：{}\n提示片段：{}\n计划模式：{}\n计划步骤：{}\n工作目录：{}\n消息数：{}\n最大轮次：{}",
            session.session_id().unwrap_or("（临时会话）"),
            session.model(),
            session.permission_mode(),
            session.output_style_name().unwrap_or("（无）"),
            session.persona_name().unwrap_or("（无）"),
            render_names(session.prompt_fragment_names(), language),
            planning.active,
            planning.plan.len(),
            format_path(session.working_directory()),
            session.message_count(),
            session.max_turns()
        ),
    }
}

pub(super) fn status_text(
    session: &AgentSession,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> String {
    let (config, reload_note) = runtime_config(metadata);
    let text = match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => workspace_status_text(
            session.working_directory(),
            &metadata.config_path,
            &config,
            &stats,
            language,
        ),
        Err(error) => format!(
            "{}\n{}: {}\n{}: {}",
            session_text(session, language),
            config_path_label(language),
            format_path(&metadata.config_path),
            status_error_label(language),
            error
        ),
    };
    append_note(
        format!(
            "{text}\n{}\n{}\n{}",
            field_line(
                language,
                active_output_style_label(language),
                session.output_style_name().unwrap_or(none_text(language)),
            ),
            field_line(
                language,
                active_persona_label(language),
                session.persona_name().unwrap_or(none_text(language)),
            ),
            field_line(
                language,
                active_prompt_fragments_label(language),
                &render_names(session.prompt_fragment_names(), language),
            )
        ),
        reload_note,
    )
}

#[cfg(test)]
pub(super) fn config_text(metadata: &ReplMetadata) -> String {
    let (config, reload_note) = runtime_config(metadata);
    let rendered = toml::to_string_pretty(&config)
        .unwrap_or_else(|error| format!("failed to render config: {error}"));
    append_note(
        format!(
            "config_path: {}\n\n{}",
            format_path(&metadata.config_path),
            rendered
        ),
        reload_note,
    )
}

pub(super) fn resume_help_text(metadata: &ReplMetadata, language: AppLanguage) -> String {
    match list_sessions(&metadata.sessions_root) {
        Ok(sessions) if !sessions.is_empty() => format!(
            "{}\n\n{}",
            resume_usage_text(language),
            format_session_list(&sessions)
        ),
        Ok(_) => no_persisted_sessions_text(language).to_string(),
        Err(error) => format!("{}: {error}", unable_to_inspect_sessions_label(language)),
    }
}

pub(super) fn permissions_text(session: &AgentSession, language: AppLanguage) -> String {
    match language {
        AppLanguage::English => format!(
            "Current permission mode: `{}`.\nUse `/permissions <mode>` to switch. Available modes: {}",
            session.permission_mode(),
            hellox_config::PermissionMode::supported_values().join(", ")
        ),
        AppLanguage::SimplifiedChinese => format!(
            "当前权限模式：`{}`。\n使用 `/permissions <mode>` 可切换。可用模式：{}",
            session.permission_mode(),
            hellox_config::PermissionMode::supported_values().join(", ")
        ),
    }
}

pub(super) fn session_list_text(metadata: &ReplMetadata, language: AppLanguage) -> String {
    match list_sessions(&metadata.sessions_root) {
        Ok(sessions) => format_session_list(&sessions),
        Err(error) => format!("{}: {error}", unable_to_inspect_sessions_label(language)),
    }
}

pub(super) fn session_detail_text(
    metadata: &ReplMetadata,
    session_id: &str,
    language: AppLanguage,
) -> String {
    match load_session(&metadata.sessions_root, session_id) {
        Ok(snapshot) => format_session_detail(&snapshot),
        Err(error) => format!("{} `{session_id}`: {error}", unable_to_load_label(language)),
    }
}

pub(super) fn search_text(
    session: &AgentSession,
    metadata: &ReplMetadata,
    query: &str,
    limit: usize,
    language: AppLanguage,
) -> String {
    let session_hits = match search_sessions(&metadata.sessions_root, query, limit) {
        Ok(hits) => hits,
        Err(error) => {
            return format!(
                "{}: {error}",
                unable_to_search_persisted_sessions_label(language)
            )
        }
    };
    let memory_hits = match search_memories(&metadata.memory_root, query, limit) {
        Ok(hits) => hits,
        Err(error) => return format!("{}: {error}", unable_to_search_memory_files_label(language)),
    };
    let transcript_hits = search_current_session(session, query, limit);
    let hits = merge_search_hits(limit, vec![transcript_hits, session_hits, memory_hits]);
    format_search_results(query, &hits)
}

pub(super) fn doctor_text(
    session: &AgentSession,
    metadata: &ReplMetadata,
    language: AppLanguage,
) -> String {
    let (config, reload_note) = runtime_config(metadata);
    let text = workspace_doctor_text(
        session.working_directory(),
        &metadata.config_path,
        &config,
        language,
    )
    .unwrap_or_else(|error| {
        format!(
            "{}: {error}",
            unable_to_inspect_workspace_health_label(language)
        )
    });
    append_note(text, reload_note)
}

pub(super) fn usage_text(session: &AgentSession, language: AppLanguage) -> String {
    match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => workspace_usage_text(&stats, language),
        Err(error) => format!(
            "{}: {error}",
            unable_to_inspect_workspace_usage_label(language)
        ),
    }
}

pub(super) fn stats_text(session: &AgentSession, language: AppLanguage) -> String {
    match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => workspace_stats_text(&stats, language),
        Err(error) => format!(
            "{}: {error}",
            unable_to_inspect_workspace_stats_label(language)
        ),
    }
}

pub(super) fn cost_text(session: &AgentSession, language: AppLanguage) -> String {
    match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => match load_or_default(Some(default_config_path())) {
            Ok(config) => workspace_cost_text(&stats, &config, language),
            Err(error) => format!(
                "{}: {error}",
                unable_to_load_config_for_cost_label(language)
            ),
        },
        Err(error) => format!("{}: {error}", unable_to_inspect_cost_state_label(language)),
    }
}

fn format_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn runtime_config(metadata: &ReplMetadata) -> (HelloxConfig, Option<String>) {
    match load_or_default(Some(metadata.config_path.clone())) {
        Ok(config) => (config, None),
        Err(error) => (
            metadata.config.clone(),
            Some(error.to_string().replace('\\', "/")),
        ),
    }
}

fn append_note(text: String, note: Option<String>) -> String {
    match note {
        Some(note) => format!("{text}\n{note}"),
        None => text,
    }
}

fn render_names(names: &[String], language: AppLanguage) -> String {
    if names.is_empty() {
        none_text(language).to_string()
    } else {
        names.join(", ")
    }
}
