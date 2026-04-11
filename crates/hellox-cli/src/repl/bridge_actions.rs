use anyhow::Result;
use hellox_bridge::{
    format_bridge_session_detail, format_bridge_session_list, format_bridge_status,
    format_ide_status, inspect_bridge_status, inspect_ide_status, list_bridge_sessions,
    load_bridge_session, BridgeRuntimePaths,
};

use crate::bridge_panel::{render_bridge_panel, render_ide_panel};

use super::commands::{BridgeCommand, IdeCommand};
use super::ReplMetadata;

pub(super) fn handle_bridge_command(
    command: BridgeCommand,
    metadata: &ReplMetadata,
) -> Result<String> {
    let paths = runtime_paths(metadata);

    match command {
        BridgeCommand::Status => Ok(format_bridge_status(&inspect_bridge_status(&paths)?)),
        BridgeCommand::Panel { session_id } => render_bridge_panel(&paths, session_id.as_deref()),
        BridgeCommand::Sessions => Ok(format_bridge_session_list(&list_bridge_sessions(&paths)?)),
        BridgeCommand::Show { session_id: None } => {
            Ok("Usage: /bridge show <session-id>".to_string())
        }
        BridgeCommand::Show {
            session_id: Some(session_id),
        } => Ok(format_bridge_session_detail(&load_bridge_session(
            &paths,
            &session_id,
        )?)),
        BridgeCommand::Help => Ok(bridge_help_text()),
    }
}

pub(super) fn handle_ide_command(command: IdeCommand, metadata: &ReplMetadata) -> Result<String> {
    let paths = runtime_paths(metadata);

    match command {
        IdeCommand::Status => Ok(format_ide_status(&inspect_ide_status(&paths)?)),
        IdeCommand::Panel => render_ide_panel(&paths),
        IdeCommand::Help => Ok(ide_help_text()),
    }
}

fn runtime_paths(metadata: &ReplMetadata) -> BridgeRuntimePaths {
    BridgeRuntimePaths::new(
        metadata.config_path.clone(),
        metadata.sessions_root.clone(),
        metadata.plugins_root.clone(),
    )
}

fn bridge_help_text() -> String {
    [
        "Usage:",
        "  /bridge",
        "  /bridge panel [session-id]",
        "  /bridge sessions",
        "  /bridge show <session-id>",
    ]
    .join("\n")
}

fn ide_help_text() -> String {
    ["Usage:", "  /ide", "  /ide status", "  /ide panel"].join("\n")
}
