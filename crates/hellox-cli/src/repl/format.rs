use std::path::Path;

use hellox_agent::AgentSession;
use hellox_config::{default_config_path, load_or_default, HelloxConfig};

use crate::diagnostics::{
    cost_text as workspace_cost_text, doctor_text as workspace_doctor_text, gather_workspace_stats,
    stats_text as workspace_stats_text, status_text as workspace_status_text,
    usage_text as workspace_usage_text,
};
use crate::repl::ReplMetadata;
use crate::search::{
    format_search_results, merge_search_hits, search_current_session, search_memories,
    search_sessions,
};
use crate::sessions::{format_session_detail, format_session_list, list_sessions, load_session};
use crate::workflows::list_invocable_workflows;

#[cfg(test)]
pub(super) fn help_text() -> String {
    render_help_text(None)
}

pub(super) fn help_text_for_workdir(root: &Path) -> String {
    match list_invocable_workflows(root) {
        Ok(workflows) if !workflows.is_empty() => render_help_text(Some(
            &workflows
                .iter()
                .map(|workflow| workflow.name.clone())
                .collect::<Vec<_>>(),
        )),
        Ok(_) => render_help_text(None),
        Err(error) => {
            let mut text = render_help_text(None);
            text.push_str(&format!("\nworkflow_discovery_error: {error}"));
            text
        }
    }
}

fn render_help_text(workflow_names: Option<&[String]>) -> String {
    let mut lines = [
        "Available slash commands:",
        "Core local workflow:",
        "  /help                Show this help message",
        "  /status              Show current REPL status",
        "  /doctor              Inspect local config, providers, and storage health",
        "  /usage               Show persisted workspace activity totals",
        "  /stats               Show detailed persisted workspace statistics",
        "  /cost                Show estimated persisted token cost",
        "  /brief               Show the current workspace brief",
        "  /brief set <message> Store a local brief for the current workspace",
        "  /brief clear         Remove the current workspace brief",
        "  /tools <query>       Search available local tools by name or description",
        "  /plan                Show current planning state for this session",
        "  /plan panel [n]      Show a plan dashboard or focus one accepted step",
        "  /plan enter          Enable plan mode for this session",
        "  /plan add [--index <n>] <status>:<text> Insert or append an accepted plan step",
        "  /plan update <n> <status>:<text> Replace an accepted plan step",
        "  /plan remove <n>     Remove an accepted plan step",
        "  /plan allow <prompt> Add an allowed follow-up prompt",
        "  /plan disallow <prompt> Remove an allowed follow-up prompt",
        "  /plan exit --step <status>:<text>... Store an accepted plan and leave plan mode",
        "  /plan clear          Clear accepted plan state for this session",
        "  /install             Show local install status and stable target paths",
        "  /install plan [source] [target] Plan an offline local install",
        "  /install apply [source] [target] [--force] Copy a local binary into the stable install target",
        "  /upgrade             Show local upgrade status",
        "  /upgrade plan <source> [target] Plan an offline local upgrade",
        "  /upgrade apply <source> [target] [--backup] [--force] Replace the stable local install",
        "  /output-style        Show active, default, and discovered output styles",
        "  /output-style panel [name] Show an output-style dashboard or inspect one style",
        "  /output-style use <name> Apply an output style to the current session",
        "  /persona             Show active, default, and discovered personas",
        "  /persona panel [name] Show a persona dashboard or inspect one persona",
        "  /persona use <name>  Apply a persona to the current session",
        "  /fragment            Show active, default, and discovered prompt fragments",
        "  /fragment panel [name] Show a prompt-fragment dashboard or inspect one fragment",
        "  /fragment use <name> [name...] Apply prompt fragments to the current session",
        "  /search <query>      Search transcript, persisted sessions, and memory",
        "  /skills [name]       List skills or show a skill definition",
        "  /hooks [name]        List hooks or show a hook definition",
        "  /workflow            List project workflow scripts",
        "  /workflow dashboard [name] Open the interactive workflow dashboard shell",
        "  /workflow overview [name] Show a selector-style workflow overview",
        "  /workflow panel [name] [n] Show an authoring panel with copyable edit actions",
        "  /workflow panel --script-path <path> [n] Open an explicit workflow script",
        "  /workflow runs [name] List recorded workflow runs",
        "  /workflow runs --script-path <path> List recorded runs for one explicit script",
        "  /workflow validate [name] Validate workflow scripts",
        "  /workflow validate --script-path <path> Validate one explicit workflow script",
        "  /workflow show-run <id> [n] Show a recorded workflow run",
        "  /workflow last-run [name] [n] Show the latest recorded workflow run",
        "  /workflow last-run --script-path <path> [n] Show the latest recorded run for one explicit script",
        "  /workflow show <name> Show a workflow script definition",
        "  /workflow show --script-path <path> Show one explicit workflow script",
        "  /workflow init <name> Create a starter workflow script",
        "  /workflow add-step <name> --prompt <text> Add a workflow step",
        "  /workflow add-step --script-path <path> --prompt <text> Add a step to an explicit script",
        "  /workflow update-step <name> <n> ... Edit a workflow step",
        "  /workflow update-step --script-path <path> <n> ... Edit an explicit workflow script",
        "  /workflow duplicate-step <name> <n> [--to <m>] Duplicate a workflow step",
        "  /workflow duplicate-step --script-path <path> <n> [--to <m>] Duplicate a step in an explicit workflow script",
        "  /workflow move-step <name> <n> --to <m> Reorder a workflow step",
        "  /workflow move-step --script-path <path> <n> --to <m> Reorder a step in an explicit workflow script",
        "  /workflow remove-step <name> <n> Remove a workflow step",
        "  /workflow remove-step --script-path <path> <n> Remove a step from an explicit workflow script",
        "  /workflow set-shared-context <name> <text> Set workflow shared context",
        "  /workflow set-shared-context --script-path <path> <text> Set shared context on an explicit workflow script",
        "  /workflow clear-shared-context <name> Clear workflow shared context",
        "  /workflow clear-shared-context --script-path <path> Clear shared context on an explicit workflow script",
        "  /workflow enable-continue-on-error <name> Enable continue_on_error",
        "  /workflow enable-continue-on-error --script-path <path> Enable continue_on_error on an explicit workflow script",
        "  /workflow disable-continue-on-error <name> Disable continue_on_error",
        "  /workflow disable-continue-on-error --script-path <path> Disable continue_on_error on an explicit workflow script",
        "  /workflow run <name> [shared_context] Run a workflow script locally",
        "  /workflow run --script-path <path> [shared_context] Run one explicit workflow script",
        "Local integration:",
        "  /bridge              Show local bridge runtime status",
        "  /bridge sessions     List persisted bridge sessions",
        "  /bridge show <id>    Show bridge session details",
        "  /ide                 Show IDE-facing bridge status",
        "  /mcp                 List configured MCP servers",
        "  /mcp panel [name]    Show an MCP dashboard or inspect one server",
        "  /mcp show <name>     Show a configured MCP server",
        "  /mcp tools <name>    List tools exposed by a configured MCP server",
        "  /mcp call <name> <tool> [json] Call an MCP tool with optional JSON input",
        "  /mcp resources <name> List MCP resources exposed by a configured server",
        "  /mcp prompts <name>  List MCP prompts exposed by a configured server",
        "  /mcp read-resource <name> <uri> Read a resource from a configured MCP server",
        "  /mcp get-prompt <name> <prompt> [json] Fetch an MCP prompt with optional JSON input",
        "  /mcp auth ...        Show, set, or clear MCP bearer-token helper state",
        "  /mcp auth oauth-set <name> <client-id> <authorize-url> <token-url> <redirect-url> [scope...] Configure OAuth client settings",
        "  /mcp auth oauth-start <name> Start an OAuth PKCE flow for a configured MCP server",
        "  /mcp auth oauth-exchange <name> <code> <verifier> Exchange an OAuth code for tokens",
        "  /mcp auth oauth-refresh <name> Refresh a linked MCP OAuth access token",
        "  /mcp auth oauth-clear <name> Clear the linked MCP OAuth account",
        "  /mcp registry list [cursor] [limit] Browse the official MCP registry",
        "  /mcp registry show <name> Show the latest registry metadata for a server",
        "  /mcp registry install <name> [server-name] [scope] Install a registry server into local config",
        "  /mcp add ...         Add a stdio, SSE, or WS MCP server",
        "  /mcp enable <name>   Enable a configured MCP server",
        "  /mcp disable <name>  Disable a configured MCP server",
        "  /mcp remove <name>   Remove a configured MCP server",
        "  /plugin              List installed plugins",
        "  /plugin panel [id]   Show a plugin dashboard or inspect one plugin",
        "  /plugin show <id>    Show an installed plugin",
        "  /plugin install <path> Install a local plugin package",
        "  /plugin enable <id>  Enable an installed plugin",
        "  /plugin disable <id> Disable an installed plugin",
        "  /plugin remove <id>  Remove an installed plugin",
        "  /plugin marketplace  List configured marketplaces",
        "Optional remote-capable commands:",
        "  /remote-env          List configured remote environments",
        "  /remote-env add ...  Add a remote environment profile",
        "  /teleport plan ...   Build a direct-connect teleport plan",
        "  /teleport connect .. Create a remote direct-connect session",
        "  /assistant           List assistant-viewable sessions",
        "  /assistant show <id> Show assistant session details",
        "  /memory              Show current session memory or fall back to project memory",
        "  /memory panel [--archived] [id] Show a memory dashboard or inspect one memory file",
        "  /memory list [--archived] List captured memory files",
        "  /memory show [--archived] <id> Show a captured memory file",
        "  /memory search [--archived] <query> Search memory files with relevance and age ranking",
        "  /memory clusters [--archived] [--limit <n>] [--semantic] Cluster memory files by token overlap or TF-IDF cosine similarity",
        "  /memory prune [--scope <all|session|project>] [--older-than-days <n>] [--keep-latest <n>] [--apply]",
        "  /memory archive [--scope <all|session|project>] [--older-than-days <n>] [--keep-latest <n>] [--apply]",
        "  /memory decay [--scope <all|session|project>] [--older-than-days <n>] [--keep-latest <n>] [--max-summary-lines <n>] [--max-summary-chars <n>] [--apply]",
        "  /memory save [instructions] Capture memory from the current session",
        "  /session             Show current session metadata",
        "  /session panel [id]  Show a session dashboard or inspect one persisted session",
        "  /session list        List persisted sessions",
        "  /session show <id>   Show persisted session details",
        "  /session share <id> [path] Export a persisted session transcript",
        "  /tasks               List workspace tasks",
        "  /tasks panel [id]    Show a task dashboard or inspect one task",
        "  /tasks add <text>    Add a workspace task",
        "  /tasks show <id>     Show a single workspace task",
        "  /tasks update <id> --status <value> [--output <text>] ... Update task fields",
        "  /tasks output <id>   Show the latest stored task output",
        "  /tasks stop <id> [reason] Cancel a task and optionally record a reason",
        "  /tasks start <id>    Mark a task in progress",
        "  /tasks done <id>     Mark a task completed",
        "  /tasks cancel <id>   Mark a task cancelled",
        "  /tasks remove <id>   Delete a task",
        "  /tasks clear <mode>  Clear `completed` or `all` tasks",
        "  /model               Show the active session model",
        "  /model panel [name]  Show a model dashboard or inspect one profile",
        "  /model list          List configured model profiles",
        "  /model show [name]   Show the current or named model profile",
        "  /model use <name>    Switch the active session model",
        "  /model default <name> Persist the default model profile",
        "  /permissions [mode] Show or switch the permission mode",
        "  /config              Show active config path and resolved config",
        "  /config panel [key]  Show a config dashboard or focus one resolved key",
        "  /config path|keys    Show active config path or supported writable keys",
        "  /config set <key> <value> Update a supported config key",
        "  /config clear <key>  Clear a supported optional config key",
        "  /resume [session-id] List resumable sessions or switch to one",
        "  /share [path]        Export the current transcript as Markdown",
        "  /compact [instructions] Replace history with a summary context",
        "  /rewind             Remove the most recent conversation turn",
        "  /clear               Clear the current conversation history",
        "  /exit                Exit the REPL",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>();

    if let Some(workflow_names) = workflow_names {
        lines.push("Project workflow commands:".to_string());
        for workflow_name in workflow_names {
            lines.push(format!(
                "  /{} [shared_context] Run project workflow `{}`",
                workflow_name, workflow_name
            ));
        }
    }

    lines.join("\n")
}

pub(super) fn session_text(session: &AgentSession) -> String {
    let planning = session.planning_state();
    format!(
        "session_id: {}\nmodel: {}\npermission_mode: {}\noutput_style: {}\npersona: {}\nprompt_fragments: {}\nplan_mode: {}\nplan_steps: {}\nworking_directory: {}\nmessages: {}\nmax_turns: {}",
        session.session_id().unwrap_or("(ephemeral)"),
        session.model(),
        session.permission_mode(),
        session.output_style_name().unwrap_or("(none)"),
        session.persona_name().unwrap_or("(none)"),
        render_names(session.prompt_fragment_names()),
        planning.active,
        planning.plan.len(),
        format_path(session.working_directory()),
        session.message_count(),
        session.max_turns()
    )
}

pub(super) fn status_text(session: &AgentSession, metadata: &ReplMetadata) -> String {
    let (config, reload_note) = runtime_config(metadata);
    let text = match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => workspace_status_text(
            session.working_directory(),
            &metadata.config_path,
            &config,
            &stats,
        ),
        Err(error) => format!(
            "{}\nconfig_path: {}\nstatus_error: {}",
            session_text(session),
            format_path(&metadata.config_path),
            error
        ),
    };
    append_note(
        format!(
            "{text}\nactive_output_style: {}\nactive_persona: {}\nactive_prompt_fragments: {}",
            session.output_style_name().unwrap_or("(none)"),
            session.persona_name().unwrap_or("(none)"),
            render_names(session.prompt_fragment_names())
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

pub(super) fn resume_help_text(metadata: &ReplMetadata) -> String {
    match list_sessions(&metadata.sessions_root) {
        Ok(sessions) if !sessions.is_empty() => format!(
            "Use `/resume <session-id>` to switch sessions.\n\n{}",
            format_session_list(&sessions)
        ),
        Ok(_) => "No persisted sessions found. Start a session with persistence enabled first."
            .to_string(),
        Err(error) => format!("Unable to inspect persisted sessions: {error}"),
    }
}

pub(super) fn permissions_text(session: &AgentSession) -> String {
    format!(
        "Current permission mode: `{}`.\nUse `/permissions <mode>` to switch. Available modes: {}",
        session.permission_mode(),
        hellox_config::PermissionMode::supported_values().join(", ")
    )
}

pub(super) fn session_list_text(metadata: &ReplMetadata) -> String {
    match list_sessions(&metadata.sessions_root) {
        Ok(sessions) => format_session_list(&sessions),
        Err(error) => format!("Unable to inspect persisted sessions: {error}"),
    }
}

pub(super) fn session_detail_text(metadata: &ReplMetadata, session_id: &str) -> String {
    match load_session(&metadata.sessions_root, session_id) {
        Ok(snapshot) => format_session_detail(&snapshot),
        Err(error) => format!("Unable to load `{session_id}`: {error}"),
    }
}

pub(super) fn search_text(
    session: &AgentSession,
    metadata: &ReplMetadata,
    query: &str,
    limit: usize,
) -> String {
    let session_hits = match search_sessions(&metadata.sessions_root, query, limit) {
        Ok(hits) => hits,
        Err(error) => return format!("Unable to search persisted sessions: {error}"),
    };
    let memory_hits = match search_memories(&metadata.memory_root, query, limit) {
        Ok(hits) => hits,
        Err(error) => return format!("Unable to search memory files: {error}"),
    };
    let transcript_hits = search_current_session(session, query, limit);
    let hits = merge_search_hits(limit, vec![transcript_hits, session_hits, memory_hits]);
    format_search_results(query, &hits)
}

pub(super) fn doctor_text(session: &AgentSession, metadata: &ReplMetadata) -> String {
    let (config, reload_note) = runtime_config(metadata);
    let text = workspace_doctor_text(session.working_directory(), &metadata.config_path, &config)
        .unwrap_or_else(|error| format!("Unable to inspect workspace health: {error}"));
    append_note(text, reload_note)
}

pub(super) fn usage_text(session: &AgentSession) -> String {
    match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => workspace_usage_text(&stats),
        Err(error) => format!("Unable to inspect workspace usage: {error}"),
    }
}

pub(super) fn stats_text(session: &AgentSession) -> String {
    match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => workspace_stats_text(&stats),
        Err(error) => format!("Unable to inspect workspace stats: {error}"),
    }
}

pub(super) fn cost_text(session: &AgentSession) -> String {
    match gather_workspace_stats(session.working_directory()) {
        Ok(stats) => match load_or_default(Some(default_config_path())) {
            Ok(config) => workspace_cost_text(&stats, &config),
            Err(error) => format!("Unable to load config for cost inspection: {error}"),
        },
        Err(error) => format!("Unable to inspect cost state: {error}"),
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
            Some(format!(
                "config_reload_error: {}",
                error.to_string().replace('\\', "/")
            )),
        ),
    }
}

fn append_note(text: String, note: Option<String>) -> String {
    match note {
        Some(note) => format!("{text}\n{note}"),
        None => text,
    }
}

fn render_names(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    }
}
