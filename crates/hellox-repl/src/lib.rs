mod command_parser;
mod command_types;
mod input_helper;
mod mcp_command_types;
mod plan_command_parser;
mod plugin_command_types;
mod remote_command_parser;
mod runtime;
mod style_parser;
mod workflow_command_parser;
mod workflow_command_parser_authoring;
mod workflow_command_types;

pub use command_parser::parse_command;
pub use command_types::{
    AssistantCommand, BridgeCommand, BriefCommand, ConfigCommand, IdeCommand, InstallCommand,
    MarketplaceCommand, McpCommand, MemoryCommand, ModelCommand, OutputStyleCommand,
    PersonaCommand, PlanCommand, PluginCommand, PromptFragmentCommand, RemoteEnvCommand,
    ReplCommand, SessionCommand, TaskCommand, TeleportCommand, ToolsCommand, UpgradeCommand,
    WorkflowCommand,
};
pub use input_helper::{ReplCompletion, ReplPromptState};
pub use runtime::{run_repl_loop, ReplAction, ReplExit, ReplLoopDriver, ReplMetadata};

#[cfg(test)]
mod tests {
    use super::{parse_command, ReplCommand, WorkflowCommand};

    #[test]
    fn parses_basic_repl_commands() {
        assert_eq!(parse_command("/help"), Some(ReplCommand::Help));
        assert_eq!(parse_command("?"), Some(ReplCommand::Shortcuts));
        assert_eq!(parse_command("/shortcuts"), Some(ReplCommand::Shortcuts));
        assert_eq!(
            parse_command("/workflow dashboard release-review"),
            Some(ReplCommand::Workflow(WorkflowCommand::Dashboard {
                workflow_name: Some(String::from("release-review")),
                script_path: None,
            }))
        );
        assert_eq!(
            parse_command("/workflow dashboard --script-path scripts/custom-release.json"),
            Some(ReplCommand::Workflow(WorkflowCommand::Dashboard {
                workflow_name: None,
                script_path: Some(String::from("scripts/custom-release.json")),
            }))
        );
        assert_eq!(
            parse_command("/workflow overview --script-path scripts/custom-release.json"),
            Some(ReplCommand::Workflow(WorkflowCommand::Overview {
                workflow_name: None,
                script_path: Some(String::from("scripts/custom-release.json")),
            }))
        );
        assert_eq!(
            parse_command("/workflow runs --script-path scripts/custom-release.json"),
            Some(ReplCommand::Workflow(WorkflowCommand::Runs {
                workflow_name: None,
                script_path: Some(String::from("scripts/custom-release.json")),
            }))
        );
        assert_eq!(
            parse_command("/workflow last-run --script-path scripts/custom-release.json 2"),
            Some(ReplCommand::Workflow(WorkflowCommand::LastRun {
                workflow_name: None,
                script_path: Some(String::from("scripts/custom-release.json")),
                step_number: Some(2),
            }))
        );
        assert_eq!(
            parse_command("/workflow demo"),
            Some(ReplCommand::Workflow(WorkflowCommand::Run {
                workflow_name: Some(String::from("demo")),
                script_path: None,
                shared_context: None,
            }))
        );
        assert_eq!(parse_command("plain prompt"), None);
    }
}
