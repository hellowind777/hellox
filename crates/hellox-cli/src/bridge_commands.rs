use std::io;

use anyhow::Result;
use hellox_bridge::{
    format_bridge_session_detail, format_bridge_session_list, format_bridge_status,
    format_ide_status, inspect_bridge_status, inspect_ide_status, list_bridge_sessions,
    load_bridge_session, run_stdio_bridge, BridgeRuntimePaths,
};
use hellox_config::{default_config_path, plugins_root, sessions_root};

use crate::cli_types::{BridgeCommands, IdeCommands};

pub fn handle_bridge_command(command: BridgeCommands) -> Result<()> {
    let paths = runtime_paths();

    match command {
        BridgeCommands::Status => {
            println!("{}", format_bridge_status(&inspect_bridge_status(&paths)?));
        }
        BridgeCommands::Sessions => {
            println!(
                "{}",
                format_bridge_session_list(&list_bridge_sessions(&paths)?)
            );
        }
        BridgeCommands::ShowSession { session_id } => {
            println!(
                "{}",
                format_bridge_session_detail(&load_bridge_session(&paths, &session_id)?)
            );
        }
        BridgeCommands::Stdio => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            run_stdio_bridge(stdin.lock(), stdout.lock(), &paths)?;
        }
    }

    Ok(())
}

pub fn handle_ide_command(command: IdeCommands) -> Result<()> {
    let paths = runtime_paths();

    match command {
        IdeCommands::Status => {
            println!("{}", format_ide_status(&inspect_ide_status(&paths)?));
        }
    }

    Ok(())
}

fn runtime_paths() -> BridgeRuntimePaths {
    BridgeRuntimePaths::new(default_config_path(), sessions_root(), plugins_root())
}
