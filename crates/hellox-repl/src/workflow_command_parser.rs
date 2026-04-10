use crate::workflow_command_parser_authoring::{
    parse_workflow_add_step_command, parse_workflow_duplicate_step_command,
    parse_workflow_move_step_command, parse_workflow_update_step_command,
};
use crate::workflow_command_types::WorkflowCommand;

pub fn parse_workflow_command(remainder: &str) -> WorkflowCommand {
    let trimmed = remainder.trim();
    let mut parts = trimmed.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => WorkflowCommand::List,
        Some(action) if action == "list" => WorkflowCommand::List,
        Some(action) if action == "overview" || action == "selector" || action == "inspect" => {
            WorkflowCommand::Overview {
                workflow_name: parts.next().map(ToString::to_string),
            }
        }
        Some(action) if action == "panel" || action == "edit" || action == "board" => {
            let workflow_name = parts.next().map(ToString::to_string);
            let step_number = parts.next().and_then(|value| value.parse::<usize>().ok());
            WorkflowCommand::Panel {
                workflow_name,
                step_number,
            }
        }
        Some(action) if action == "runs" || action == "history" => WorkflowCommand::Runs {
            workflow_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "validate" => WorkflowCommand::Validate {
            workflow_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "show-run" => WorkflowCommand::ShowRun {
            run_id: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "last-run" => WorkflowCommand::LastRun {
            workflow_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "show" => WorkflowCommand::Show {
            workflow_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "init" => WorkflowCommand::Init {
            workflow_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "add-step" || action == "add" => {
            parse_workflow_add_step_command(trimmed[action.len()..].trim())
        }
        Some(action) if action == "update-step" || action == "update" || action == "set-step" => {
            parse_workflow_update_step_command(trimmed[action.len()..].trim())
        }
        Some(action)
            if action == "duplicate-step"
                || action == "duplicate"
                || action == "clone-step"
                || action == "clone"
                || action == "copy-step" =>
        {
            parse_workflow_duplicate_step_command(trimmed[action.len()..].trim())
        }
        Some(action) if action == "move-step" || action == "move" || action == "reorder-step" => {
            parse_workflow_move_step_command(trimmed[action.len()..].trim())
        }
        Some(action) if action == "remove-step" || action == "remove" || action == "rm-step" => {
            let mut values = trimmed[action.len()..].trim().split_whitespace();
            WorkflowCommand::RemoveStep {
                workflow_name: values.next().map(ToString::to_string),
                step_number: values.next().and_then(|value| value.parse::<usize>().ok()),
            }
        }
        Some(action) if action == "set-shared-context" => {
            let mut values = trimmed[action.len()..].trim().split_whitespace();
            let workflow_name = values.next().map(ToString::to_string);
            let value = joined_remainder(values.collect::<Vec<_>>());
            WorkflowCommand::SetSharedContext {
                workflow_name,
                value,
            }
        }
        Some(action) if action == "clear-shared-context" => WorkflowCommand::ClearSharedContext {
            workflow_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "enable-continue-on-error" => {
            WorkflowCommand::EnableContinueOnError {
                workflow_name: parts.next().map(ToString::to_string),
            }
        }
        Some(action) if action == "disable-continue-on-error" => {
            WorkflowCommand::DisableContinueOnError {
                workflow_name: parts.next().map(ToString::to_string),
            }
        }
        Some(action) if action == "run" => {
            let workflow_name = parts.next().map(ToString::to_string);
            let shared_context = joined_remainder(parts.collect::<Vec<_>>());
            WorkflowCommand::Run {
                workflow_name,
                shared_context,
            }
        }
        Some(action) if action == "help" => WorkflowCommand::Help,
        Some(workflow_name) => WorkflowCommand::Run {
            workflow_name: Some(workflow_name),
            shared_context: joined_remainder(parts.collect::<Vec<_>>()),
        },
    }
}

fn joined_remainder(parts: Vec<&str>) -> Option<String> {
    let value = parts.join(" ");
    (!value.is_empty()).then_some(value)
}
