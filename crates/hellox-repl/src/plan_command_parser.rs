use crate::command_types::PlanCommand;

pub fn parse_plan_command(remainder: &str) -> PlanCommand {
    let trimmed = remainder.trim();
    let mut parts = trimmed.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => PlanCommand::Show,
        Some(action) if action == "show" => PlanCommand::Show,
        Some(action) if action == "panel" => PlanCommand::Panel {
            step_number: parts.next().and_then(|value| value.parse::<usize>().ok()),
        },
        Some(action) if action == "enter" => PlanCommand::Enter,
        Some(action) if action == "add" => parse_plan_add_command(trimmed[action.len()..].trim()),
        Some(action) if action == "update" || action == "set" => {
            parse_plan_update_command(trimmed[action.len()..].trim())
        }
        Some(action) if action == "remove" || action == "rm" => PlanCommand::Remove {
            step_number: parts.next().and_then(|value| value.parse::<usize>().ok()),
        },
        Some(action) if action == "allow" => PlanCommand::Allow {
            prompt: joined_remainder(parts.collect::<Vec<_>>()),
        },
        Some(action) if action == "disallow" => PlanCommand::Disallow {
            prompt: joined_remainder(parts.collect::<Vec<_>>()),
        },
        Some(action) if action == "clear" || action == "reset" => PlanCommand::Clear,
        Some(action) if action == "exit" || action == "accept" => {
            let tail = trimmed[action.len()..].trim();
            let (steps, allowed_prompts) = parse_plan_segments(tail);
            PlanCommand::Exit {
                steps,
                allowed_prompts,
            }
        }
        Some(_) => PlanCommand::Help,
    }
}

#[derive(Clone, Copy)]
enum PlanSegmentKind {
    Step,
    Allow,
}

fn parse_plan_segments(remainder: &str) -> (Vec<String>, Vec<String>) {
    let mut steps = Vec::new();
    let mut allowed_prompts = Vec::new();
    let mut current_kind = None;
    let mut current_value = String::new();

    for token in remainder.split_whitespace() {
        let next_kind = match token {
            "--step" => Some(PlanSegmentKind::Step),
            "--allow" => Some(PlanSegmentKind::Allow),
            _ => None,
        };

        if let Some(next_kind) = next_kind {
            push_plan_segment(
                &mut steps,
                &mut allowed_prompts,
                current_kind.take(),
                &mut current_value,
            );
            current_kind = Some(next_kind);
            continue;
        }

        if !current_value.is_empty() {
            current_value.push(' ');
        }
        current_value.push_str(token);
    }

    push_plan_segment(
        &mut steps,
        &mut allowed_prompts,
        current_kind,
        &mut current_value,
    );

    (steps, allowed_prompts)
}

fn push_plan_segment(
    steps: &mut Vec<String>,
    allowed_prompts: &mut Vec<String>,
    kind: Option<PlanSegmentKind>,
    current_value: &mut String,
) {
    let value = current_value.trim().to_string();
    current_value.clear();
    if value.is_empty() {
        return;
    }

    match kind {
        Some(PlanSegmentKind::Step) => steps.push(value),
        Some(PlanSegmentKind::Allow) => allowed_prompts.push(value),
        None => {}
    }
}

fn parse_plan_add_command(remainder: &str) -> PlanCommand {
    let mut index = None;
    let mut step_tokens = Vec::new();
    let mut tokens = remainder.split_whitespace();

    while let Some(token) = tokens.next() {
        if token == "--index" && step_tokens.is_empty() {
            index = tokens.next().and_then(|value| value.parse::<usize>().ok());
            continue;
        }
        step_tokens.push(token);
        step_tokens.extend(tokens);
        break;
    }

    PlanCommand::Add {
        step: joined_remainder(step_tokens),
        index,
    }
}

fn parse_plan_update_command(remainder: &str) -> PlanCommand {
    let mut parts = remainder.split_whitespace();
    let step_number = parts.next().and_then(|value| value.parse::<usize>().ok());
    let step = joined_remainder(parts.collect::<Vec<_>>());
    PlanCommand::Update { step_number, step }
}

fn joined_remainder(parts: Vec<&str>) -> Option<String> {
    let value = parts.join(" ");
    (!value.is_empty()).then_some(value)
}
