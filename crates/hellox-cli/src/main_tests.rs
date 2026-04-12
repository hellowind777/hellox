use std::path::PathBuf;

use clap::{CommandFactory, Parser};

use crate::cli_types::{
    AssistantCommands, AuthCommands, BridgeCommands, Cli, Commands, IdeCommands, InstallCommands,
    MarketplaceCommands, McpCommands, McpScopeValue, MemoryCommands, ModelCommands,
    OutputStyleCommands, PermissionsCommands, PluginCommands, RemoteEnvCommands, ServerCommands,
    SessionCommands, SyncCommands, TaskCommands, TeleportCommands, UpgradeCommands,
    WorkflowCommands,
};
use crate::memory::MemoryScopeSelector;
use crate::search::DEFAULT_SEARCH_LIMIT;
use hellox_config::PermissionMode;

#[test]
fn parses_empty_root_command() {
    let cli = Cli::try_parse_from(["hellox"]).expect("parse root command");
    assert!(cli.prompt.is_none());
    assert!(!cli.print);
    assert!(!cli.continue_last);
    assert_eq!(cli.resume, None);
    assert!(cli.command.is_none());
}

#[test]
fn default_entry_uses_repl_only_for_interactive_terminals() {
    assert!(crate::should_launch_default_repl(true, true));
    assert!(!crate::should_launch_default_repl(false, true));
    assert!(!crate::should_launch_default_repl(true, false));
    assert!(!crate::should_launch_default_repl(false, false));
}

#[test]
fn root_contract_prefers_interactive_only_for_plain_tty_sessions() {
    assert!(crate::should_run_root_interactive(false, true, true));
    assert!(!crate::should_run_root_interactive(true, true, true));
    assert!(!crate::should_run_root_interactive(false, false, true));
    assert!(!crate::should_run_root_interactive(false, true, false));
}

#[test]
fn parses_root_prompt_print_continue_and_resume_flags() {
    let prompt = Cli::try_parse_from(["hellox", "summarize repo"]).expect("parse root prompt");
    let print = Cli::try_parse_from(["hellox", "--print", "summarize repo"])
        .expect("parse root print prompt");
    let continue_last = Cli::try_parse_from(["hellox", "--continue"]).expect("parse root continue");
    let resume_picker =
        Cli::try_parse_from(["hellox", "--resume"]).expect("parse root resume picker");
    let resume_session = Cli::try_parse_from(["hellox", "--resume", "session-123", "fix bug"])
        .expect("parse root resume session");

    assert_eq!(prompt.prompt.as_deref(), Some("summarize repo"));
    assert!(!prompt.print);

    assert_eq!(print.prompt.as_deref(), Some("summarize repo"));
    assert!(print.print);

    assert!(continue_last.continue_last);
    assert_eq!(continue_last.resume, None);

    assert_eq!(resume_picker.resume, Some(None));

    assert_eq!(
        resume_session.resume,
        Some(Some(String::from("session-123")))
    );
    assert_eq!(resume_session.prompt.as_deref(), Some("fix bug"));
}

#[test]
fn help_text_describes_default_root_contract() {
    let help = Cli::command().render_long_help().to_string();
    assert!(help.contains("starts an interactive session by default"));
    assert!(help.contains("-p, --print"));
    assert!(help.contains("-c, --continue"));
    assert!(help.contains("-r, --resume"));
}

#[test]
fn parses_session_share_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "session",
        "share",
        "persisted-session",
        "--output",
        "notes/out.md",
    ])
    .expect("parse session share");

    match cli.command {
        Some(Commands::Session {
            command: SessionCommands::Share { session_id, output },
        }) => {
            assert_eq!(session_id, "persisted-session");
            assert_eq!(output, Some(PathBuf::from("notes/out.md")));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_session_compact_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "session",
        "compact",
        "persisted-session",
        "--instructions",
        "keep latest implementation context",
    ])
    .expect("parse session compact");

    match cli.command {
        Some(Commands::Session {
            command:
                SessionCommands::Compact {
                    session_id,
                    instructions,
                },
        }) => {
            assert_eq!(session_id, "persisted-session");
            assert_eq!(
                instructions,
                Some(String::from("keep latest implementation context"))
            );
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_memory_capture_command() {
    let panel = Cli::try_parse_from(["hellox", "memory", "panel", "session-abc"])
        .expect("parse memory panel");
    let search = Cli::try_parse_from([
        "hellox",
        "memory",
        "search",
        "accepted architecture",
        "--limit",
        "7",
    ])
    .expect("parse memory search");
    let cli = Cli::try_parse_from([
        "hellox",
        "memory",
        "capture",
        "persisted-session",
        "--instructions",
        "preserve accepted decisions",
    ])
    .expect("parse memory capture");

    match panel.command {
        Some(Commands::Memory {
            command:
                MemoryCommands::Panel {
                    archived,
                    memory_id,
                },
        }) => {
            assert!(!archived);
            assert_eq!(memory_id, Some(String::from("session-abc")));
        }
        other => panic!("unexpected command: {other:?}"),
    }

    match search.command {
        Some(Commands::Memory {
            command:
                MemoryCommands::Search {
                    query,
                    limit,
                    archived,
                },
        }) => {
            assert_eq!(query, "accepted architecture");
            assert_eq!(limit, 7);
            assert!(!archived);
        }
        other => panic!("unexpected command: {other:?}"),
    }

    match cli.command {
        Some(Commands::Memory {
            command:
                MemoryCommands::Capture {
                    session_id,
                    instructions,
                },
        }) => {
            assert_eq!(session_id, "persisted-session");
            assert_eq!(
                instructions,
                Some(String::from("preserve accepted decisions"))
            );
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_memory_prune_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "memory",
        "prune",
        "--scope",
        "project",
        "--older-than-days",
        "14",
        "--keep-latest",
        "2",
        "--apply",
    ])
    .expect("parse memory prune");

    match cli.command {
        Some(Commands::Memory {
            command:
                MemoryCommands::Prune {
                    scope,
                    older_than_days,
                    keep_latest,
                    apply,
                },
        }) => {
            assert_eq!(scope, MemoryScopeSelector::Project);
            assert_eq!(older_than_days, 14);
            assert_eq!(keep_latest, 2);
            assert!(apply);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_memory_archive_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "memory",
        "archive",
        "--scope",
        "session",
        "--older-than-days",
        "7",
        "--keep-latest",
        "1",
        "--apply",
    ])
    .expect("parse memory archive");

    match cli.command {
        Some(Commands::Memory {
            command:
                MemoryCommands::Archive {
                    scope,
                    older_than_days,
                    keep_latest,
                    apply,
                },
        }) => {
            assert_eq!(scope, MemoryScopeSelector::Session);
            assert_eq!(older_than_days, 7);
            assert_eq!(keep_latest, 1);
            assert!(apply);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_memory_decay_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "memory",
        "decay",
        "--scope",
        "session",
        "--older-than-days",
        "90",
        "--keep-latest",
        "2",
        "--max-summary-lines",
        "12",
        "--max-summary-chars",
        "800",
        "--apply",
    ])
    .expect("parse memory decay");

    match cli.command {
        Some(Commands::Memory {
            command:
                MemoryCommands::Decay {
                    scope,
                    older_than_days,
                    keep_latest,
                    max_summary_lines,
                    max_summary_chars,
                    apply,
                },
        }) => {
            assert_eq!(scope, MemoryScopeSelector::Session);
            assert_eq!(older_than_days, 90);
            assert_eq!(keep_latest, 2);
            assert_eq!(max_summary_lines, 12);
            assert_eq!(max_summary_chars, 800);
            assert!(apply);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_memory_clusters_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "memory",
        "clusters",
        "--archived",
        "--limit",
        "250",
        "--min-jaccard",
        "0.2",
        "--max-tokens",
        "64",
    ])
    .expect("parse memory clusters");

    match cli.command {
        Some(Commands::Memory {
            command:
                MemoryCommands::Clusters {
                    archived,
                    limit,
                    min_jaccard,
                    max_tokens,
                    semantic,
                },
        }) => {
            assert!(archived);
            assert_eq!(limit, 250);
            assert!((min_jaccard - 0.2).abs() < 0.0001);
            assert_eq!(max_tokens, 64);
            assert!(!semantic);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_model_and_permissions_commands() {
    let model = Cli::try_parse_from(["hellox", "model", "set-default", "sonnet"])
        .expect("parse model set-default");
    let save = Cli::try_parse_from([
        "hellox",
        "model",
        "save",
        "custom",
        "--provider",
        "openai",
        "--upstream-model",
        "gpt-4.1-mini",
        "--display-name",
        "Custom",
        "--input-price",
        "0.25",
        "--output-price",
        "1.25",
        "--set-default",
    ])
    .expect("parse model save");
    let remove =
        Cli::try_parse_from(["hellox", "model", "remove", "custom"]).expect("parse model remove");
    let permissions = Cli::try_parse_from(["hellox", "permissions", "set", "accept-edits"])
        .expect("parse permissions set");

    match model.command {
        Some(Commands::Model {
            command:
                ModelCommands::SetDefault {
                    profile_name,
                    config,
                },
        }) => {
            assert_eq!(profile_name, "sonnet");
            assert_eq!(config, None);
        }
        other => panic!("unexpected model command: {other:?}"),
    }

    match save.command {
        Some(Commands::Model {
            command:
                ModelCommands::Save {
                    profile_name,
                    provider,
                    upstream_model,
                    display_name,
                    input_price,
                    output_price,
                    set_default,
                    config,
                },
        }) => {
            assert_eq!(profile_name, "custom");
            assert_eq!(provider, "openai");
            assert_eq!(upstream_model, "gpt-4.1-mini");
            assert_eq!(display_name, Some(String::from("Custom")));
            assert_eq!(input_price, Some(0.25));
            assert_eq!(output_price, Some(1.25));
            assert!(set_default);
            assert_eq!(config, None);
        }
        other => panic!("unexpected model save command: {other:?}"),
    }

    match remove.command {
        Some(Commands::Model {
            command:
                ModelCommands::Remove {
                    profile_name,
                    config,
                },
        }) => {
            assert_eq!(profile_name, "custom");
            assert_eq!(config, None);
        }
        other => panic!("unexpected model remove command: {other:?}"),
    }

    match permissions.command {
        Some(Commands::Permissions {
            command: PermissionsCommands::Set { mode, config },
        }) => {
            assert_eq!(mode, PermissionMode::AcceptEdits);
            assert_eq!(config, None);
        }
        other => panic!("unexpected permissions command: {other:?}"),
    }
}

#[test]
fn parses_output_style_commands() {
    let list = Cli::try_parse_from(["hellox", "output-style", "list", "--cwd", "workspace/app"])
        .expect("parse output-style list");
    let set_default = Cli::try_parse_from([
        "hellox",
        "output-style",
        "set-default",
        "reviewer",
        "--config",
        "config/custom.toml",
    ])
    .expect("parse output-style set-default");

    match list.command {
        Some(Commands::OutputStyle {
            command: OutputStyleCommands::List { cwd, config },
        }) => {
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
            assert_eq!(config, None);
        }
        other => panic!("unexpected output-style list command: {other:?}"),
    }

    match set_default.command {
        Some(Commands::OutputStyle {
            command:
                OutputStyleCommands::SetDefault {
                    style_name,
                    cwd,
                    config,
                },
        }) => {
            assert_eq!(style_name, "reviewer");
            assert_eq!(cwd, None);
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
        }
        other => panic!("unexpected output-style set-default command: {other:?}"),
    }
}

#[test]
fn parses_search_command() {
    let cli = Cli::try_parse_from(["hellox", "search", "accepted architecture", "--limit", "7"])
        .expect("parse search");

    match cli.command {
        Some(Commands::Search { query, limit }) => {
            assert_eq!(query, "accepted architecture");
            assert_eq!(limit, 7);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_install_and_upgrade_commands() {
    let install = Cli::try_parse_from([
        "hellox",
        "install",
        "apply",
        "--source",
        "target/release/hellox.exe",
        "--target",
        "C:/Users/test/.hellox/bin/hellox.exe",
        "--force",
    ])
    .expect("parse install apply");
    let upgrade = Cli::try_parse_from([
        "hellox",
        "upgrade",
        "apply",
        "--source",
        "dist/hellox.exe",
        "--target",
        "C:/Users/test/.hellox/bin/hellox.exe",
        "--backup",
        "--force",
    ])
    .expect("parse upgrade apply");

    match install.command {
        Some(Commands::Install {
            command:
                Some(InstallCommands::Apply {
                    source,
                    target,
                    force,
                }),
        }) => {
            assert_eq!(source, Some(PathBuf::from("target/release/hellox.exe")));
            assert_eq!(
                target,
                Some(PathBuf::from("C:/Users/test/.hellox/bin/hellox.exe"))
            );
            assert!(force);
        }
        other => panic!("unexpected install command: {other:?}"),
    }

    match upgrade.command {
        Some(Commands::Upgrade {
            command:
                Some(UpgradeCommands::Apply {
                    source,
                    target,
                    backup,
                    force,
                }),
        }) => {
            assert_eq!(source, PathBuf::from("dist/hellox.exe"));
            assert_eq!(
                target,
                Some(PathBuf::from("C:/Users/test/.hellox/bin/hellox.exe"))
            );
            assert!(backup);
            assert!(force);
        }
        other => panic!("unexpected upgrade command: {other:?}"),
    }
}

#[test]
fn parses_hidden_worker_run_agent_command() {
    let cli = Cli::try_parse_from(["hellox", "worker-run-agent", "--job", "tmp/job.json"])
        .expect("parse worker-run-agent");

    match cli.command {
        Some(Commands::WorkerRunAgent { job }) => {
            assert_eq!(job, PathBuf::from("tmp/job.json"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn search_command_uses_default_limit() {
    let cli =
        Cli::try_parse_from(["hellox", "search", "accepted architecture"]).expect("parse search");

    match cli.command {
        Some(Commands::Search { query, limit }) => {
            assert_eq!(query, "accepted architecture");
            assert_eq!(limit, DEFAULT_SEARCH_LIMIT);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_tasks_add_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "tasks",
        "add",
        "write implementation notes",
        "--priority",
        "high",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse tasks add");

    match cli.command {
        Some(Commands::Tasks {
            command:
                TaskCommands::Add {
                    content,
                    priority,
                    description,
                    cwd,
                },
        }) => {
            assert_eq!(content, "write implementation notes");
            assert_eq!(priority, Some(String::from("high")));
            assert_eq!(description, None);
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_workflow_commands() {
    let dashboard = Cli::try_parse_from([
        "hellox",
        "workflow",
        "dashboard",
        "release-review",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse workflow dashboard");
    let dashboard_by_path = Cli::try_parse_from([
        "hellox",
        "workflow",
        "dashboard",
        "--script-path",
        "scripts/custom-release.json",
    ])
    .expect("parse workflow dashboard by script path");
    let overview = Cli::try_parse_from([
        "hellox",
        "workflow",
        "overview",
        "release-review",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse workflow overview");
    let overview_by_path = Cli::try_parse_from([
        "hellox",
        "workflow",
        "overview",
        "--script-path",
        "scripts/custom-release.json",
    ])
    .expect("parse workflow overview by script path");
    let panel = Cli::try_parse_from([
        "hellox",
        "workflow",
        "panel",
        "release-review",
        "--step",
        "2",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse workflow panel");
    let runs = Cli::try_parse_from([
        "hellox",
        "workflow",
        "runs",
        "release-review",
        "--limit",
        "5",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse workflow runs");
    let runs_by_path = Cli::try_parse_from([
        "hellox",
        "workflow",
        "runs",
        "--script-path",
        "scripts/custom-release.json",
        "--limit",
        "7",
    ])
    .expect("parse workflow runs by script path");
    let run = Cli::try_parse_from([
        "hellox",
        "workflow",
        "run",
        "release-review",
        "--shared-context",
        "ship carefully",
        "--config",
        "config/custom.toml",
        "--cwd",
        "workspace/app",
        "--continue-on-error",
    ])
    .expect("parse workflow run");
    let validate = Cli::try_parse_from(["hellox", "workflow", "validate", "release-review"])
        .expect("parse workflow validate");
    let show_run =
        Cli::try_parse_from(["hellox", "workflow", "show-run", "run-123", "--step", "2"])
            .expect("parse workflow show-run");
    let last_run = Cli::try_parse_from([
        "hellox",
        "workflow",
        "last-run",
        "release-review",
        "--step",
        "3",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse workflow last-run");
    let last_run_by_path = Cli::try_parse_from([
        "hellox",
        "workflow",
        "last-run",
        "--script-path",
        "scripts/custom-release.json",
        "--step",
        "2",
    ])
    .expect("parse workflow last-run by script path");
    let init = Cli::try_parse_from([
        "hellox",
        "workflow",
        "init",
        "release-review",
        "--cwd",
        "workspace/app",
        "--shared-context",
        "ship carefully",
        "--continue-on-error",
        "--force",
    ])
    .expect("parse workflow init");

    match dashboard.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Dashboard {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected workflow dashboard command: {other:?}"),
    }

    match dashboard_by_path.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Dashboard {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, None);
            assert_eq!(
                script_path,
                Some(PathBuf::from("scripts/custom-release.json"))
            );
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow dashboard-by-path command: {other:?}"),
    }

    match overview.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Overview {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected workflow overview command: {other:?}"),
    }

    match overview_by_path.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Overview {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, None);
            assert_eq!(
                script_path,
                Some(PathBuf::from("scripts/custom-release.json"))
            );
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow overview-by-path command: {other:?}"),
    }

    match panel.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Panel {
                    workflow_name,
                    script_path,
                    step,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(step, Some(2));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected workflow panel command: {other:?}"),
    }

    match runs.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Runs {
                    workflow_name,
                    script_path,
                    limit,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(limit, 5);
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected workflow runs command: {other:?}"),
    }

    match runs_by_path.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Runs {
                    workflow_name,
                    script_path,
                    limit,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, None);
            assert_eq!(
                script_path,
                Some(PathBuf::from("scripts/custom-release.json"))
            );
            assert_eq!(limit, 7);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow runs-by-path command: {other:?}"),
    }

    match run.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Run {
                    workflow_name,
                    script_path,
                    shared_context,
                    continue_on_error,
                    config,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(shared_context, Some(String::from("ship carefully")));
            assert!(continue_on_error);
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected workflow command: {other:?}"),
    }

    match validate.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Validate {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow validate command: {other:?}"),
    }

    match show_run.command {
        Some(Commands::Workflow {
            command: WorkflowCommands::ShowRun { run_id, step, cwd },
        }) => {
            assert_eq!(run_id, "run-123");
            assert_eq!(step, Some(2));
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow show-run command: {other:?}"),
    }

    match last_run.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::LastRun {
                    workflow_name,
                    script_path,
                    step,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(step, Some(3));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected workflow last-run command: {other:?}"),
    }

    match last_run_by_path.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::LastRun {
                    workflow_name,
                    script_path,
                    step,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, None);
            assert_eq!(
                script_path,
                Some(PathBuf::from("scripts/custom-release.json"))
            );
            assert_eq!(step, Some(2));
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow last-run-by-path command: {other:?}"),
    }

    match init.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::Init {
                    workflow_name,
                    cwd,
                    shared_context,
                    continue_on_error,
                    force,
                },
        }) => {
            assert_eq!(workflow_name, "release-review");
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
            assert_eq!(shared_context, Some(String::from("ship carefully")));
            assert!(continue_on_error);
            assert!(force);
        }
        other => panic!("unexpected workflow init command: {other:?}"),
    }
}

#[test]
fn parses_workflow_authoring_commands() {
    let add_step = Cli::try_parse_from([
        "hellox",
        "workflow",
        "add-step",
        "--workflow",
        "release-review",
        "--prompt",
        "review release notes",
        "--name",
        "review",
        "--index",
        "1",
        "--backend",
        "detached_process",
        "--run-in-background",
    ])
    .expect("parse workflow add-step");
    let update_step = Cli::try_parse_from([
        "hellox",
        "workflow",
        "update-step",
        "--workflow",
        "release-review",
        "2",
        "--clear-name",
        "--prompt",
        "summarize findings",
        "--foreground",
    ])
    .expect("parse workflow update-step");
    let set_context = Cli::try_parse_from([
        "hellox",
        "workflow",
        "set-shared-context",
        "--workflow",
        "release-review",
        "ship carefully",
    ])
    .expect("parse workflow set-shared-context");
    let enable_continue = Cli::try_parse_from([
        "hellox",
        "workflow",
        "enable-continue-on-error",
        "--workflow",
        "release-review",
    ])
    .expect("parse workflow enable-continue-on-error");

    match add_step.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::AddStep {
                    workflow_name,
                    script_path,
                    name,
                    prompt,
                    index,
                    when,
                    model,
                    backend,
                    step_cwd,
                    run_in_background,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(name, Some(String::from("review")));
            assert_eq!(prompt, "review release notes");
            assert_eq!(index, Some(1));
            assert_eq!(when, None);
            assert_eq!(model, None);
            assert_eq!(backend, Some(String::from("detached_process")));
            assert_eq!(step_cwd, None);
            assert!(run_in_background);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow add-step command: {other:?}"),
    }

    match update_step.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::UpdateStep {
                    workflow_name,
                    step_number,
                    script_path,
                    name,
                    clear_name,
                    prompt,
                    when,
                    clear_when,
                    model,
                    clear_model,
                    backend,
                    clear_backend,
                    step_cwd,
                    clear_step_cwd,
                    run_in_background,
                    foreground,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(step_number, 2);
            assert_eq!(script_path, None);
            assert_eq!(name, None);
            assert!(clear_name);
            assert_eq!(prompt, Some(String::from("summarize findings")));
            assert_eq!(when, None);
            assert!(!clear_when);
            assert_eq!(model, None);
            assert!(!clear_model);
            assert_eq!(backend, None);
            assert!(!clear_backend);
            assert_eq!(step_cwd, None);
            assert!(!clear_step_cwd);
            assert!(!run_in_background);
            assert!(foreground);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow update-step command: {other:?}"),
    }

    match set_context.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::SetSharedContext {
                    workflow_name,
                    value,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(value, Some(String::from("ship carefully")));
            assert_eq!(script_path, None);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow set-shared-context command: {other:?}"),
    }

    match enable_continue.command {
        Some(Commands::Workflow {
            command:
                WorkflowCommands::EnableContinueOnError {
                    workflow_name,
                    script_path,
                    cwd,
                },
        }) => {
            assert_eq!(workflow_name, Some(String::from("release-review")));
            assert_eq!(script_path, None);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected workflow enable-continue-on-error command: {other:?}"),
    }
}

#[test]
fn parses_tasks_clear_command() {
    let cli = Cli::try_parse_from(["hellox", "tasks", "clear", "--completed"])
        .expect("parse tasks clear");

    match cli.command {
        Some(Commands::Tasks {
            command:
                TaskCommands::Clear {
                    completed,
                    all,
                    cwd,
                },
        }) => {
            assert!(completed);
            assert!(!all);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_mcp_add_stdio_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "mcp",
        "add-stdio",
        "filesystem",
        "--command",
        "npx",
        "--arg",
        "@modelcontextprotocol/server-filesystem",
        "--env",
        "MODE=rw",
        "--cwd",
        "workspace",
        "--scope",
        "project",
        "--description",
        "Workspace filesystem",
    ])
    .expect("parse mcp add-stdio");

    match cli.command {
        Some(Commands::Mcp {
            command:
                McpCommands::AddStdio {
                    server_name,
                    command,
                    args,
                    env,
                    cwd,
                    scope,
                    description,
                },
        }) => {
            assert_eq!(server_name, "filesystem");
            assert_eq!(command, "npx");
            assert_eq!(
                args,
                vec![String::from("@modelcontextprotocol/server-filesystem")]
            );
            assert_eq!(env, vec![String::from("MODE=rw")]);
            assert_eq!(cwd, Some(PathBuf::from("workspace")));
            assert_eq!(scope, McpScopeValue::Project);
            assert_eq!(description, Some(String::from("Workspace filesystem")));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_mcp_add_sse_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "mcp",
        "add-sse",
        "remote-docs",
        "--url",
        "https://example.test/sse",
        "--header",
        "Authorization=Bearer token",
    ])
    .expect("parse mcp add-sse");

    match cli.command {
        Some(Commands::Mcp {
            command:
                McpCommands::AddSse {
                    server_name,
                    url,
                    headers,
                    oauth_client_id,
                    oauth_authorize_url,
                    oauth_token_url,
                    oauth_redirect_url,
                    oauth_provider,
                    oauth_scopes,
                    oauth_login_hint,
                    oauth_account_id,
                    scope,
                    description,
                },
        }) => {
            assert_eq!(server_name, "remote-docs");
            assert_eq!(url, "https://example.test/sse");
            assert_eq!(headers, vec![String::from("Authorization=Bearer token")]);
            assert_eq!(oauth_client_id, None);
            assert_eq!(oauth_authorize_url, None);
            assert_eq!(oauth_token_url, None);
            assert_eq!(oauth_redirect_url, None);
            assert_eq!(oauth_provider, None);
            assert!(oauth_scopes.is_empty());
            assert_eq!(oauth_login_hint, None);
            assert_eq!(oauth_account_id, None);
            assert_eq!(scope, McpScopeValue::User);
            assert_eq!(description, None);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_mcp_enable_and_remove_commands() {
    let enable =
        Cli::try_parse_from(["hellox", "mcp", "enable", "filesystem"]).expect("parse mcp enable");
    let remove =
        Cli::try_parse_from(["hellox", "mcp", "remove", "filesystem"]).expect("parse mcp remove");

    match enable.command {
        Some(Commands::Mcp {
            command: McpCommands::Enable { server_name },
        }) => assert_eq!(server_name, "filesystem"),
        other => panic!("unexpected enable command: {other:?}"),
    }

    match remove.command {
        Some(Commands::Mcp {
            command: McpCommands::Remove { server_name },
        }) => assert_eq!(server_name, "filesystem"),
        other => panic!("unexpected remove command: {other:?}"),
    }
}

#[test]
fn parses_mcp_runtime_and_auth_commands() {
    let tools =
        Cli::try_parse_from(["hellox", "mcp", "tools", "filesystem"]).expect("parse mcp tools");
    let call = Cli::try_parse_from([
        "hellox",
        "mcp",
        "call",
        "filesystem",
        "read_file",
        "--input",
        "{\"path\":\"README.md\"}",
    ])
    .expect("parse mcp call");
    let read_resource = Cli::try_parse_from([
        "hellox",
        "mcp",
        "read-resource",
        "docs",
        "file:///README.md",
    ])
    .expect("parse mcp read-resource");
    let prompts =
        Cli::try_parse_from(["hellox", "mcp", "prompts", "docs"]).expect("parse mcp prompts");
    let get_prompt = Cli::try_parse_from([
        "hellox",
        "mcp",
        "get-prompt",
        "docs",
        "reviewer",
        "--input",
        "{\"audience\":\"dev\"}",
    ])
    .expect("parse mcp get-prompt");
    let auth_set = Cli::try_parse_from([
        "hellox",
        "mcp",
        "auth-set-token",
        "docs",
        "--bearer-token",
        "token-123",
    ])
    .expect("parse mcp auth-set-token");
    let oauth_set = Cli::try_parse_from([
        "hellox",
        "mcp",
        "auth-oauth-set",
        "docs",
        "--client-id",
        "client-123",
        "--authorize-url",
        "https://auth.example.test/authorize",
        "--token-url",
        "https://auth.example.test/token",
        "--redirect-url",
        "http://127.0.0.1:8910/callback",
        "--scope",
        "openid",
    ])
    .expect("parse mcp auth-oauth-set");
    let registry_install = Cli::try_parse_from([
        "hellox",
        "mcp",
        "registry-install",
        "ac.tandem/docs-mcp",
        "--server-name",
        "docs",
        "--scope",
        "project",
    ])
    .expect("parse mcp registry-install");

    match tools.command {
        Some(Commands::Mcp {
            command: McpCommands::Tools { server_name },
        }) => assert_eq!(server_name, "filesystem"),
        other => panic!("unexpected tools command: {other:?}"),
    }

    match call.command {
        Some(Commands::Mcp {
            command:
                McpCommands::Call {
                    server_name,
                    tool_name,
                    input,
                },
        }) => {
            assert_eq!(server_name, "filesystem");
            assert_eq!(tool_name, "read_file");
            assert_eq!(input, Some(String::from("{\"path\":\"README.md\"}")));
        }
        other => panic!("unexpected call command: {other:?}"),
    }

    match read_resource.command {
        Some(Commands::Mcp {
            command: McpCommands::ReadResource { server_name, uri },
        }) => {
            assert_eq!(server_name, "docs");
            assert_eq!(uri, "file:///README.md");
        }
        other => panic!("unexpected read-resource command: {other:?}"),
    }

    match prompts.command {
        Some(Commands::Mcp {
            command: McpCommands::Prompts { server_name },
        }) => assert_eq!(server_name, "docs"),
        other => panic!("unexpected prompts command: {other:?}"),
    }

    match get_prompt.command {
        Some(Commands::Mcp {
            command:
                McpCommands::GetPrompt {
                    server_name,
                    prompt_name,
                    input,
                },
        }) => {
            assert_eq!(server_name, "docs");
            assert_eq!(prompt_name, "reviewer");
            assert_eq!(input, Some(String::from("{\"audience\":\"dev\"}")));
        }
        other => panic!("unexpected get-prompt command: {other:?}"),
    }

    match auth_set.command {
        Some(Commands::Mcp {
            command:
                McpCommands::AuthSetToken {
                    server_name,
                    bearer_token,
                },
        }) => {
            assert_eq!(server_name, "docs");
            assert_eq!(bearer_token, "token-123");
        }
        other => panic!("unexpected auth-set-token command: {other:?}"),
    }

    match oauth_set.command {
        Some(Commands::Mcp {
            command:
                McpCommands::AuthOauthSet {
                    server_name,
                    client_id,
                    authorize_url,
                    token_url,
                    redirect_url,
                    provider,
                    scopes,
                    login_hint,
                    account_id,
                },
        }) => {
            assert_eq!(server_name, "docs");
            assert_eq!(client_id, "client-123");
            assert_eq!(authorize_url, "https://auth.example.test/authorize");
            assert_eq!(token_url, "https://auth.example.test/token");
            assert_eq!(redirect_url, "http://127.0.0.1:8910/callback");
            assert_eq!(provider, None);
            assert_eq!(scopes, vec![String::from("openid")]);
            assert_eq!(login_hint, None);
            assert_eq!(account_id, None);
        }
        other => panic!("unexpected auth-oauth-set command: {other:?}"),
    }

    match registry_install.command {
        Some(Commands::Mcp {
            command:
                McpCommands::RegistryInstall {
                    name,
                    server_name,
                    scope,
                },
        }) => {
            assert_eq!(name, "ac.tandem/docs-mcp");
            assert_eq!(server_name, Some(String::from("docs")));
            assert_eq!(scope, McpScopeValue::Project);
        }
        other => panic!("unexpected registry-install command: {other:?}"),
    }
}

#[test]
fn parses_plugin_install_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "plugin",
        "install",
        "plugins/filesystem",
        "--disabled",
    ])
    .expect("parse plugin install");

    match cli.command {
        Some(Commands::Plugin {
            command: PluginCommands::Install { source, disabled },
        }) => {
            assert_eq!(source, PathBuf::from("plugins/filesystem"));
            assert!(disabled);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_plugin_marketplace_add_command() {
    let cli = Cli::try_parse_from([
        "hellox",
        "plugin",
        "marketplace",
        "add",
        "official",
        "--url",
        "https://plugins.example.test/index.json",
        "--description",
        "Official feed",
    ])
    .expect("parse plugin marketplace add");

    match cli.command {
        Some(Commands::Plugin {
            command:
                PluginCommands::Marketplace {
                    command:
                        MarketplaceCommands::Add {
                            marketplace_name,
                            url,
                            description,
                        },
                },
        }) => {
            assert_eq!(marketplace_name, "official");
            assert_eq!(url, "https://plugins.example.test/index.json");
            assert_eq!(description, Some(String::from("Official feed")));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_plugin_enable_and_remove_commands() {
    let enable = Cli::try_parse_from(["hellox", "plugin", "enable", "filesystem"])
        .expect("parse plugin enable");
    let remove = Cli::try_parse_from(["hellox", "plugin", "remove", "filesystem"])
        .expect("parse plugin remove");

    match enable.command {
        Some(Commands::Plugin {
            command: PluginCommands::Enable { plugin_id },
        }) => assert_eq!(plugin_id, "filesystem"),
        other => panic!("unexpected enable command: {other:?}"),
    }

    match remove.command {
        Some(Commands::Plugin {
            command: PluginCommands::Remove { plugin_id },
        }) => assert_eq!(plugin_id, "filesystem"),
        other => panic!("unexpected remove command: {other:?}"),
    }
}

#[test]
fn parses_skills_and_hooks_commands() {
    let skills = Cli::try_parse_from(["hellox", "skills", "review"]).expect("parse skills");
    let hooks = Cli::try_parse_from(["hellox", "hooks", "pre_tool"]).expect("parse hooks");

    match skills.command {
        Some(Commands::Skills { name }) => assert_eq!(name, Some(String::from("review"))),
        other => panic!("unexpected skills command: {other:?}"),
    }

    match hooks.command {
        Some(Commands::Hooks { name }) => assert_eq!(name, Some(String::from("pre_tool"))),
        other => panic!("unexpected hooks command: {other:?}"),
    }
}

#[test]
fn parses_bridge_and_ide_commands() {
    let bridge = Cli::try_parse_from(["hellox", "bridge", "show-session", "session-123"])
        .expect("parse bridge show-session");
    let bridge_panel = Cli::try_parse_from(["hellox", "bridge", "panel", "session-123"])
        .expect("parse bridge panel");
    let ide = Cli::try_parse_from(["hellox", "ide", "status"]).expect("parse ide status");
    let ide_panel = Cli::try_parse_from(["hellox", "ide", "panel"]).expect("parse ide panel");

    match bridge.command {
        Some(Commands::Bridge {
            command: BridgeCommands::ShowSession { session_id },
        }) => assert_eq!(session_id, "session-123"),
        other => panic!("unexpected bridge command: {other:?}"),
    }

    match bridge_panel.command {
        Some(Commands::Bridge {
            command: BridgeCommands::Panel { session_id },
        }) => assert_eq!(session_id, Some(String::from("session-123"))),
        other => panic!("unexpected bridge panel command: {other:?}"),
    }

    match ide.command {
        Some(Commands::Ide {
            command: IdeCommands::Status,
        }) => {}
        other => panic!("unexpected ide command: {other:?}"),
    }

    match ide_panel.command {
        Some(Commands::Ide {
            command: IdeCommands::Panel,
        }) => {}
        other => panic!("unexpected ide panel command: {other:?}"),
    }
}

#[test]
fn parses_remote_env_teleport_and_assistant_commands() {
    let remote_panel = Cli::try_parse_from(["hellox", "remote-env", "panel", "dev"])
        .expect("parse remote-env panel");
    let remote_env = Cli::try_parse_from([
        "hellox",
        "remote-env",
        "add",
        "dev",
        "--url",
        "https://remote.example.test",
        "--token-env",
        "REMOTE_TOKEN",
        "--account-id",
        "account-1",
        "--device-id",
        "device-1",
    ])
    .expect("parse remote-env add");
    let teleport = Cli::try_parse_from([
        "hellox",
        "teleport",
        "plan",
        "dev",
        "--session-id",
        "session-123",
    ])
    .expect("parse teleport plan");
    let teleport_panel = Cli::try_parse_from([
        "hellox",
        "teleport",
        "panel",
        "dev",
        "--session-id",
        "session-123",
    ])
    .expect("parse teleport panel");
    let teleport_connect = Cli::try_parse_from([
        "hellox",
        "teleport",
        "connect",
        "dev",
        "--session-id",
        "session-123",
    ])
    .expect("parse teleport connect");
    let assistant = Cli::try_parse_from(["hellox", "assistant", "show", "session-123"])
        .expect("parse assistant show");

    match remote_env.command {
        Some(Commands::RemoteEnv {
            command:
                RemoteEnvCommands::Add {
                    environment_name,
                    url,
                    token_env,
                    account_id,
                    device_id,
                    description,
                },
        }) => {
            assert_eq!(environment_name, "dev");
            assert_eq!(url, "https://remote.example.test");
            assert_eq!(token_env, Some(String::from("REMOTE_TOKEN")));
            assert_eq!(account_id, Some(String::from("account-1")));
            assert_eq!(device_id, Some(String::from("device-1")));
            assert_eq!(description, None);
        }
        other => panic!("unexpected remote-env command: {other:?}"),
    }

    match remote_panel.command {
        Some(Commands::RemoteEnv {
            command: RemoteEnvCommands::Panel { environment_name },
        }) => assert_eq!(environment_name, Some(String::from("dev"))),
        other => panic!("unexpected remote-env panel command: {other:?}"),
    }

    match teleport.command {
        Some(Commands::Teleport {
            command:
                TeleportCommands::Plan {
                    environment_name,
                    session_id,
                    model,
                    cwd,
                },
        }) => {
            assert_eq!(environment_name, "dev");
            assert_eq!(session_id, Some(String::from("session-123")));
            assert_eq!(model, None);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected teleport command: {other:?}"),
    }

    match teleport_panel.command {
        Some(Commands::Teleport {
            command:
                TeleportCommands::Panel {
                    environment_name,
                    session_id,
                    model,
                    cwd,
                },
        }) => {
            assert_eq!(environment_name, "dev");
            assert_eq!(session_id, Some(String::from("session-123")));
            assert_eq!(model, None);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected teleport panel command: {other:?}"),
    }

    match teleport_connect.command {
        Some(Commands::Teleport {
            command:
                TeleportCommands::Connect {
                    environment_name,
                    session_id,
                    model,
                    cwd,
                },
        }) => {
            assert_eq!(environment_name, "dev");
            assert_eq!(session_id, Some(String::from("session-123")));
            assert_eq!(model, None);
            assert_eq!(cwd, None);
        }
        other => panic!("unexpected teleport connect command: {other:?}"),
    }

    match assistant.command {
        Some(Commands::Assistant {
            command:
                AssistantCommands::Show {
                    session_id,
                    environment_name,
                },
        }) => {
            assert_eq!(session_id, "session-123");
            assert_eq!(environment_name, None);
        }
        other => panic!("unexpected assistant command: {other:?}"),
    }
}

#[test]
fn parses_server_commands() {
    let status = Cli::try_parse_from(["hellox", "server", "status"]).expect("parse server status");
    let create = Cli::try_parse_from([
        "hellox",
        "server",
        "create-session",
        "--session-id",
        "session-123",
        "--model",
        "sonnet",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse server create-session");

    match status.command {
        Some(Commands::Server {
            command: ServerCommands::Status { config },
        }) => assert_eq!(config, None),
        other => panic!("unexpected server status command: {other:?}"),
    }

    match create.command {
        Some(Commands::Server {
            command:
                ServerCommands::CreateSession {
                    config,
                    base_url,
                    session_id,
                    model,
                    cwd,
                },
        }) => {
            assert_eq!(config, None);
            assert_eq!(base_url, None);
            assert_eq!(session_id, Some(String::from("session-123")));
            assert_eq!(model, Some(String::from("sonnet")));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
        }
        other => panic!("unexpected server create-session command: {other:?}"),
    }
}

#[test]
fn parses_auth_and_sync_commands() {
    let auth = Cli::try_parse_from([
        "hellox",
        "auth",
        "login",
        "account-1",
        "--provider",
        "hellox-remote",
        "--access-token",
        "token-123",
        "--scope",
        "user:profile",
    ])
    .expect("parse auth login");
    let device = Cli::try_parse_from([
        "hellox",
        "auth",
        "trust-device",
        "account-1",
        "workstation",
        "--scope",
        "remote:sessions",
    ])
    .expect("parse auth trust-device");
    let sync = Cli::try_parse_from([
        "hellox",
        "sync",
        "team-memory-put",
        "repo-1",
        "architecture",
        "keep-it-simple",
    ])
    .expect("parse sync team-memory-put");
    let sync_panel = Cli::try_parse_from(["hellox", "sync", "team-memory-panel", "repo-1"])
        .expect("parse sync team-memory-panel");
    let settings_push = Cli::try_parse_from(["hellox", "sync", "settings-push", "dev"])
        .expect("parse sync settings-push");

    match auth.command {
        Some(Commands::Auth {
            command:
                AuthCommands::Login {
                    account_id,
                    provider,
                    access_token,
                    refresh_token,
                    scopes,
                },
        }) => {
            assert_eq!(account_id, "account-1");
            assert_eq!(provider, "hellox-remote");
            assert_eq!(access_token, "token-123");
            assert_eq!(refresh_token, None);
            assert_eq!(scopes, vec![String::from("user:profile")]);
        }
        other => panic!("unexpected auth command: {other:?}"),
    }

    match device.command {
        Some(Commands::Auth {
            command:
                AuthCommands::TrustDevice {
                    account_id,
                    device_name,
                    scopes,
                },
        }) => {
            assert_eq!(account_id, "account-1");
            assert_eq!(device_name, "workstation");
            assert_eq!(scopes, vec![String::from("remote:sessions")]);
        }
        other => panic!("unexpected auth device command: {other:?}"),
    }

    match sync.command {
        Some(Commands::Sync {
            command:
                SyncCommands::TeamMemoryPut {
                    repo_id,
                    key,
                    content,
                },
        }) => {
            assert_eq!(repo_id, "repo-1");
            assert_eq!(key, "architecture");
            assert_eq!(content, "keep-it-simple");
        }
        other => panic!("unexpected sync command: {other:?}"),
    }

    match sync_panel.command {
        Some(Commands::Sync {
            command: SyncCommands::TeamMemoryPanel { repo_id },
        }) => {
            assert_eq!(repo_id, "repo-1");
        }
        other => panic!("unexpected sync panel command: {other:?}"),
    }

    match settings_push.command {
        Some(Commands::Sync {
            command:
                SyncCommands::SettingsPush {
                    environment_name,
                    config,
                },
        }) => {
            assert_eq!(environment_name, "dev");
            assert_eq!(config, None);
        }
        other => panic!("unexpected sync push command: {other:?}"),
    }
}
