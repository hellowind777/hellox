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
        Some(action) if action == "dashboard" || action == "tui" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::Dashboard {
                workflow_name,
                script_path,
            }
        }
        Some(action) if action == "overview" || action == "selector" || action == "inspect" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::Overview {
                workflow_name,
                script_path,
            }
        }
        Some(action) if action == "panel" || action == "edit" || action == "board" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, consumed) = parse_workflow_lookup(&values);
            let step_number = values
                .get(consumed)
                .and_then(|value| value.parse::<usize>().ok());
            WorkflowCommand::Panel {
                workflow_name,
                script_path,
                step_number,
            }
        }
        Some(action) if action == "runs" || action == "history" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::Runs {
                workflow_name,
                script_path,
            }
        }
        Some(action) if action == "validate" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::Validate {
                workflow_name,
                script_path,
            }
        }
        Some(action) if action == "show-run" => WorkflowCommand::ShowRun {
            run_id: parts.next().map(ToString::to_string),
            step_number: parts.next().and_then(|value| value.parse::<usize>().ok()),
        },
        Some(action) if action == "last-run" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, consumed) = parse_workflow_lookup(&values);
            WorkflowCommand::LastRun {
                workflow_name,
                script_path,
                step_number: values
                    .get(consumed)
                    .and_then(|value| value.parse::<usize>().ok()),
            }
        }
        Some(action) if action == "show" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::Show {
                workflow_name,
                script_path,
            }
        }
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
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, consumed) = parse_workflow_lookup(&values);
            WorkflowCommand::RemoveStep {
                workflow_name,
                script_path,
                step_number: values
                    .get(consumed)
                    .and_then(|value| value.parse::<usize>().ok()),
            }
        }
        Some(action) if action == "set-shared-context" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, consumed) = parse_workflow_lookup(&values);
            let value = joined_remainder(values.into_iter().skip(consumed).collect::<Vec<_>>());
            WorkflowCommand::SetSharedContext {
                workflow_name,
                script_path,
                value,
            }
        }
        Some(action) if action == "clear-shared-context" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::ClearSharedContext {
                workflow_name,
                script_path,
            }
        }
        Some(action) if action == "disable-continue-on-error" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::DisableContinueOnError {
                workflow_name,
                script_path,
            }
        }
        Some(action) if action == "enable-continue-on-error" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, _) = parse_workflow_lookup(&values);
            WorkflowCommand::EnableContinueOnError {
                workflow_name,
                script_path,
            }
        }
        Some(action) if action == "run" => {
            let values = trimmed[action.len()..]
                .trim()
                .split_whitespace()
                .collect::<Vec<_>>();
            let (workflow_name, script_path, consumed) = parse_workflow_lookup(&values);
            let shared_context = joined_remainder(values.into_iter().skip(consumed).collect());
            WorkflowCommand::Run {
                workflow_name,
                script_path,
                shared_context,
            }
        }
        Some(action) if action == "help" => WorkflowCommand::Help,
        Some(workflow_name) => WorkflowCommand::Run {
            workflow_name: Some(workflow_name),
            script_path: None,
            shared_context: joined_remainder(parts.collect::<Vec<_>>()),
        },
    }
}

fn parse_workflow_lookup(values: &[&str]) -> (Option<String>, Option<String>, usize) {
    match values {
        ["--script-path", path, ..] => (None, Some((*path).to_string()), 2),
        [workflow_name, ..] => (Some((*workflow_name).to_string()), None, 1),
        [] => (None, None, 0),
    }
}

fn joined_remainder(parts: Vec<&str>) -> Option<String> {
    let value = parts.join(" ");
    (!value.is_empty()).then_some(value)
}
