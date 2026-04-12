use hellox_memory::MemoryScopeSelector;

use crate::command_types::{
    BridgeCommand, BriefCommand, ConfigCommand, IdeCommand, InstallCommand, MarketplaceCommand,
    McpCommand, MemoryCommand, ModelCommand, PluginCommand, ReplCommand, SessionCommand,
    TaskCommand, ToolsCommand, UpgradeCommand,
};
use crate::plan_command_parser::parse_plan_command;
use crate::remote_command_parser::{
    parse_assistant_command, parse_remote_env_command, parse_teleport_command,
};
use crate::style_parser::{
    parse_output_style_command, parse_persona_command, parse_prompt_fragment_command,
};
use crate::workflow_command_parser::parse_workflow_command;

pub fn parse_command(input: &str) -> Option<ReplCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed[1..].split_whitespace().peekable();
    let name = parts.next().unwrap_or_default().to_ascii_lowercase();
    let remainder = parts.collect::<Vec<_>>().join(" ");

    let command = match name.as_str() {
        "" => ReplCommand::Help,
        "help" => ReplCommand::Help,
        "status" => ReplCommand::Status,
        "doctor" => ReplCommand::Doctor,
        "usage" => ReplCommand::Usage,
        "stats" => ReplCommand::Stats,
        "cost" => ReplCommand::Cost,
        "brief" => ReplCommand::Brief(parse_brief_command(&remainder)),
        "tools" => ReplCommand::Tools(parse_tools_command(&remainder)),
        "plan" => ReplCommand::Plan(parse_plan_command(&remainder)),
        "install" => ReplCommand::Install(parse_install_command(&remainder)),
        "upgrade" => ReplCommand::Upgrade(parse_upgrade_command(&remainder)),
        "output-style" | "outputstyle" => {
            ReplCommand::OutputStyle(parse_output_style_command(&remainder))
        }
        "persona" => ReplCommand::Persona(parse_persona_command(&remainder)),
        "fragment" | "prompt-fragment" | "promptfragment" => {
            ReplCommand::PromptFragment(parse_prompt_fragment_command(&remainder))
        }
        "search" => ReplCommand::Search {
            query: (!remainder.is_empty()).then_some(remainder),
        },
        "skills" => ReplCommand::Skills {
            name: (!remainder.is_empty()).then_some(remainder),
        },
        "hooks" => ReplCommand::Hooks {
            name: (!remainder.is_empty()).then_some(remainder),
        },
        "remote-env" => ReplCommand::RemoteEnv(parse_remote_env_command(&remainder)),
        "teleport" => ReplCommand::Teleport(parse_teleport_command(&remainder)),
        "assistant" => ReplCommand::Assistant(parse_assistant_command(&remainder)),
        "bridge" => ReplCommand::Bridge(parse_bridge_command(&remainder)),
        "ide" => ReplCommand::Ide(parse_ide_command(&remainder)),
        "mcp" => ReplCommand::Mcp(parse_mcp_command(&remainder)),
        "plugin" => ReplCommand::Plugin(parse_plugin_command(&remainder)),
        "memory" => ReplCommand::Memory(parse_memory_command(&remainder)),
        "session" => ReplCommand::Session(parse_session_command(&remainder)),
        "tasks" => ReplCommand::Tasks(parse_task_command(&remainder)),
        "workflow" => ReplCommand::Workflow(parse_workflow_command(&remainder)),
        "config" | "settings" => ReplCommand::Config(parse_config_command(&remainder)),
        "permissions" | "permission" => ReplCommand::Permissions {
            value: (!remainder.is_empty()).then_some(remainder),
        },
        "resume" => ReplCommand::Resume {
            session_id: (!remainder.is_empty()).then_some(remainder),
        },
        "share" => ReplCommand::Share {
            path: (!remainder.is_empty()).then_some(remainder),
        },
        "compact" => ReplCommand::Compact {
            instructions: (!remainder.is_empty()).then_some(remainder),
        },
        "rewind" => ReplCommand::Rewind,
        "clear" => ReplCommand::Clear,
        "exit" | "quit" => ReplCommand::Exit,
        "model" => ReplCommand::Model(parse_model_command(&remainder)),
        _ => ReplCommand::Unknown(name),
    };

    Some(command)
}

fn parse_brief_command(remainder: &str) -> BriefCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => BriefCommand::Show,
        Some(action) if action == "show" => BriefCommand::Show,
        Some(action) if action == "set" => {
            let message = parts.collect::<Vec<_>>().join(" ");
            BriefCommand::Set {
                message: (!message.is_empty()).then_some(message),
            }
        }
        Some(action) if action == "clear" => BriefCommand::Clear,
        Some(_) => BriefCommand::Help,
    }
}

fn parse_tools_command(remainder: &str) -> ToolsCommand {
    let mut parts = remainder.split_whitespace();
    let first = parts.next();
    let second = parts.next();
    let third = parts.next();

    match (first, second, third) {
        (None, _, _) => ToolsCommand::Help,
        (Some("search"), Some(query), maybe_limit) => ToolsCommand::Search {
            query: Some(query.to_string()),
            limit: maybe_limit
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(20),
        },
        (Some("search"), None, _) => ToolsCommand::Search {
            query: None,
            limit: 20,
        },
        (Some(query), maybe_limit, _) => ToolsCommand::Search {
            query: Some(query.to_string()),
            limit: maybe_limit
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(20),
        },
    }
}

fn parse_config_command(remainder: &str) -> ConfigCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => ConfigCommand::Show,
        Some(action) if action == "show" => ConfigCommand::Show,
        Some(action) if action == "panel" => ConfigCommand::Panel {
            focus_key: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "path" => ConfigCommand::Path,
        Some(action) if action == "keys" => ConfigCommand::Keys,
        Some(action) if action == "set" => {
            let key = parts.next().map(ToString::to_string);
            let value = {
                let remainder = parts.collect::<Vec<_>>().join(" ");
                (!remainder.is_empty()).then_some(remainder)
            };
            ConfigCommand::Set { key, value }
        }
        Some(action) if action == "clear" => ConfigCommand::Clear {
            key: parts.next().map(ToString::to_string),
        },
        Some(_) => ConfigCommand::Help,
    }
}

fn parse_model_command(remainder: &str) -> ModelCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => ModelCommand::Current,
        Some(action) if action == "panel" => ModelCommand::Panel {
            profile_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "list" => ModelCommand::List,
        Some(action) if action == "show" => ModelCommand::Show {
            profile_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "use" => ModelCommand::Use {
            value: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "default" => ModelCommand::Default {
            profile_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "help" => ModelCommand::Help,
        Some(value) => ModelCommand::Use { value: Some(value) },
    }
}

fn parse_install_command(remainder: &str) -> InstallCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => InstallCommand::Status,
        Some(action) if action == "status" => InstallCommand::Status,
        Some(action) if action == "plan" => InstallCommand::Plan {
            source: parts.next().map(ToString::to_string),
            target: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "apply" => {
            let remaining = parts.collect::<Vec<_>>();
            let force = remaining.iter().any(|part| *part == "--force");
            let values = remaining
                .into_iter()
                .filter(|part| *part != "--force")
                .collect::<Vec<_>>();
            InstallCommand::Apply {
                source: values.first().map(|value| (*value).to_string()),
                target: values.get(1).map(|value| (*value).to_string()),
                force,
            }
        }
        Some(_) => InstallCommand::Help,
    }
}

fn parse_upgrade_command(remainder: &str) -> UpgradeCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => UpgradeCommand::Status,
        Some(action) if action == "status" => UpgradeCommand::Status,
        Some(action) if action == "plan" => UpgradeCommand::Plan {
            source: parts.next().map(ToString::to_string),
            target: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "apply" => {
            let remaining = parts.collect::<Vec<_>>();
            let backup = remaining.iter().any(|part| *part == "--backup");
            let force = remaining.iter().any(|part| *part == "--force");
            let values = remaining
                .into_iter()
                .filter(|part| *part != "--backup" && *part != "--force")
                .collect::<Vec<_>>();
            UpgradeCommand::Apply {
                source: values.first().map(|value| (*value).to_string()),
                target: values.get(1).map(|value| (*value).to_string()),
                backup,
                force,
            }
        }
        Some(_) => UpgradeCommand::Help,
    }
}

fn parse_bridge_command(remainder: &str) -> BridgeCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => BridgeCommand::Status,
        Some(action) if action == "status" => BridgeCommand::Status,
        Some(action) if action == "panel" => BridgeCommand::Panel {
            session_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "sessions" => BridgeCommand::Sessions,
        Some(action) if action == "show" => BridgeCommand::Show {
            session_id: parts.next().map(ToString::to_string),
        },
        Some(_) => BridgeCommand::Help,
    }
}

fn parse_ide_command(remainder: &str) -> IdeCommand {
    match remainder.split_whitespace().next() {
        None => IdeCommand::Status,
        Some(action) if action.eq_ignore_ascii_case("status") => IdeCommand::Status,
        Some(action) if action.eq_ignore_ascii_case("panel") => IdeCommand::Panel,
        Some(_) => IdeCommand::Help,
    }
}

fn parse_session_command(remainder: &str) -> SessionCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => SessionCommand::Current,
        Some(action) if action == "panel" => SessionCommand::Panel {
            session_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "list" => SessionCommand::List,
        Some(action) if action == "show" => SessionCommand::Show {
            session_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "share" => {
            let session_id = parts.next().map(ToString::to_string);
            let path = {
                let remainder = parts.collect::<Vec<_>>().join(" ");
                (!remainder.is_empty()).then_some(remainder)
            };
            SessionCommand::Share { session_id, path }
        }
        Some(_) => SessionCommand::Current,
    }
}

fn parse_memory_command(remainder: &str) -> MemoryCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => MemoryCommand::Current,
        Some(action) if action == "panel" => {
            let (archived, memory_id) = parse_archived_flag_and_value(parts.collect::<Vec<_>>());
            MemoryCommand::Panel {
                archived,
                memory_id,
            }
        }
        Some(action) if action == "list" => {
            let archived = parts.any(|part| part.eq_ignore_ascii_case("--archived"));
            MemoryCommand::List { archived }
        }
        Some(action) if action == "show" => {
            let (archived, memory_id) = parse_archived_flag_and_value(parts.collect::<Vec<_>>());
            MemoryCommand::Show {
                archived,
                memory_id,
            }
        }
        Some(action) if action == "search" => {
            let mut archived = false;
            let mut tokens = Vec::new();
            for part in parts {
                if part.eq_ignore_ascii_case("--archived") {
                    archived = true;
                    continue;
                }
                tokens.push(part);
            }
            let remainder = tokens.join(" ");
            MemoryCommand::Search {
                archived,
                query: (!remainder.is_empty()).then_some(remainder),
            }
        }
        Some(action) if action == "clusters" => {
            parse_memory_clusters_command(parts.collect::<Vec<_>>())
        }
        Some(action) if action == "prune" => parse_memory_prune_command(parts.collect::<Vec<_>>()),
        Some(action) if action == "archive" => {
            parse_memory_archive_command(parts.collect::<Vec<_>>())
        }
        Some(action) if action == "decay" => parse_memory_decay_command(parts.collect::<Vec<_>>()),
        Some(action) if action == "save" => {
            let remainder = parts.collect::<Vec<_>>().join(" ");
            MemoryCommand::Save {
                instructions: (!remainder.is_empty()).then_some(remainder),
            }
        }
        Some(_) => MemoryCommand::Current,
    }
}

fn parse_archived_flag_and_value(parts: Vec<&str>) -> (bool, Option<String>) {
    let mut archived = false;
    let mut value = None;

    for part in parts {
        if part.eq_ignore_ascii_case("--archived") {
            archived = true;
            continue;
        }

        if value.is_none() {
            value = Some(part.to_string());
        }
    }

    (archived, value)
}

fn parse_memory_clusters_command(parts: Vec<&str>) -> MemoryCommand {
    let mut archived = false;
    let mut limit = 200_usize;
    let mut semantic = false;
    let mut index = 0;

    while index < parts.len() {
        match parts[index] {
            "--archived" => {
                archived = true;
                index += 1;
            }
            "--semantic" => {
                semantic = true;
                index += 1;
            }
            "--limit" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    limit = value;
                }
                index += 2;
            }
            _ => index += 1,
        }
    }

    MemoryCommand::Clusters {
        archived,
        limit,
        semantic,
    }
}

fn parse_memory_prune_command(parts: Vec<&str>) -> MemoryCommand {
    let mut scope = MemoryScopeSelector::All;
    let mut older_than_days = 30_u64;
    let mut keep_latest = 3_usize;
    let mut apply = false;
    let mut index = 0;

    while index < parts.len() {
        match parts[index] {
            "--apply" => {
                apply = true;
                index += 1;
            }
            "--scope" => {
                if let Some(value) = parts.get(index + 1) {
                    scope = parse_memory_scope_selector(value).unwrap_or(MemoryScopeSelector::All);
                }
                index += 2;
            }
            "--older-than-days" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    older_than_days = value;
                }
                index += 2;
            }
            "--keep-latest" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    keep_latest = value;
                }
                index += 2;
            }
            _ => index += 1,
        }
    }

    MemoryCommand::Prune {
        scope,
        older_than_days,
        keep_latest,
        apply,
    }
}

fn parse_memory_archive_command(parts: Vec<&str>) -> MemoryCommand {
    let mut scope = MemoryScopeSelector::All;
    let mut older_than_days = 30_u64;
    let mut keep_latest = 3_usize;
    let mut apply = false;
    let mut index = 0;

    while index < parts.len() {
        match parts[index] {
            "--apply" => {
                apply = true;
                index += 1;
            }
            "--scope" => {
                if let Some(value) = parts.get(index + 1) {
                    scope = parse_memory_scope_selector(value).unwrap_or(MemoryScopeSelector::All);
                }
                index += 2;
            }
            "--older-than-days" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    older_than_days = value;
                }
                index += 2;
            }
            "--keep-latest" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    keep_latest = value;
                }
                index += 2;
            }
            _ => index += 1,
        }
    }

    MemoryCommand::Archive {
        scope,
        older_than_days,
        keep_latest,
        apply,
    }
}

fn parse_memory_decay_command(parts: Vec<&str>) -> MemoryCommand {
    let mut scope = MemoryScopeSelector::All;
    let mut older_than_days = 180_u64;
    let mut keep_latest = 20_usize;
    let mut max_summary_lines = 24_usize;
    let mut max_summary_chars = 1600_usize;
    let mut apply = false;
    let mut index = 0;

    while index < parts.len() {
        match parts[index] {
            "--apply" => {
                apply = true;
                index += 1;
            }
            "--scope" => {
                if let Some(value) = parts.get(index + 1) {
                    scope = parse_memory_scope_selector(value).unwrap_or(MemoryScopeSelector::All);
                }
                index += 2;
            }
            "--older-than-days" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    older_than_days = value;
                }
                index += 2;
            }
            "--keep-latest" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    keep_latest = value;
                }
                index += 2;
            }
            "--max-summary-lines" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    max_summary_lines = value;
                }
                index += 2;
            }
            "--max-summary-chars" => {
                if let Some(value) = parts.get(index + 1).and_then(|value| value.parse().ok()) {
                    max_summary_chars = value;
                }
                index += 2;
            }
            _ => index += 1,
        }
    }

    MemoryCommand::Decay {
        scope,
        older_than_days,
        keep_latest,
        max_summary_lines,
        max_summary_chars,
        apply,
    }
}

fn parse_memory_scope_selector(value: &str) -> Option<MemoryScopeSelector> {
    match value.to_ascii_lowercase().as_str() {
        "all" => Some(MemoryScopeSelector::All),
        "session" => Some(MemoryScopeSelector::Session),
        "project" => Some(MemoryScopeSelector::Project),
        _ => None,
    }
}

fn parse_task_command(remainder: &str) -> TaskCommand {
    let trimmed = remainder.trim();
    let mut parts = trimmed.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => TaskCommand::List,
        Some(action) if action == "list" => TaskCommand::List,
        Some(action) if action == "panel" => TaskCommand::Panel {
            task_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "add" => {
            let content = parts.collect::<Vec<_>>().join(" ");
            TaskCommand::Add {
                content: (!content.is_empty()).then_some(content),
            }
        }
        Some(action) if action == "show" => TaskCommand::Show {
            task_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "update" => parse_task_update_command(trimmed),
        Some(action) if action == "output" => TaskCommand::Output {
            task_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "stop" => {
            let task_id = parts.next().map(ToString::to_string);
            let reason = {
                let remainder = parts.collect::<Vec<_>>().join(" ");
                (!remainder.is_empty()).then_some(remainder)
            };
            TaskCommand::Stop { task_id, reason }
        }
        Some(action) if action == "start" => TaskCommand::Start {
            task_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "done" => TaskCommand::Done {
            task_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "cancel" => TaskCommand::Cancel {
            task_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "remove" => TaskCommand::Remove {
            task_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "clear" => TaskCommand::Clear {
            target: parts.next().map(ToString::to_string),
        },
        Some(_) => TaskCommand::List,
    }
}

#[derive(Clone, Copy)]
enum TaskUpdateSegmentKind {
    Content,
    Priority,
    Description,
    Status,
    Output,
}

fn parse_task_update_command(remainder: &str) -> TaskCommand {
    let mut parts = remainder.split_whitespace();
    let _ = parts.next();
    let task_id = parts.next().map(ToString::to_string);

    let mut content = None;
    let mut priority = None;
    let mut description = None;
    let mut status = None;
    let mut output = None;
    let mut clear_priority = false;
    let mut clear_description = false;
    let mut clear_output = false;
    let mut current_kind = None;
    let mut current_value = String::new();

    for token in parts {
        let next_kind = match token {
            "--content" => Some(TaskUpdateSegmentKind::Content),
            "--priority" => Some(TaskUpdateSegmentKind::Priority),
            "--description" => Some(TaskUpdateSegmentKind::Description),
            "--status" => Some(TaskUpdateSegmentKind::Status),
            "--output" => Some(TaskUpdateSegmentKind::Output),
            "--clear-priority" => {
                push_task_update_segment(
                    &mut content,
                    &mut priority,
                    &mut description,
                    &mut status,
                    &mut output,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_priority = true;
                None
            }
            "--clear-description" => {
                push_task_update_segment(
                    &mut content,
                    &mut priority,
                    &mut description,
                    &mut status,
                    &mut output,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_description = true;
                None
            }
            "--clear-output" => {
                push_task_update_segment(
                    &mut content,
                    &mut priority,
                    &mut description,
                    &mut status,
                    &mut output,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_output = true;
                None
            }
            _ => {
                if !current_value.is_empty() {
                    current_value.push(' ');
                }
                current_value.push_str(token);
                continue;
            }
        };

        if let Some(next_kind) = next_kind {
            push_task_update_segment(
                &mut content,
                &mut priority,
                &mut description,
                &mut status,
                &mut output,
                current_kind.take(),
                &mut current_value,
            );
            current_kind = Some(next_kind);
        }
    }

    push_task_update_segment(
        &mut content,
        &mut priority,
        &mut description,
        &mut status,
        &mut output,
        current_kind,
        &mut current_value,
    );

    TaskCommand::Update {
        task_id,
        content,
        priority,
        clear_priority,
        description,
        clear_description,
        status,
        output,
        clear_output,
    }
}

fn push_task_update_segment(
    content: &mut Option<String>,
    priority: &mut Option<String>,
    description: &mut Option<String>,
    status: &mut Option<String>,
    output: &mut Option<String>,
    kind: Option<TaskUpdateSegmentKind>,
    current_value: &mut String,
) {
    let value = current_value.trim().to_string();
    current_value.clear();
    if value.is_empty() {
        return;
    }

    match kind {
        Some(TaskUpdateSegmentKind::Content) => *content = Some(value),
        Some(TaskUpdateSegmentKind::Priority) => *priority = Some(value),
        Some(TaskUpdateSegmentKind::Description) => *description = Some(value),
        Some(TaskUpdateSegmentKind::Status) => *status = Some(value),
        Some(TaskUpdateSegmentKind::Output) => *output = Some(value),
        None => {}
    }
}

fn parse_mcp_command(remainder: &str) -> McpCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => McpCommand::List,
        Some(action) if action == "list" => McpCommand::List,
        Some(action) if action == "panel" => McpCommand::Panel {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "show" => McpCommand::Show {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "tools" => McpCommand::Tools {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "call" => {
            let server_name = parts.next().map(ToString::to_string);
            let tool_name = parts.next().map(ToString::to_string);
            let input = {
                let remainder = parts.collect::<Vec<_>>().join(" ");
                (!remainder.is_empty()).then_some(remainder)
            };
            McpCommand::Call {
                server_name,
                tool_name,
                input,
            }
        }
        Some(action) if action == "resources" => McpCommand::Resources {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "prompts" => McpCommand::Prompts {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "read-resource" => McpCommand::ReadResource {
            server_name: parts.next().map(ToString::to_string),
            uri: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "get-prompt" => {
            let server_name = parts.next().map(ToString::to_string);
            let prompt_name = parts.next().map(ToString::to_string);
            let input = {
                let remainder = parts.collect::<Vec<_>>().join(" ");
                (!remainder.is_empty()).then_some(remainder)
            };
            McpCommand::GetPrompt {
                server_name,
                prompt_name,
                input,
            }
        }
        Some(action) if action == "auth" => {
            match parts.next().map(|part| part.to_ascii_lowercase()) {
                Some(mode) if mode == "show" => McpCommand::AuthShow {
                    server_name: parts.next().map(ToString::to_string),
                },
                Some(mode) if mode == "set-token" => McpCommand::AuthSetToken {
                    server_name: parts.next().map(ToString::to_string),
                    bearer_token: parts.next().map(ToString::to_string),
                },
                Some(mode) if mode == "clear" => McpCommand::AuthClear {
                    server_name: parts.next().map(ToString::to_string),
                },
                Some(mode) if mode == "oauth-set" => McpCommand::AuthOauthSet {
                    server_name: parts.next().map(ToString::to_string),
                    client_id: parts.next().map(ToString::to_string),
                    authorize_url: parts.next().map(ToString::to_string),
                    token_url: parts.next().map(ToString::to_string),
                    redirect_url: parts.next().map(ToString::to_string),
                    scopes: parts.map(ToString::to_string).collect(),
                },
                Some(mode) if mode == "oauth-start" => McpCommand::AuthOauthStart {
                    server_name: parts.next().map(ToString::to_string),
                },
                Some(mode) if mode == "oauth-exchange" => McpCommand::AuthOauthExchange {
                    server_name: parts.next().map(ToString::to_string),
                    code: parts.next().map(ToString::to_string),
                    code_verifier: parts.next().map(ToString::to_string),
                },
                Some(mode) if mode == "oauth-refresh" => McpCommand::AuthOauthRefresh {
                    server_name: parts.next().map(ToString::to_string),
                },
                Some(mode) if mode == "oauth-clear" => McpCommand::AuthOauthClear {
                    server_name: parts.next().map(ToString::to_string),
                },
                _ => McpCommand::Help,
            }
        }
        Some(action) if action == "registry" => {
            match parts.next().map(|part| part.to_ascii_lowercase()) {
                None => McpCommand::RegistryList {
                    cursor: None,
                    limit: None,
                },
                Some(mode) if mode == "list" => McpCommand::RegistryList {
                    cursor: parts.next().map(ToString::to_string),
                    limit: parts.next().and_then(|value| value.parse::<usize>().ok()),
                },
                Some(mode) if mode == "show" => McpCommand::RegistryShow {
                    name: parts.next().map(ToString::to_string),
                },
                Some(mode) if mode == "install" => McpCommand::RegistryInstall {
                    name: parts.next().map(ToString::to_string),
                    server_name: parts.next().map(ToString::to_string),
                    scope: parts.next().map(ToString::to_string),
                },
                _ => McpCommand::Help,
            }
        }
        Some(action) if action == "enable" => McpCommand::Enable {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "disable" => McpCommand::Disable {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "remove" => McpCommand::Remove {
            server_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "add" => match parts.next().map(|part| part.to_ascii_lowercase())
        {
            Some(kind) if kind == "stdio" => McpCommand::AddStdio {
                server_name: parts.next().map(ToString::to_string),
                command: parts.next().map(ToString::to_string),
                args: parts.map(ToString::to_string).collect(),
            },
            Some(kind) if kind == "sse" => McpCommand::AddSse {
                server_name: parts.next().map(ToString::to_string),
                url: parts.next().map(ToString::to_string),
            },
            Some(kind) if kind == "ws" => McpCommand::AddWs {
                server_name: parts.next().map(ToString::to_string),
                url: parts.next().map(ToString::to_string),
            },
            _ => McpCommand::Help,
        },
        Some(_) => McpCommand::Help,
    }
}

fn parse_plugin_command(remainder: &str) -> PluginCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => PluginCommand::List,
        Some(action) if action == "list" => PluginCommand::List,
        Some(action) if action == "panel" => PluginCommand::Panel {
            plugin_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "show" => PluginCommand::Show {
            plugin_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "install" => {
            let remaining = parts.collect::<Vec<_>>();
            let disabled = remaining.iter().any(|part| *part == "--disabled");
            let source = remaining
                .into_iter()
                .filter(|part| *part != "--disabled")
                .collect::<Vec<_>>()
                .join(" ");
            PluginCommand::Install {
                source: (!source.is_empty()).then_some(source),
                disabled,
            }
        }
        Some(action) if action == "enable" => PluginCommand::Enable {
            plugin_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "disable" => PluginCommand::Disable {
            plugin_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "remove" => PluginCommand::Remove {
            plugin_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "marketplace" => PluginCommand::Marketplace(
            parse_marketplace_command(&parts.collect::<Vec<_>>().join(" ")),
        ),
        Some(_) => PluginCommand::Help,
    }
}

fn parse_marketplace_command(remainder: &str) -> MarketplaceCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => MarketplaceCommand::List,
        Some(action) if action == "list" => MarketplaceCommand::List,
        Some(action) if action == "show" => MarketplaceCommand::Show {
            marketplace_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "add" => MarketplaceCommand::Add {
            marketplace_name: parts.next().map(ToString::to_string),
            url: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "enable" => MarketplaceCommand::Enable {
            marketplace_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "disable" => MarketplaceCommand::Disable {
            marketplace_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "remove" => MarketplaceCommand::Remove {
            marketplace_name: parts.next().map(ToString::to_string),
        },
        Some(_) => MarketplaceCommand::Help,
    }
}
