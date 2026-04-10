use std::path::PathBuf;

use clap::Parser;

use crate::cli_types::{Cli, Commands, ServerCommands};

#[test]
fn parses_server_admin_commands() {
    let sessions =
        Cli::try_parse_from(["hellox", "server", "sessions"]).expect("parse server sessions");
    let show = Cli::try_parse_from(["hellox", "server", "show-session", "session-123"])
        .expect("parse server show-session");
    let managed_set = Cli::try_parse_from([
        "hellox",
        "server",
        "managed-settings-set",
        "docs/managed.toml",
        "--signature",
        "sig-123",
    ])
    .expect("parse managed settings set");
    let policy_set = Cli::try_parse_from([
        "hellox",
        "server",
        "policy-limits-set",
        "--disable-command",
        "plugin",
        "--disable-tool",
        "bash",
        "--notes",
        "enterprise policy",
    ])
    .expect("parse policy limits set");
    let settings_show = Cli::try_parse_from(["hellox", "server", "settings-show", "account-1"])
        .expect("parse settings show");
    let team_memory_show = Cli::try_parse_from([
        "hellox",
        "server",
        "team-memory-show",
        "account-1",
        "repo-1",
    ])
    .expect("parse team memory show");
    let team_memory_panel = Cli::try_parse_from([
        "hellox",
        "server",
        "team-memory-panel",
        "account-1",
        "repo-1",
    ])
    .expect("parse team memory panel");

    match sessions.command {
        Some(Commands::Server {
            command: ServerCommands::Sessions { config },
        }) => assert_eq!(config, None),
        other => panic!("unexpected server sessions command: {other:?}"),
    }

    match show.command {
        Some(Commands::Server {
            command: ServerCommands::ShowSession { session_id, config },
        }) => {
            assert_eq!(session_id, "session-123");
            assert_eq!(config, None);
        }
        other => panic!("unexpected server show-session command: {other:?}"),
    }

    match managed_set.command {
        Some(Commands::Server {
            command:
                ServerCommands::ManagedSettingsSet {
                    config_toml_file,
                    config,
                    signature,
                },
        }) => {
            assert_eq!(config_toml_file, PathBuf::from("docs/managed.toml"));
            assert_eq!(config, None);
            assert_eq!(signature, Some(String::from("sig-123")));
        }
        other => panic!("unexpected managed-settings-set command: {other:?}"),
    }

    match policy_set.command {
        Some(Commands::Server {
            command:
                ServerCommands::PolicyLimitsSet {
                    config,
                    disabled_commands,
                    disabled_tools,
                    notes,
                },
        }) => {
            assert_eq!(config, None);
            assert_eq!(disabled_commands, vec![String::from("plugin")]);
            assert_eq!(disabled_tools, vec![String::from("bash")]);
            assert_eq!(notes, Some(String::from("enterprise policy")));
        }
        other => panic!("unexpected policy-limits-set command: {other:?}"),
    }

    match settings_show.command {
        Some(Commands::Server {
            command: ServerCommands::SettingsShow { account_id, config },
        }) => {
            assert_eq!(account_id, "account-1");
            assert_eq!(config, None);
        }
        other => panic!("unexpected settings-show command: {other:?}"),
    }

    match team_memory_show.command {
        Some(Commands::Server {
            command:
                ServerCommands::TeamMemoryShow {
                    account_id,
                    repo_id,
                    config,
                },
        }) => {
            assert_eq!(account_id, "account-1");
            assert_eq!(repo_id, "repo-1");
            assert_eq!(config, None);
        }
        other => panic!("unexpected team-memory-show command: {other:?}"),
    }

    match team_memory_panel.command {
        Some(Commands::Server {
            command:
                ServerCommands::TeamMemoryPanel {
                    account_id,
                    repo_id,
                    config,
                },
        }) => {
            assert_eq!(account_id, "account-1");
            assert_eq!(repo_id, "repo-1");
            assert_eq!(config, None);
        }
        other => panic!("unexpected team-memory-panel command: {other:?}"),
    }
}
