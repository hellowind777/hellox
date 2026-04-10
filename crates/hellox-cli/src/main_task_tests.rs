use std::path::PathBuf;

use clap::Parser;

use crate::cli_task_types::TaskCommands;
use crate::cli_types::{Cli, Commands};

#[test]
fn parses_extended_task_commands() {
    let panel =
        Cli::try_parse_from(["hellox", "tasks", "panel", "task-7"]).expect("parse tasks panel");
    let show =
        Cli::try_parse_from(["hellox", "tasks", "show", "task-7"]).expect("parse tasks show");
    let update = Cli::try_parse_from([
        "hellox",
        "tasks",
        "update",
        "task-7",
        "--status",
        "in_progress",
        "--output",
        "Started implementation",
        "--clear-description",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse tasks update");
    let stop = Cli::try_parse_from([
        "hellox",
        "tasks",
        "stop",
        "task-7",
        "--reason",
        "Waiting for tmux host",
    ])
    .expect("parse tasks stop");

    match panel.command {
        Some(Commands::Tasks {
            command: TaskCommands::Panel { task_id, cwd },
        }) => {
            assert_eq!(task_id, Some(String::from("task-7")));
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected tasks panel command: {other:?}"),
    }

    match show.command {
        Some(Commands::Tasks {
            command: TaskCommands::Show { task_id, cwd },
        }) => {
            assert_eq!(task_id, "task-7");
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected tasks show command: {other:?}"),
    }

    match update.command {
        Some(Commands::Tasks {
            command:
                TaskCommands::Update {
                    task_id,
                    content,
                    priority,
                    clear_priority,
                    description,
                    clear_description,
                    status,
                    output,
                    clear_output,
                    cwd,
                },
        }) => {
            assert_eq!(task_id, "task-7");
            assert_eq!(content, None);
            assert_eq!(priority, None);
            assert!(!clear_priority);
            assert_eq!(description, None);
            assert!(clear_description);
            assert_eq!(status, Some(String::from("in_progress")));
            assert_eq!(output, Some(String::from("Started implementation")));
            assert!(!clear_output);
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected tasks update command: {other:?}"),
    }

    match stop.command {
        Some(Commands::Tasks {
            command:
                TaskCommands::Stop {
                    task_id,
                    reason,
                    cwd,
                },
        }) => {
            assert_eq!(task_id, "task-7");
            assert_eq!(reason, Some(String::from("Waiting for tmux host")));
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected tasks stop command: {other:?}"),
    }
}
