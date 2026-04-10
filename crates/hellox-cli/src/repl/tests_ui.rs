use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{load_or_default, HelloxConfig, PermissionMode};

use super::commands::{BriefCommand, ConfigCommand, PlanCommand, ReplCommand, ToolsCommand};
use super::format::help_text;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-ui-{suffix}"));
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
        config_path: root.join(".hellox").join("config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join(".hellox").join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

#[test]
fn parse_ui_commands() {
    assert_eq!(
        super::commands::parse_command("/brief"),
        Some(ReplCommand::Brief(BriefCommand::Show))
    );
    assert_eq!(
        super::commands::parse_command("/brief set ship release carefully"),
        Some(ReplCommand::Brief(BriefCommand::Set {
            message: Some(String::from("ship release carefully"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/tools mcp 5"),
        Some(ReplCommand::Tools(ToolsCommand::Search {
            query: Some(String::from("mcp")),
            limit: 5,
        }))
    );
    assert_eq!(
        super::commands::parse_command("/config set prompt.persona reviewer"),
        Some(ReplCommand::Config(ConfigCommand::Set {
            key: Some(String::from("prompt.persona")),
            value: Some(String::from("reviewer")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/config panel"),
        Some(ReplCommand::Config(ConfigCommand::Panel))
    );
    assert_eq!(
        super::commands::parse_command(
            "/plan exit --step completed:Audit docs --step in_progress:Implement config"
        ),
        Some(ReplCommand::Plan(PlanCommand::Exit {
            steps: vec![
                String::from("completed:Audit docs"),
                String::from("in_progress:Implement config"),
            ],
            allowed_prompts: Vec::new(),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/plan panel"),
        Some(ReplCommand::Plan(PlanCommand::Panel))
    );
    assert_eq!(
        super::commands::parse_command("/plan add --index 1 pending:Draft implementation plan"),
        Some(ReplCommand::Plan(PlanCommand::Add {
            step: Some(String::from("pending:Draft implementation plan")),
            index: Some(1),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/plan update 2 completed:Ship implementation"),
        Some(ReplCommand::Plan(PlanCommand::Update {
            step_number: Some(2),
            step: Some(String::from("completed:Ship implementation")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/plan allow continue implementation"),
        Some(ReplCommand::Plan(PlanCommand::Allow {
            prompt: Some(String::from("continue implementation")),
        }))
    );
}

#[test]
fn help_text_lists_ui_commands() {
    let text = help_text();
    assert!(text.contains("/brief"));
    assert!(text.contains("/tools <query>"));
    assert!(text.contains("/config set <key> <value>"));
    assert!(text.contains("/config panel"));
    assert!(text.contains("/plan add [--index <n>] <status>:<text>"));
    assert!(text.contains("/plan panel"));
    assert!(text.contains("/plan exit --step <status>:<text>..."));
}

#[test]
fn handle_brief_and_tools_commands_stay_in_repl() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    assert_eq!(
        handle_repl_input("/brief set stabilize workflow ui", &mut session, &metadata)
            .expect("brief set"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/brief", &mut session, &metadata).expect("brief show"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/tools mcp", &mut session, &metadata).expect("tools search"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/config set prompt.persona reviewer",
            &mut session,
            &metadata
        )
        .expect("config set"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/config panel", &mut session, &metadata).expect("config panel"),
        ReplAction::Continue
    );
    let config = load_or_default(Some(metadata.config_path.clone())).expect("load config");
    assert_eq!(config.prompt.persona.as_deref(), Some("reviewer"));

    assert_eq!(
        handle_repl_input("/plan enter", &mut session, &metadata).expect("plan enter"),
        ReplAction::Continue
    );
    assert!(session.planning_state().active);
    assert_eq!(
        handle_repl_input("/plan panel", &mut session, &metadata).expect("plan panel"),
        ReplAction::Continue
    );

    assert_eq!(
        handle_repl_input("/plan add completed:Audit docs", &mut session, &metadata,)
            .expect("plan add"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/plan add --index 1 in_progress:Implement plan authoring",
            &mut session,
            &metadata,
        )
        .expect("plan insert"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/plan allow continue implementation",
            &mut session,
            &metadata,
        )
        .expect("plan allow"),
        ReplAction::Continue
    );
    let planning = session.planning_state();
    assert!(planning.active);
    assert_eq!(planning.plan.len(), 2);
    assert_eq!(planning.plan[0].status, "in_progress");
    assert_eq!(
        planning.allowed_prompts,
        vec![String::from("continue implementation")]
    );

    assert_eq!(
        handle_repl_input(
            "/plan exit --step completed:Audit docs --allow continue implementation",
            &mut session,
            &metadata,
        )
        .expect("plan exit"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/plan update 1 completed:Ship plan authoring",
            &mut session,
            &metadata,
        )
        .expect("plan update"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input(
            "/plan disallow continue implementation",
            &mut session,
            &metadata,
        )
        .expect("plan disallow"),
        ReplAction::Continue
    );
    assert_eq!(
        handle_repl_input("/plan remove 1", &mut session, &metadata).expect("plan remove"),
        ReplAction::Continue
    );
    let planning = session.planning_state();
    assert!(!planning.active);
    assert!(planning.plan.is_empty());
    assert!(planning.allowed_prompts.is_empty());
}

#[tokio::test]
async fn plan_panel_renders_selector_and_lens() {
    let root = temp_dir();
    let mut session = session(root.clone());

    let mut planning = session.planning_state();
    planning.enter();
    planning
        .add_step(
            hellox_agent::PlanItem {
                status: String::from("in_progress"),
                step: String::from("Ship richer plan surface"),
            },
            None,
        )
        .expect("add step");
    planning
        .allow_prompt(String::from("continue implementation"))
        .expect("allow prompt");
    session
        .set_planning_state(planning)
        .expect("set planning state");

    let text = super::plan_actions::handle_plan_command(PlanCommand::Panel, &mut session)
        .await
        .expect("render plan panel");

    assert!(text.contains("== Accepted plan selector =="));
    assert!(text.contains("== Focused step lens =="));
    assert!(text.contains("== Allowed prompt selector =="));
    assert!(text.contains("/plan update 1 <status>:<text>"));
}
