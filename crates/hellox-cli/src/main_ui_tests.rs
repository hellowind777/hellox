use std::path::PathBuf;

use clap::Parser;

use crate::cli_types::{BriefCommands, Cli, Commands, ConfigCommands, PlanCommands, ToolsCommands};

#[test]
fn parses_brief_and_tools_commands() {
    let brief = Cli::try_parse_from([
        "hellox",
        "brief",
        "set",
        "Need review on the release notes",
        "--attachment",
        "notes/release.md",
        "--status",
        "in_progress",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse brief set");
    let tools = Cli::try_parse_from(["hellox", "tools", "search", "mcp", "--limit", "5"])
        .expect("parse tools search");

    match brief.command {
        Some(Commands::Brief {
            command:
                BriefCommands::Set {
                    message,
                    attachments,
                    status,
                    cwd,
                },
        }) => {
            assert_eq!(message, "Need review on the release notes");
            assert_eq!(attachments, vec![String::from("notes/release.md")]);
            assert_eq!(status, Some(String::from("in_progress")));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected brief command: {other:?}"),
    }

    match tools.command {
        Some(Commands::Tools {
            command: ToolsCommands::Search { query, limit },
        }) => {
            assert_eq!(query, "mcp");
            assert_eq!(limit, 5);
        }
        other => panic!("unexpected tools command: {other:?}"),
    }
}

#[test]
fn parses_config_and_plan_commands() {
    let config_panel = Cli::try_parse_from([
        "hellox",
        "config",
        "panel",
        "--config",
        "config/custom.toml",
    ])
    .expect("parse config panel");
    let config = Cli::try_parse_from([
        "hellox",
        "config",
        "set",
        "prompt.persona",
        "reviewer",
        "--config",
        "config/custom.toml",
    ])
    .expect("parse config set");
    let plan = Cli::try_parse_from([
        "hellox",
        "plan",
        "exit",
        "session-123",
        "--step",
        "completed:Audit docs",
        "--step",
        "in_progress:Implement config surface",
        "--allow",
        "continue implementation",
    ])
    .expect("parse plan exit");
    let plan_panel =
        Cli::try_parse_from(["hellox", "plan", "panel", "session-123"]).expect("parse plan panel");

    match config_panel.command {
        Some(Commands::Config {
            command: ConfigCommands::Panel { config },
        }) => {
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
        }
        other => panic!("unexpected config panel command: {other:?}"),
    }

    match config.command {
        Some(Commands::Config {
            command: ConfigCommands::Set { key, value, config },
        }) => {
            assert_eq!(key, "prompt.persona");
            assert_eq!(value, "reviewer");
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
        }
        other => panic!("unexpected config command: {other:?}"),
    }

    match plan.command {
        Some(Commands::Plan {
            command:
                PlanCommands::Exit {
                    session_id,
                    steps,
                    allowed_prompts,
                },
        }) => {
            assert_eq!(session_id, "session-123");
            assert_eq!(
                steps,
                vec![
                    String::from("completed:Audit docs"),
                    String::from("in_progress:Implement config surface"),
                ]
            );
            assert_eq!(
                allowed_prompts,
                vec![String::from("continue implementation")]
            );
        }
        other => panic!("unexpected plan command: {other:?}"),
    }

    match plan_panel.command {
        Some(Commands::Plan {
            command: PlanCommands::Panel { session_id },
        }) => {
            assert_eq!(session_id, "session-123");
        }
        other => panic!("unexpected plan panel command: {other:?}"),
    }
}

#[test]
fn parses_plan_authoring_commands() {
    let add_step = Cli::try_parse_from([
        "hellox",
        "plan",
        "add-step",
        "session-123",
        "--step",
        "in_progress:Implement plan authoring",
        "--index",
        "1",
    ])
    .expect("parse plan add-step");
    let allow = Cli::try_parse_from([
        "hellox",
        "plan",
        "allow",
        "session-123",
        "continue implementation",
    ])
    .expect("parse plan allow");

    match add_step.command {
        Some(Commands::Plan {
            command:
                PlanCommands::AddStep {
                    session_id,
                    step,
                    index,
                },
        }) => {
            assert_eq!(session_id, "session-123");
            assert_eq!(step, "in_progress:Implement plan authoring");
            assert_eq!(index, Some(1));
        }
        other => panic!("unexpected plan add-step command: {other:?}"),
    }

    match allow.command {
        Some(Commands::Plan {
            command: PlanCommands::Allow { session_id, prompt },
        }) => {
            assert_eq!(session_id, "session-123");
            assert_eq!(prompt, "continue implementation");
        }
        other => panic!("unexpected plan allow command: {other:?}"),
    }
}
