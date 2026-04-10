use anyhow::Result;
use hellox_agent::AgentSession;

use crate::cli_ui_types::{BriefCommands, ToolsCommands};
use crate::ui_commands::{brief_command_text, tools_command_text};

use super::commands::{BriefCommand, ToolsCommand};

pub(super) fn handle_brief_command(
    command: BriefCommand,
    session: &AgentSession,
) -> Result<String> {
    match command {
        BriefCommand::Show => brief_command_text(BriefCommands::Show {
            cwd: Some(session.working_directory().to_path_buf()),
        }),
        BriefCommand::Set { message: None } => Ok("Usage: /brief set <message>".to_string()),
        BriefCommand::Set {
            message: Some(message),
        } => brief_command_text(BriefCommands::Set {
            message,
            attachments: Vec::new(),
            status: None,
            cwd: Some(session.working_directory().to_path_buf()),
        }),
        BriefCommand::Clear => brief_command_text(BriefCommands::Clear {
            cwd: Some(session.working_directory().to_path_buf()),
        }),
        BriefCommand::Help => Ok("Usage: /brief [show|set <message>|clear]".to_string()),
    }
}

pub(super) fn handle_tools_command(command: ToolsCommand) -> Result<String> {
    match command {
        ToolsCommand::Search { query: None, .. } => Ok("Usage: /tools <query>".to_string()),
        ToolsCommand::Search {
            query: Some(query),
            limit,
        } => tools_command_text(ToolsCommands::Search { query, limit }),
        ToolsCommand::Help => Ok("Usage: /tools <query> [limit]".to_string()),
    }
}
