use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{HelloxConfig, PermissionMode};

use crate::tasks::{load_tasks, save_tasks, TaskItem};

use super::commands::{ReplCommand, TaskCommand};
use super::format::help_text;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-tasks-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session(root: PathBuf) -> AgentSession {
    AgentSession::create(
        GatewayClient::new("http://127.0.0.1:7821"),
        default_tool_registry(),
        root.join(".hellox").join("config.toml"),
        root,
        "powershell",
        AgentOptions::default(),
        PermissionMode::BypassPermissions,
        None,
        None,
        false,
        None,
    )
}

fn metadata(root: &PathBuf) -> ReplMetadata {
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: PathBuf::from("C:/Users/test/.hellox/config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

#[test]
fn parse_task_commands() {
    assert_eq!(
        super::commands::parse_command("/tasks"),
        Some(ReplCommand::Tasks(TaskCommand::List))
    );
    assert_eq!(
        super::commands::parse_command("/tasks panel"),
        Some(ReplCommand::Tasks(TaskCommand::Panel { task_id: None }))
    );
    assert_eq!(
        super::commands::parse_command("/tasks panel task-7"),
        Some(ReplCommand::Tasks(TaskCommand::Panel {
            task_id: Some(String::from("task-7"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/tasks add finish docs"),
        Some(ReplCommand::Tasks(TaskCommand::Add {
            content: Some(String::from("finish docs"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/tasks done task-7"),
        Some(ReplCommand::Tasks(TaskCommand::Done {
            task_id: Some(String::from("task-7"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/tasks show task-7"),
        Some(ReplCommand::Tasks(TaskCommand::Show {
            task_id: Some(String::from("task-7"))
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/tasks update task-7 --status in_progress --output started work --clear-description"
        ),
        Some(ReplCommand::Tasks(TaskCommand::Update {
            task_id: Some(String::from("task-7")),
            content: None,
            priority: None,
            clear_priority: false,
            description: None,
            clear_description: true,
            status: Some(String::from("in_progress")),
            output: Some(String::from("started work")),
            clear_output: false,
        }))
    );
}

#[test]
fn help_text_lists_task_commands() {
    let text = help_text();
    assert!(text.contains("/tasks"));
    assert!(text.contains("/tasks panel"));
    assert!(text.contains("/tasks add <text>"));
    assert!(text.contains("/tasks show <id>"));
    assert!(text.contains("/tasks output <id>"));
}

#[test]
fn tasks_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    save_tasks(
        &root,
        &[
            TaskItem {
                id: String::from("task-1"),
                content: String::from("ship feature"),
                status: String::from("pending"),
                priority: None,
                description: None,
                output: None,
            },
            TaskItem {
                id: String::from("task-2"),
                content: String::from("write docs"),
                status: String::from("pending"),
                priority: None,
                description: None,
                output: None,
            },
        ],
    )
    .expect("save tasks");

    let driver = super::CliReplDriver::new();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            match driver
                .handle_repl_input_async("1", &mut session, &metadata)
                .await
                .expect("submit")
            {
                ReplAction::Submit(text) => assert_eq!(text, "1"),
                other => panic!("expected submit action, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("/tasks panel", &mut session, &metadata)
                    .await
                    .expect("open tasks panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::TaskPanelList { task_ids }) => {
                    assert_eq!(task_ids, vec!["task-1".to_string(), "task-2".to_string()]);
                }
                other => panic!("expected task selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("2", &mut session, &metadata)
                    .await
                    .expect("select task"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}

#[test]
fn handle_tasks_add_writes_workspace_task() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    let action =
        handle_repl_input("/tasks add finish docs", &mut session, &metadata).expect("tasks add");
    assert_eq!(action, ReplAction::Continue);

    let tasks = load_tasks(&root).expect("load tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].content, "finish docs");
    assert_eq!(tasks[0].status, "pending");
}

#[test]
fn handle_tasks_done_updates_status() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    save_tasks(
        &root,
        &[TaskItem {
            id: String::from("task-1"),
            content: String::from("ship feature"),
            status: String::from("pending"),
            priority: None,
            description: None,
            output: None,
        }],
    )
    .expect("save tasks");

    let action =
        handle_repl_input("/tasks done task-1", &mut session, &metadata).expect("tasks done");
    assert_eq!(action, ReplAction::Continue);

    let tasks = load_tasks(&root).expect("load tasks");
    assert_eq!(tasks[0].status, "completed");
}

#[test]
fn handle_tasks_panel_renders_selector_and_lens() {
    let root = temp_dir();
    let session = session(root.clone());
    save_tasks(
        &root,
        &[TaskItem {
            id: String::from("task-1"),
            content: String::from("ship feature"),
            status: String::from("in_progress"),
            priority: Some(String::from("high")),
            description: Some(String::from("needs review")),
            output: Some(String::from("waiting on tests")),
        }],
    )
    .expect("save tasks");

    let list =
        super::task_actions::handle_task_command(TaskCommand::Panel { task_id: None }, &session)
            .expect("tasks panel list");
    assert!(list.contains("== Task selector =="));
    assert!(list.contains("hellox tasks panel task-1"));
    assert!(list.contains("/tasks panel task-1"));

    let detail = super::task_actions::handle_task_command(
        TaskCommand::Panel {
            task_id: Some(String::from("task-1")),
        },
        &session,
    )
    .expect("tasks panel detail");
    assert!(detail.contains("== Task lens =="));
    assert!(detail.contains("> [1] task-1 — IN_PROGRESS"));
    assert!(detail.contains("description: needs review"));
}

#[test]
fn handle_tasks_show_update_output_and_stop() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    save_tasks(
        &root,
        &[TaskItem {
            id: String::from("task-1"),
            content: String::from("ship feature"),
            status: String::from("pending"),
            priority: None,
            description: Some(String::from("needs review")),
            output: None,
        }],
    )
    .expect("save tasks");

    assert_eq!(
        handle_repl_input("/tasks show task-1", &mut session, &metadata).expect("tasks show"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/tasks update task-1 --status in_progress --output started work --clear-description",
            &mut session,
            &metadata,
        )
        .expect("tasks update"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/tasks output task-1", &mut session, &metadata).expect("tasks output"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/tasks stop task-1 blocked by host",
            &mut session,
            &metadata
        )
        .expect("tasks stop"),
        ReplAction::Continue
    );

    let tasks = load_tasks(&root).expect("load tasks");
    assert_eq!(tasks[0].status, "cancelled");
    assert_eq!(tasks[0].description, None);
    assert_eq!(tasks[0].output.as_deref(), Some("blocked by host"));
}
