use std::path::PathBuf;

use clap::Parser;

use crate::cli_types::{Cli, Commands, ModelCommands, SessionCommands};

#[test]
fn parses_model_panel_command() {
    let panel = Cli::try_parse_from([
        "hellox",
        "model",
        "panel",
        "sonnet",
        "--config",
        "config/custom.toml",
    ])
    .expect("parse model panel");

    match panel.command {
        Some(Commands::Model {
            command:
                ModelCommands::Panel {
                    profile_name,
                    config,
                },
        }) => {
            assert_eq!(profile_name, Some(String::from("sonnet")));
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
        }
        other => panic!("unexpected model panel command: {other:?}"),
    }
}

#[test]
fn parses_session_panel_command() {
    let panel = Cli::try_parse_from(["hellox", "session", "panel", "persisted-session"])
        .expect("parse session panel");

    match panel.command {
        Some(Commands::Session {
            command: SessionCommands::Panel { session_id },
        }) => {
            assert_eq!(session_id, Some(String::from("persisted-session")));
        }
        other => panic!("unexpected session panel command: {other:?}"),
    }
}
