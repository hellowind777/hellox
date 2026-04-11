use crate::workflow_command_types::WorkflowCommand;

#[derive(Clone, Copy)]
enum WorkflowStepSegmentKind {
    Name,
    Prompt,
    When,
    Model,
    Backend,
    StepCwd,
}

#[derive(Clone, Copy)]
enum WorkflowStepPatchKind {
    Name,
    Prompt,
    When,
    Model,
    Backend,
    StepCwd,
}

#[derive(Clone, Copy)]
enum WorkflowDuplicateSegmentKind {
    Name,
}

pub(super) fn parse_workflow_add_step_command(remainder: &str) -> WorkflowCommand {
    let tokens = remainder.split_whitespace().collect::<Vec<_>>();
    let mut workflow_name = None;
    let mut script_path = None;
    let mut name = None;
    let mut prompt = None;
    let mut index = None;
    let mut when = None;
    let mut model = None;
    let mut backend = None;
    let mut step_cwd = None;
    let mut run_in_background = false;
    let mut current_kind = None;
    let mut current_value = String::new();

    let mut token_index = 0;
    if let Some(first) = tokens.first() {
        if *first == "--script-path" {
            script_path = tokens.get(1).map(|value| (*value).to_string());
            token_index = usize::min(2, tokens.len());
        } else if !first.starts_with("--") {
            workflow_name = Some((*first).to_string());
            token_index = 1;
        }
    }

    while token_index < tokens.len() {
        let token = tokens[token_index];
        token_index += 1;
        let next_kind = match token {
            "--script-path" => {
                push_workflow_step_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                script_path = tokens.get(token_index).map(|value| (*value).to_string());
                token_index = usize::min(token_index + 1, tokens.len());
                current_kind = None;
                continue;
            }
            "--name" => Some(WorkflowStepSegmentKind::Name),
            "--prompt" => Some(WorkflowStepSegmentKind::Prompt),
            "--when" => Some(WorkflowStepSegmentKind::When),
            "--model" => Some(WorkflowStepSegmentKind::Model),
            "--backend" => Some(WorkflowStepSegmentKind::Backend),
            "--step-cwd" => Some(WorkflowStepSegmentKind::StepCwd),
            "--index" => {
                push_workflow_step_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                current_kind = None;
                continue;
            }
            "--background" => {
                push_workflow_step_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                run_in_background = true;
                current_kind = None;
                continue;
            }
            _ => None,
        };

        if let Some(next_kind) = next_kind {
            push_workflow_step_segment(
                &mut name,
                &mut prompt,
                &mut when,
                &mut model,
                &mut backend,
                &mut step_cwd,
                current_kind.take(),
                &mut current_value,
            );
            current_kind = Some(next_kind);
            continue;
        }

        if current_kind.is_none()
            && index.is_none()
            && token.parse::<usize>().is_ok()
            && prompt.is_some()
        {
            index = token.parse::<usize>().ok();
            continue;
        }

        if current_kind.is_none() && index.is_none() && token.parse::<usize>().is_ok() {
            index = token.parse::<usize>().ok();
            continue;
        }

        if !current_value.is_empty() {
            current_value.push(' ');
        }
        current_value.push_str(token);
    }

    push_workflow_step_segment(
        &mut name,
        &mut prompt,
        &mut when,
        &mut model,
        &mut backend,
        &mut step_cwd,
        current_kind,
        &mut current_value,
    );

    WorkflowCommand::AddStep {
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
    }
}

pub(super) fn parse_workflow_update_step_command(remainder: &str) -> WorkflowCommand {
    let tokens = remainder.split_whitespace().collect::<Vec<_>>();
    let mut workflow_name = None;
    let mut script_path = None;
    let mut token_index = 0;
    if let Some(first) = tokens.first() {
        if *first == "--script-path" {
            script_path = tokens.get(1).map(|value| (*value).to_string());
            token_index = usize::min(2, tokens.len());
        } else if !first.starts_with("--") {
            workflow_name = Some((*first).to_string());
            token_index = 1;
        }
    }
    let step_number = tokens
        .get(token_index)
        .and_then(|value| value.parse::<usize>().ok());
    if step_number.is_some() {
        token_index += 1;
    }
    let mut name = None;
    let mut clear_name = false;
    let mut prompt = None;
    let mut when = None;
    let mut clear_when = false;
    let mut model = None;
    let mut clear_model = false;
    let mut backend = None;
    let mut clear_backend = false;
    let mut step_cwd = None;
    let mut clear_step_cwd = false;
    let mut run_in_background = None;
    let mut current_kind = None;
    let mut current_value = String::new();

    while token_index < tokens.len() {
        let token = tokens[token_index];
        token_index += 1;
        let next_kind = match token {
            "--script-path" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                script_path = tokens.get(token_index).map(|value| (*value).to_string());
                token_index = usize::min(token_index + 1, tokens.len());
                current_kind = None;
                continue;
            }
            "--name" => Some(WorkflowStepPatchKind::Name),
            "--prompt" => Some(WorkflowStepPatchKind::Prompt),
            "--when" => Some(WorkflowStepPatchKind::When),
            "--model" => Some(WorkflowStepPatchKind::Model),
            "--backend" => Some(WorkflowStepPatchKind::Backend),
            "--step-cwd" => Some(WorkflowStepPatchKind::StepCwd),
            "--clear-name" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_name = true;
                None
            }
            "--clear-when" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_when = true;
                None
            }
            "--clear-model" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_model = true;
                None
            }
            "--clear-backend" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_backend = true;
                None
            }
            "--clear-step-cwd" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                clear_step_cwd = true;
                None
            }
            "--background" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                run_in_background = Some(true);
                None
            }
            "--foreground" => {
                push_workflow_step_patch_segment(
                    &mut name,
                    &mut prompt,
                    &mut when,
                    &mut model,
                    &mut backend,
                    &mut step_cwd,
                    current_kind.take(),
                    &mut current_value,
                );
                run_in_background = Some(false);
                None
            }
            _ => None,
        };

        if let Some(next_kind) = next_kind {
            push_workflow_step_patch_segment(
                &mut name,
                &mut prompt,
                &mut when,
                &mut model,
                &mut backend,
                &mut step_cwd,
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

    push_workflow_step_patch_segment(
        &mut name,
        &mut prompt,
        &mut when,
        &mut model,
        &mut backend,
        &mut step_cwd,
        current_kind,
        &mut current_value,
    );

    WorkflowCommand::UpdateStep {
        workflow_name,
        script_path,
        step_number,
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
    }
}

pub(super) fn parse_workflow_duplicate_step_command(remainder: &str) -> WorkflowCommand {
    let tokens = remainder.split_whitespace().collect::<Vec<_>>();
    let mut workflow_name = None;
    let mut script_path = None;
    let mut token_index = 0;
    if let Some(first) = tokens.first() {
        if *first == "--script-path" {
            script_path = tokens.get(1).map(|value| (*value).to_string());
            token_index = usize::min(2, tokens.len());
        } else if !first.starts_with("--") {
            workflow_name = Some((*first).to_string());
            token_index = 1;
        }
    }
    let step_number = tokens
        .get(token_index)
        .and_then(|value| value.parse::<usize>().ok());
    if step_number.is_some() {
        token_index += 1;
    }
    let mut to_step_number = None;
    let mut name = None;
    let mut current_kind = None;
    let mut current_value = String::new();

    while token_index < tokens.len() {
        let token = tokens[token_index];
        token_index += 1;
        match token {
            "--script-path" => {
                push_duplicate_segment(&mut name, current_kind.take(), &mut current_value);
                script_path = tokens.get(token_index).map(|value| (*value).to_string());
                token_index = usize::min(token_index + 1, tokens.len());
            }
            "--to" | "--index" => {
                push_duplicate_segment(&mut name, current_kind.take(), &mut current_value);
                to_step_number = tokens
                    .get(token_index)
                    .and_then(|value| value.parse::<usize>().ok());
                token_index = usize::min(token_index + 1, tokens.len());
            }
            "--name" => {
                push_duplicate_segment(&mut name, current_kind.take(), &mut current_value);
                current_kind = Some(WorkflowDuplicateSegmentKind::Name);
            }
            _ => {
                if !current_value.is_empty() {
                    current_value.push(' ');
                }
                current_value.push_str(token);
            }
        }
    }

    push_duplicate_segment(&mut name, current_kind, &mut current_value);

    WorkflowCommand::DuplicateStep {
        workflow_name,
        script_path,
        step_number,
        to_step_number,
        name,
    }
}

pub(super) fn parse_workflow_move_step_command(remainder: &str) -> WorkflowCommand {
    let tokens = remainder.split_whitespace().collect::<Vec<_>>();
    let mut workflow_name = None;
    let mut script_path = None;
    let mut token_index = 0;
    if let Some(first) = tokens.first() {
        if *first == "--script-path" {
            script_path = tokens.get(1).map(|value| (*value).to_string());
            token_index = usize::min(2, tokens.len());
        } else if !first.starts_with("--") {
            workflow_name = Some((*first).to_string());
            token_index = 1;
        }
    }
    let step_number = tokens
        .get(token_index)
        .and_then(|value| value.parse::<usize>().ok());
    if step_number.is_some() {
        token_index += 1;
    }
    let mut to_step_number = None;

    while token_index < tokens.len() {
        let token = tokens[token_index];
        token_index += 1;
        match token {
            "--script-path" => {
                script_path = tokens.get(token_index).map(|value| (*value).to_string());
                token_index = usize::min(token_index + 1, tokens.len());
            }
            "--to" => {
                to_step_number = tokens
                    .get(token_index)
                    .and_then(|value| value.parse::<usize>().ok());
                token_index = usize::min(token_index + 1, tokens.len());
            }
            _ if to_step_number.is_none() => {
                to_step_number = token.parse::<usize>().ok();
            }
            _ => {}
        }
    }

    WorkflowCommand::MoveStep {
        workflow_name,
        script_path,
        step_number,
        to_step_number,
    }
}

fn push_workflow_step_segment(
    name: &mut Option<String>,
    prompt: &mut Option<String>,
    when: &mut Option<String>,
    model: &mut Option<String>,
    backend: &mut Option<String>,
    step_cwd: &mut Option<String>,
    kind: Option<WorkflowStepSegmentKind>,
    current_value: &mut String,
) {
    let value = current_value.trim().to_string();
    current_value.clear();
    if value.is_empty() {
        return;
    }

    match kind {
        Some(WorkflowStepSegmentKind::Name) => *name = Some(value),
        Some(WorkflowStepSegmentKind::Prompt) => *prompt = Some(value),
        Some(WorkflowStepSegmentKind::When) => *when = Some(value),
        Some(WorkflowStepSegmentKind::Model) => *model = Some(value),
        Some(WorkflowStepSegmentKind::Backend) => *backend = Some(value),
        Some(WorkflowStepSegmentKind::StepCwd) => *step_cwd = Some(value),
        None => {
            if prompt.is_none() {
                *prompt = Some(value);
            }
        }
    }
}

fn push_workflow_step_patch_segment(
    name: &mut Option<String>,
    prompt: &mut Option<String>,
    when: &mut Option<String>,
    model: &mut Option<String>,
    backend: &mut Option<String>,
    step_cwd: &mut Option<String>,
    kind: Option<WorkflowStepPatchKind>,
    current_value: &mut String,
) {
    let value = current_value.trim().to_string();
    current_value.clear();
    if value.is_empty() {
        return;
    }

    match kind {
        Some(WorkflowStepPatchKind::Name) => *name = Some(value),
        Some(WorkflowStepPatchKind::Prompt) => *prompt = Some(value),
        Some(WorkflowStepPatchKind::When) => *when = Some(value),
        Some(WorkflowStepPatchKind::Model) => *model = Some(value),
        Some(WorkflowStepPatchKind::Backend) => *backend = Some(value),
        Some(WorkflowStepPatchKind::StepCwd) => *step_cwd = Some(value),
        None => {}
    }
}

fn push_duplicate_segment(
    name: &mut Option<String>,
    kind: Option<WorkflowDuplicateSegmentKind>,
    current_value: &mut String,
) {
    let value = current_value.trim().to_string();
    current_value.clear();
    if value.is_empty() {
        return;
    }

    if let Some(WorkflowDuplicateSegmentKind::Name) = kind {
        *name = Some(value);
    }
}
