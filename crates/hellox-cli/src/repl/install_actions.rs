use std::path::PathBuf;

use anyhow::Result;

use crate::cli_install_types::{InstallCommands, UpgradeCommands};
use crate::install_commands::{install_command_text, upgrade_command_text};

use super::commands::{InstallCommand, UpgradeCommand};

pub(super) fn handle_install_command(command: InstallCommand) -> Result<String> {
    match command {
        InstallCommand::Status => install_command_text(InstallCommands::Status),
        InstallCommand::Plan { source, target } => install_command_text(InstallCommands::Plan {
            source: source.map(PathBuf::from),
            target: target.map(PathBuf::from),
        }),
        InstallCommand::Apply {
            source,
            target,
            force,
        } => install_command_text(InstallCommands::Apply {
            source: source.map(PathBuf::from),
            target: target.map(PathBuf::from),
            force,
        }),
        InstallCommand::Help => Ok("Usage: /install [status|plan|apply]".to_string()),
    }
}

pub(super) fn handle_upgrade_command(command: UpgradeCommand) -> Result<String> {
    match command {
        UpgradeCommand::Status => upgrade_command_text(UpgradeCommands::Status),
        UpgradeCommand::Plan { source: None, .. } => {
            Ok("Usage: /upgrade plan <source> [target]".to_string())
        }
        UpgradeCommand::Apply { source: None, .. } => {
            Ok("Usage: /upgrade apply <source> [target] [--backup] [--force]".to_string())
        }
        UpgradeCommand::Plan {
            source: Some(source),
            target,
        } => upgrade_command_text(UpgradeCommands::Plan {
            source: PathBuf::from(source),
            target: target.map(PathBuf::from),
        }),
        UpgradeCommand::Apply {
            source: Some(source),
            target,
            backup,
            force,
        } => upgrade_command_text(UpgradeCommands::Apply {
            source: PathBuf::from(source),
            target: target.map(PathBuf::from),
            backup,
            force,
        }),
        UpgradeCommand::Help => Ok("Usage: /upgrade [status|plan|apply]".to_string()),
    }
}
