use anyhow::Result;

use crate::cli_types::ConfigCommands;
use crate::config_commands::{config_command_text, config_path_text};

use super::commands::ConfigCommand;
use super::ReplMetadata;

pub(super) fn handle_config_command(
    command: ConfigCommand,
    metadata: &ReplMetadata,
) -> Result<String> {
    match command {
        ConfigCommand::Show => config_command_text(ConfigCommands::Show {
            config: Some(metadata.config_path.clone()),
        }),
        ConfigCommand::Panel { focus_key } => config_command_text(ConfigCommands::Panel {
            focus_key,
            config: Some(metadata.config_path.clone()),
        }),
        ConfigCommand::Path => Ok(config_path_text(&metadata.config_path)),
        ConfigCommand::Keys => config_command_text(ConfigCommands::Keys),
        ConfigCommand::Set { key: None, .. } => Ok("Usage: /config set <key> <value>".to_string()),
        ConfigCommand::Set {
            key: Some(_),
            value: None,
        } => Ok("Usage: /config set <key> <value>".to_string()),
        ConfigCommand::Set {
            key: Some(key),
            value: Some(value),
        } => config_command_text(ConfigCommands::Set {
            key,
            value,
            config: Some(metadata.config_path.clone()),
        }),
        ConfigCommand::Clear { key: None } => Ok("Usage: /config clear <key>".to_string()),
        ConfigCommand::Clear { key: Some(key) } => config_command_text(ConfigCommands::Clear {
            key,
            config: Some(metadata.config_path.clone()),
        }),
        ConfigCommand::Help => Ok(
            "Usage: /config [show|panel [key]|path|keys|set <key> <value>|clear <key>]".to_string(),
        ),
    }
}
