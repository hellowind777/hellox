use clap::Parser;

use crate::cli_types::{Cli, Commands, McpCommands, PluginCommands};

#[test]
fn parses_mcp_panel_command() {
    let panel = Cli::try_parse_from(["hellox", "mcp", "panel", "docs"]).expect("parse mcp panel");

    match panel.command {
        Some(Commands::Mcp {
            command: McpCommands::Panel { server_name },
        }) => {
            assert_eq!(server_name, Some(String::from("docs")));
        }
        other => panic!("unexpected mcp panel command: {other:?}"),
    }
}

#[test]
fn parses_plugin_panel_command() {
    let panel = Cli::try_parse_from(["hellox", "plugin", "panel", "filesystem"])
        .expect("parse plugin panel");

    match panel.command {
        Some(Commands::Plugin {
            command: PluginCommands::Panel { plugin_id },
        }) => {
            assert_eq!(plugin_id, Some(String::from("filesystem")));
        }
        other => panic!("unexpected plugin panel command: {other:?}"),
    }
}
