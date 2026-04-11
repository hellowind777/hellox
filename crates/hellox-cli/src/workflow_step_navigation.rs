#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkflowStepNavigationShortcut {
    Next,
    Previous,
    First,
    Last,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkflowStepNavigationResult {
    pub(crate) step_number: usize,
    pub(crate) changed: bool,
}

impl WorkflowStepNavigationShortcut {
    pub(crate) fn boundary_name(self) -> &'static str {
        match self {
            Self::Next | Self::Last => "last",
            Self::Previous | Self::First => "first",
        }
    }
}

pub(crate) fn parse_workflow_step_navigation(
    input: &str,
) -> Option<Result<WorkflowStepNavigationShortcut, String>> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?.to_ascii_lowercase();
    let tail = parts.collect::<Vec<_>>();

    Some(match command.as_str() {
        "next" => parse_no_tail(&tail, "Usage: next").map(|_| WorkflowStepNavigationShortcut::Next),
        "prev" | "previous" => {
            parse_no_tail(&tail, "Usage: prev").map(|_| WorkflowStepNavigationShortcut::Previous)
        }
        "first" => {
            parse_no_tail(&tail, "Usage: first").map(|_| WorkflowStepNavigationShortcut::First)
        }
        "last" => parse_no_tail(&tail, "Usage: last").map(|_| WorkflowStepNavigationShortcut::Last),
        _ => return None,
    })
}

pub(crate) fn execute_workflow_step_navigation(
    selected_step: usize,
    step_count: usize,
    shortcut: WorkflowStepNavigationShortcut,
) -> Result<WorkflowStepNavigationResult, String> {
    if step_count == 0 {
        return Err("No workflow steps are available yet.".to_string());
    }

    let selected_step = selected_step.clamp(1, step_count);
    let step_number = match shortcut {
        WorkflowStepNavigationShortcut::Next => (selected_step + 1).min(step_count),
        WorkflowStepNavigationShortcut::Previous => {
            if selected_step > 1 {
                selected_step - 1
            } else {
                1
            }
        }
        WorkflowStepNavigationShortcut::First => 1,
        WorkflowStepNavigationShortcut::Last => step_count,
    };

    Ok(WorkflowStepNavigationResult {
        step_number,
        changed: step_number != selected_step,
    })
}

fn parse_no_tail(parts: &[&str], usage: &str) -> Result<(), String> {
    if parts.is_empty() {
        Ok(())
    } else {
        Err(usage.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute_workflow_step_navigation, parse_workflow_step_navigation,
        WorkflowStepNavigationResult, WorkflowStepNavigationShortcut,
    };

    #[test]
    fn parses_navigation_shortcuts() {
        assert_eq!(
            parse_workflow_step_navigation("next"),
            Some(Ok(WorkflowStepNavigationShortcut::Next))
        );
        assert_eq!(
            parse_workflow_step_navigation("prev"),
            Some(Ok(WorkflowStepNavigationShortcut::Previous))
        );
        assert_eq!(
            parse_workflow_step_navigation("first"),
            Some(Ok(WorkflowStepNavigationShortcut::First))
        );
        assert_eq!(
            parse_workflow_step_navigation("last"),
            Some(Ok(WorkflowStepNavigationShortcut::Last))
        );
        assert_eq!(
            parse_workflow_step_navigation("previous"),
            Some(Ok(WorkflowStepNavigationShortcut::Previous))
        );
    }

    #[test]
    fn invalid_navigation_shortcuts_return_usage() {
        assert_eq!(
            parse_workflow_step_navigation("next now"),
            Some(Err("Usage: next".to_string()))
        );
        assert_eq!(parse_workflow_step_navigation("/workflow panel"), None);
        assert_eq!(parse_workflow_step_navigation("mystery"), None);
    }

    #[test]
    fn executes_navigation_with_boundary_handling() {
        assert_eq!(
            execute_workflow_step_navigation(1, 3, WorkflowStepNavigationShortcut::Next),
            Ok(WorkflowStepNavigationResult {
                step_number: 2,
                changed: true,
            })
        );
        assert_eq!(
            execute_workflow_step_navigation(3, 3, WorkflowStepNavigationShortcut::Next),
            Ok(WorkflowStepNavigationResult {
                step_number: 3,
                changed: false,
            })
        );
        assert_eq!(
            execute_workflow_step_navigation(3, 3, WorkflowStepNavigationShortcut::Previous),
            Ok(WorkflowStepNavigationResult {
                step_number: 2,
                changed: true,
            })
        );
        assert_eq!(
            execute_workflow_step_navigation(2, 3, WorkflowStepNavigationShortcut::First),
            Ok(WorkflowStepNavigationResult {
                step_number: 1,
                changed: true,
            })
        );
        assert_eq!(
            execute_workflow_step_navigation(2, 3, WorkflowStepNavigationShortcut::Last),
            Ok(WorkflowStepNavigationResult {
                step_number: 3,
                changed: true,
            })
        );
        assert_eq!(
            execute_workflow_step_navigation(1, 0, WorkflowStepNavigationShortcut::Next),
            Err("No workflow steps are available yet.".to_string())
        );
    }
}
