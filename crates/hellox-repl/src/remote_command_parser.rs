use crate::command_types::{AssistantCommand, RemoteEnvCommand, TeleportCommand};

pub fn parse_remote_env_command(remainder: &str) -> RemoteEnvCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => RemoteEnvCommand::List,
        Some(action) if action == "panel" => RemoteEnvCommand::Panel {
            environment_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "list" => RemoteEnvCommand::List,
        Some(action) if action == "show" => RemoteEnvCommand::Show {
            environment_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "add" => RemoteEnvCommand::Add {
            environment_name: parts.next().map(ToString::to_string),
            url: parts.next().map(ToString::to_string),
            token_env: parts.next().map(ToString::to_string),
            account_id: parts.next().map(ToString::to_string),
            device_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "enable" => RemoteEnvCommand::Enable {
            environment_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "disable" => RemoteEnvCommand::Disable {
            environment_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "remove" => RemoteEnvCommand::Remove {
            environment_name: parts.next().map(ToString::to_string),
        },
        Some(_) => RemoteEnvCommand::Help,
    }
}

pub fn parse_teleport_command(remainder: &str) -> TeleportCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => TeleportCommand::Help,
        Some(action) if action == "panel" => TeleportCommand::Panel {
            environment_name: parts.next().map(ToString::to_string),
            session_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "plan" => TeleportCommand::Plan {
            environment_name: parts.next().map(ToString::to_string),
            session_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "connect" => TeleportCommand::Connect {
            environment_name: parts.next().map(ToString::to_string),
            session_id: parts.next().map(ToString::to_string),
        },
        Some(_) => TeleportCommand::Help,
    }
}

pub fn parse_assistant_command(remainder: &str) -> AssistantCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => AssistantCommand::List {
            environment_name: None,
        },
        Some(action) if action == "list" => AssistantCommand::List {
            environment_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "show" => AssistantCommand::Show {
            session_id: parts.next().map(ToString::to_string),
            environment_name: parts.next().map(ToString::to_string),
        },
        Some(_) => AssistantCommand::Help,
    }
}
