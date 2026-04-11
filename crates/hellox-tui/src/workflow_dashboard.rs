#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowDashboardView {
    OverviewList,
    OverviewFocus {
        workflow_name: String,
    },
    PanelList,
    PanelFocus {
        workflow_name: String,
        step_number: Option<usize>,
    },
    Runs {
        workflow_name: Option<String>,
    },
    RunInspect {
        run_id: String,
        step_number: Option<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowDashboardOpenTarget {
    OverviewWorkflow(String),
    OverviewRun(String),
    PanelWorkflow(String),
    PanelStep {
        workflow_name: String,
        step_number: usize,
    },
    Run(String),
    RunStep {
        run_id: String,
        step_number: usize,
    },
}

impl WorkflowDashboardOpenTarget {
    pub fn into_view(self) -> WorkflowDashboardView {
        match self {
            Self::OverviewWorkflow(workflow_name) => {
                WorkflowDashboardView::OverviewFocus { workflow_name }
            }
            Self::OverviewRun(run_id) | Self::Run(run_id) => WorkflowDashboardView::RunInspect {
                run_id,
                step_number: None,
            },
            Self::PanelWorkflow(workflow_name) => WorkflowDashboardView::PanelFocus {
                workflow_name,
                step_number: None,
            },
            Self::PanelStep {
                workflow_name,
                step_number,
            } => WorkflowDashboardView::PanelFocus {
                workflow_name,
                step_number: Some(step_number),
            },
            Self::RunStep {
                run_id,
                step_number,
            } => WorkflowDashboardView::RunInspect {
                run_id,
                step_number: Some(step_number),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowDashboardCommand {
    Overview {
        workflow_name: Option<String>,
    },
    Show {
        workflow_name: Option<String>,
    },
    Run {
        shared_context: Option<String>,
    },
    Panel {
        workflow_name: Option<String>,
        step_number: Option<usize>,
    },
    Runs {
        workflow_name: Option<String>,
    },
    Validate {
        workflow_name: Option<String>,
    },
    Init {
        workflow_name: Option<String>,
    },
    AddStep {
        name: Option<String>,
        prompt: Option<String>,
        index: Option<usize>,
        when: Option<String>,
        model: Option<String>,
        backend: Option<String>,
        step_cwd: Option<String>,
        run_in_background: bool,
    },
    UpdateStep {
        step_number: Option<usize>,
        name: Option<String>,
        clear_name: bool,
        prompt: Option<String>,
        when: Option<String>,
        clear_when: bool,
        model: Option<String>,
        clear_model: bool,
        backend: Option<String>,
        clear_backend: bool,
        step_cwd: Option<String>,
        clear_step_cwd: bool,
        run_in_background: Option<bool>,
    },
    ShowRun {
        run_id: Option<String>,
        step_number: Option<usize>,
    },
    LastRun {
        workflow_name: Option<String>,
        step_number: Option<usize>,
    },
    SetSharedContext {
        value: Option<String>,
    },
    ClearSharedContext,
    EnableContinueOnError,
    DisableContinueOnError,
    Open {
        index: usize,
    },
    Duplicate {
        to_step_number: Option<usize>,
    },
    DuplicateStep {
        step_number: Option<usize>,
        to_step_number: Option<usize>,
        name: Option<String>,
    },
    Move {
        to_step_number: Option<usize>,
    },
    MoveStep {
        step_number: Option<usize>,
        to_step_number: Option<usize>,
    },
    Remove,
    RemoveStep {
        step_number: Option<usize>,
    },
    Back,
    Help,
    Close,
    Quit,
    Error(String),
    Unknown,
}

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
enum WorkflowStepDuplicateKind {
    Name,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDashboardState {
    current: WorkflowDashboardView,
    history: Vec<WorkflowDashboardView>,
    open_targets: Vec<WorkflowDashboardOpenTarget>,
}

impl WorkflowDashboardState {
    pub fn new(initial: WorkflowDashboardView) -> Self {
        Self {
            current: initial,
            history: Vec::new(),
            open_targets: Vec::new(),
        }
    }

    pub fn current(&self) -> &WorkflowDashboardView {
        &self.current
    }

    pub fn set_open_targets(&mut self, open_targets: Vec<WorkflowDashboardOpenTarget>) {
        self.open_targets = open_targets;
    }

    pub fn navigate_to(&mut self, next: WorkflowDashboardView) {
        if self.current != next {
            self.history.push(self.current.clone());
            self.current = next;
        }
        self.open_targets.clear();
    }

    pub fn replace_current(&mut self, next: WorkflowDashboardView) {
        self.current = next;
        self.open_targets.clear();
    }

    pub fn back(&mut self) -> bool {
        match self.history.pop() {
            Some(previous) => {
                self.current = previous;
                self.open_targets.clear();
                true
            }
            None => false,
        }
    }

    pub fn open(&mut self, index: usize) -> Result<(), String> {
        if index == 0 || index > self.open_targets.len() {
            return Err(format!(
                "Invalid selection. Choose 1..{} or use `help` for dashboard commands.",
                self.open_targets.len()
            ));
        }

        let next = self.open_targets[index - 1].clone().into_view();
        self.navigate_to(next);
        Ok(())
    }
}

pub fn parse_workflow_dashboard_command(input: &str) -> Option<WorkflowDashboardCommand> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') {
        return None;
    }

    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return trimmed
            .parse::<usize>()
            .ok()
            .map(|index| WorkflowDashboardCommand::Open { index });
    }

    let mut parts = trimmed.split_whitespace();
    let command = parts.next()?.to_ascii_lowercase();
    let tail = parts.collect::<Vec<_>>();

    Some(match command.as_str() {
        "overview" | "ov" => WorkflowDashboardCommand::Overview {
            workflow_name: joined_value(tail),
        },
        "show" => WorkflowDashboardCommand::Show {
            workflow_name: joined_value(tail),
        },
        "run" => WorkflowDashboardCommand::Run {
            shared_context: joined_value(tail),
        },
        "panel" | "edit" | "board" => parse_panel_command(tail),
        "runs" | "history" => WorkflowDashboardCommand::Runs {
            workflow_name: joined_value(tail),
        },
        "validate" => WorkflowDashboardCommand::Validate {
            workflow_name: joined_value(tail),
        },
        "init" => WorkflowDashboardCommand::Init {
            workflow_name: joined_value(tail),
        },
        "add-step" => parse_add_step_command(tail),
        "update-step" => parse_update_step_command(tail),
        "show-run" => parse_show_run_command(tail),
        "last-run" => parse_last_run_command(tail),
        "set-shared-context" => WorkflowDashboardCommand::SetSharedContext {
            value: joined_value(tail),
        },
        "clear-shared-context" => WorkflowDashboardCommand::ClearSharedContext,
        "enable-continue-on-error" | "enable-coe" => {
            WorkflowDashboardCommand::EnableContinueOnError
        }
        "disable-continue-on-error" | "disable-coe" => {
            WorkflowDashboardCommand::DisableContinueOnError
        }
        "open" => match parse_required_index(&tail, "Usage: open <n>") {
            Ok(index) => WorkflowDashboardCommand::Open { index },
            Err(error) => WorkflowDashboardCommand::Error(error),
        },
        "dup" | "duplicate" => match parse_optional_index(&tail) {
            Ok(to_step_number) => WorkflowDashboardCommand::Duplicate { to_step_number },
            Err(error) => WorkflowDashboardCommand::Error(error),
        },
        "duplicate-step" => parse_duplicate_step_command(tail),
        "move" => match parse_required_index(&tail, "Usage: move <to-step-number>") {
            Ok(to_step_number) => WorkflowDashboardCommand::Move {
                to_step_number: Some(to_step_number),
            },
            Err(error) => WorkflowDashboardCommand::Error(error),
        },
        "move-step" => parse_move_step_command(tail),
        "rm" | "remove" | "delete" => {
            if tail.is_empty() {
                WorkflowDashboardCommand::Remove
            } else {
                WorkflowDashboardCommand::Error("Usage: rm".to_string())
            }
        }
        "remove-step" => parse_remove_step_command(tail),
        "back" => WorkflowDashboardCommand::Back,
        "help" | "?" => WorkflowDashboardCommand::Help,
        "close" => WorkflowDashboardCommand::Close,
        "quit" | "exit" => WorkflowDashboardCommand::Quit,
        _ => WorkflowDashboardCommand::Unknown,
    })
}

pub fn workflow_dashboard_help_text() -> String {
    [
        "Workflow dashboard commands:",
        "  overview [name]      Show the global workflow overview or focus one workflow",
        "  show [name]          Show the raw workflow definition for the current or named workflow",
        "  run [shared_context] Run the active workflow and open the recorded run",
        "  panel [name] [n]     Open the authoring panel or focus one workflow step",
        "  runs [name]          Show recorded run history",
        "  validate [name]      Validate the current or named workflow, or all workflows",
        "  init <name>          Scaffold a new workflow and jump into it",
        "  add-step --prompt <text> [--name <step-name>] [--index <n>] [--when <json>] [--model <name>] [--backend <name>] [--step-cwd <path>] [--background]",
        "  update-step [n] [--name <step-name>|--clear-name] [--prompt <text>] [--when <json>|--clear-when] [--model <name>|--clear-model] [--backend <name>|--clear-backend] [--step-cwd <path>|--clear-step-cwd] [--background|--foreground]",
        "  show-run <id> [n]    Inspect a recorded run and optionally focus one step",
        "  last-run [name] [n]  Jump to the latest recorded run and optionally focus one step",
        "  set-shared-context <text> Set shared context for the active workflow",
        "  clear-shared-context Clear shared context for the active workflow",
        "  enable-continue-on-error  Enable continue_on_error for the active workflow",
        "  disable-continue-on-error Disable continue_on_error for the active workflow",
        "  duplicate-step [n] [--to <m>] [--name <step-name>]",
        "                       Duplicate one workflow step from the active workflow context",
        "  move-step [n] --to <m>",
        "                       Move one workflow step from the active workflow context",
        "  remove-step [n]      Remove one workflow step from the active workflow context",
        "  open <n>             Open one selector entry from the current dashboard screen",
        "  1..n                 Shortcut for `open <n>`",
        "  name <text>          Rename the focused workflow step (panel view only)",
        "  prompt <text>        Replace the focused workflow step prompt (panel view only)",
        "  when <json>          Set the focused workflow step condition (panel view only)",
        "  model <name>         Set the focused workflow step model (panel view only)",
        "  backend <name>       Set the focused workflow step backend (panel view only)",
        "  step-cwd <path>      Set the focused workflow step cwd (panel view only)",
        "  clear-name|clear-when|clear-model|clear-backend|clear-step-cwd",
        "                       Clear one focused workflow step field (panel view only)",
        "  background|foreground Switch the focused workflow step mode (panel view only)",
        "  dup [to]             Duplicate the focused workflow step (panel view only)",
        "  move <to>            Move the focused workflow step (panel view only)",
        "  rm                   Remove the focused workflow step (panel view only)",
        "  next|prev|first|last Focus adjacent/edge step in panel or run inspect views",
        "  back                 Return to the previous dashboard screen",
        "  help                 Show this dashboard help",
        "  close / quit         Leave the workflow dashboard",
    ]
    .join("\n")
}

fn parse_panel_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    match parts.as_slice() {
        [] => WorkflowDashboardCommand::Panel {
            workflow_name: None,
            step_number: None,
        },
        [step] => match step.parse::<usize>() {
            Ok(step_number) if step_number > 0 => WorkflowDashboardCommand::Panel {
                workflow_name: None,
                step_number: Some(step_number),
            },
            _ => WorkflowDashboardCommand::Panel {
                workflow_name: Some((*step).to_string()),
                step_number: None,
            },
        },
        [workflow_name, step] => match step.parse::<usize>() {
            Ok(step_number) if step_number > 0 => WorkflowDashboardCommand::Panel {
                workflow_name: Some((*workflow_name).to_string()),
                step_number: Some(step_number),
            },
            _ => WorkflowDashboardCommand::Error(
                "Usage: panel [workflow-name] [step-number]".to_string(),
            ),
        },
        _ => WorkflowDashboardCommand::Error(
            "Usage: panel [workflow-name] [step-number]".to_string(),
        ),
    }
}

fn parse_show_run_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    match parts.as_slice() {
        [] => WorkflowDashboardCommand::ShowRun {
            run_id: None,
            step_number: None,
        },
        [run_id] => WorkflowDashboardCommand::ShowRun {
            run_id: Some((*run_id).to_string()),
            step_number: None,
        },
        [run_id, step] => match step.parse::<usize>() {
            Ok(step_number) if step_number > 0 => WorkflowDashboardCommand::ShowRun {
                run_id: Some((*run_id).to_string()),
                step_number: Some(step_number),
            },
            _ => WorkflowDashboardCommand::Error(
                "Usage: show-run <run-id> [step-number]".to_string(),
            ),
        },
        _ => WorkflowDashboardCommand::Error("Usage: show-run <run-id> [step-number]".to_string()),
    }
}

fn parse_last_run_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    match parts.as_slice() {
        [] => WorkflowDashboardCommand::LastRun {
            workflow_name: None,
            step_number: None,
        },
        [value] => match value.parse::<usize>().ok().filter(|step| *step > 0) {
            Some(step_number) => WorkflowDashboardCommand::LastRun {
                workflow_name: None,
                step_number: Some(step_number),
            },
            None => WorkflowDashboardCommand::LastRun {
                workflow_name: Some((*value).to_string()),
                step_number: None,
            },
        },
        [workflow_name, step] => match step.parse::<usize>().ok().filter(|step| *step > 0) {
            Some(step_number) => WorkflowDashboardCommand::LastRun {
                workflow_name: Some((*workflow_name).to_string()),
                step_number: Some(step_number),
            },
            None => WorkflowDashboardCommand::Error(
                "Usage: last-run [workflow-name] [step-number]".to_string(),
            ),
        },
        _ => WorkflowDashboardCommand::Error(
            "Usage: last-run [workflow-name] [step-number]".to_string(),
        ),
    }
}

fn parse_add_step_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    let usage = "Usage: add-step --prompt <text> [--name <step-name>] [--index <n>] [--when <json>] [--model <name>] [--backend <name>] [--step-cwd <path>] [--background]";
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
    let mut expecting_index = false;

    for token in parts {
        let next_kind = match token {
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
                expecting_index = true;
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
            expecting_index = false;
            continue;
        }

        if expecting_index {
            match token.parse::<usize>().ok().filter(|index| *index > 0) {
                Some(value) => {
                    index = Some(value);
                    expecting_index = false;
                }
                None => return WorkflowDashboardCommand::Error(usage.to_string()),
            }
            continue;
        }

        if current_kind.is_none() {
            return WorkflowDashboardCommand::Error(usage.to_string());
        }

        if !current_value.is_empty() {
            current_value.push(' ');
        }
        current_value.push_str(token);
    }

    if expecting_index {
        return WorkflowDashboardCommand::Error(usage.to_string());
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

    WorkflowDashboardCommand::AddStep {
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

fn parse_update_step_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    let usage = "Usage: update-step [step-number] [--name <step-name>|--clear-name] [--prompt <text>] [--when <json>|--clear-when] [--model <name>|--clear-model] [--backend <name>|--clear-backend] [--step-cwd <path>|--clear-step-cwd] [--background|--foreground]";
    let mut step_number = None;
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

    for token in parts {
        let next_kind = match token {
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
                continue;
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
                continue;
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
                continue;
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
                continue;
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
                continue;
            }
            "--background" => {
                if run_in_background == Some(false) {
                    return WorkflowDashboardCommand::Error(
                        "choose either `--background` or `--foreground`, but not both".to_string(),
                    );
                }
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
                continue;
            }
            "--foreground" => {
                if run_in_background == Some(true) {
                    return WorkflowDashboardCommand::Error(
                        "choose either `--background` or `--foreground`, but not both".to_string(),
                    );
                }
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
                continue;
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

        if current_kind.is_none() && step_number.is_none() {
            match token.parse::<usize>().ok().filter(|index| *index > 0) {
                Some(value) => {
                    step_number = Some(value);
                    continue;
                }
                None => return WorkflowDashboardCommand::Error(usage.to_string()),
            }
        }

        if current_kind.is_none() {
            return WorkflowDashboardCommand::Error(usage.to_string());
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

    WorkflowDashboardCommand::UpdateStep {
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

fn parse_duplicate_step_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    let usage = "Usage: duplicate-step [step-number] [--to <n>] [--name <step-name>]";
    let mut step_number = None;
    let mut to_step_number = None;
    let mut name = None;
    let mut current_kind = None;
    let mut current_value = String::new();
    let mut expecting_to = false;

    for token in parts {
        match token {
            "--to" => {
                push_duplicate_step_segment(&mut name, current_kind.take(), &mut current_value);
                expecting_to = true;
                continue;
            }
            "--name" => {
                push_duplicate_step_segment(&mut name, current_kind.take(), &mut current_value);
                current_kind = Some(WorkflowStepDuplicateKind::Name);
                expecting_to = false;
                continue;
            }
            _ => {}
        }

        if expecting_to {
            match token.parse::<usize>().ok().filter(|index| *index > 0) {
                Some(value) => {
                    to_step_number = Some(value);
                    expecting_to = false;
                }
                None => return WorkflowDashboardCommand::Error(usage.to_string()),
            }
            continue;
        }

        if current_kind.is_none() && step_number.is_none() {
            match token.parse::<usize>().ok().filter(|index| *index > 0) {
                Some(value) => {
                    step_number = Some(value);
                    continue;
                }
                None => return WorkflowDashboardCommand::Error(usage.to_string()),
            }
        }

        if current_kind.is_none() {
            return WorkflowDashboardCommand::Error(usage.to_string());
        }

        if !current_value.is_empty() {
            current_value.push(' ');
        }
        current_value.push_str(token);
    }

    if expecting_to {
        return WorkflowDashboardCommand::Error(usage.to_string());
    }

    push_duplicate_step_segment(&mut name, current_kind, &mut current_value);

    WorkflowDashboardCommand::DuplicateStep {
        step_number,
        to_step_number,
        name,
    }
}

fn parse_move_step_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    let usage = "Usage: move-step [step-number] --to <n>";
    let mut step_number = None;
    let mut to_step_number = None;
    let mut expecting_to = false;

    for token in parts {
        if token == "--to" {
            expecting_to = true;
            continue;
        }

        if expecting_to {
            match token.parse::<usize>().ok().filter(|index| *index > 0) {
                Some(value) => {
                    to_step_number = Some(value);
                    expecting_to = false;
                }
                None => return WorkflowDashboardCommand::Error(usage.to_string()),
            }
            continue;
        }

        if step_number.is_none() {
            match token.parse::<usize>().ok().filter(|index| *index > 0) {
                Some(value) => {
                    step_number = Some(value);
                    continue;
                }
                None => return WorkflowDashboardCommand::Error(usage.to_string()),
            }
        }

        return WorkflowDashboardCommand::Error(usage.to_string());
    }

    if expecting_to || to_step_number.is_none() {
        return WorkflowDashboardCommand::Error(usage.to_string());
    }

    WorkflowDashboardCommand::MoveStep {
        step_number,
        to_step_number,
    }
}

fn parse_remove_step_command(parts: Vec<&str>) -> WorkflowDashboardCommand {
    match parts.as_slice() {
        [] => WorkflowDashboardCommand::RemoveStep { step_number: None },
        [value] => match value.parse::<usize>().ok().filter(|index| *index > 0) {
            Some(step_number) => WorkflowDashboardCommand::RemoveStep {
                step_number: Some(step_number),
            },
            None => WorkflowDashboardCommand::Error("Usage: remove-step [step-number]".to_string()),
        },
        _ => WorkflowDashboardCommand::Error("Usage: remove-step [step-number]".to_string()),
    }
}

fn parse_required_index(parts: &[&str], usage: &str) -> Result<usize, String> {
    match parts {
        [value] => value
            .parse::<usize>()
            .ok()
            .filter(|index| *index > 0)
            .ok_or_else(|| usage.to_string()),
        _ => Err(usage.to_string()),
    }
}

fn parse_optional_index(parts: &[&str]) -> Result<Option<usize>, String> {
    match parts {
        [] => Ok(None),
        [value] => value
            .parse::<usize>()
            .ok()
            .filter(|index| *index > 0)
            .map(Some)
            .ok_or_else(|| "Usage: dup [to-step-number]".to_string()),
        _ => Err("Usage: dup [to-step-number]".to_string()),
    }
}

fn joined_value(parts: Vec<&str>) -> Option<String> {
    let value = parts.join(" ");
    let value = value.trim();
    (!value.is_empty()).then_some(value.to_string())
}

fn push_workflow_step_segment(
    name: &mut Option<String>,
    prompt: &mut Option<String>,
    when: &mut Option<String>,
    model: &mut Option<String>,
    backend: &mut Option<String>,
    step_cwd: &mut Option<String>,
    current_kind: Option<WorkflowStepSegmentKind>,
    current_value: &mut String,
) {
    let Some(current_kind) = current_kind else {
        current_value.clear();
        return;
    };

    let value = take_segment_value(current_value);
    match current_kind {
        WorkflowStepSegmentKind::Name => *name = value,
        WorkflowStepSegmentKind::Prompt => *prompt = value,
        WorkflowStepSegmentKind::When => *when = value,
        WorkflowStepSegmentKind::Model => *model = value,
        WorkflowStepSegmentKind::Backend => *backend = value,
        WorkflowStepSegmentKind::StepCwd => *step_cwd = value,
    }
}

fn push_workflow_step_patch_segment(
    name: &mut Option<String>,
    prompt: &mut Option<String>,
    when: &mut Option<String>,
    model: &mut Option<String>,
    backend: &mut Option<String>,
    step_cwd: &mut Option<String>,
    current_kind: Option<WorkflowStepPatchKind>,
    current_value: &mut String,
) {
    let Some(current_kind) = current_kind else {
        current_value.clear();
        return;
    };

    let value = take_segment_value(current_value);
    match current_kind {
        WorkflowStepPatchKind::Name => *name = value,
        WorkflowStepPatchKind::Prompt => *prompt = value,
        WorkflowStepPatchKind::When => *when = value,
        WorkflowStepPatchKind::Model => *model = value,
        WorkflowStepPatchKind::Backend => *backend = value,
        WorkflowStepPatchKind::StepCwd => *step_cwd = value,
    }
}

fn take_segment_value(current_value: &mut String) -> Option<String> {
    let value = current_value.trim().to_string();
    current_value.clear();
    (!value.is_empty()).then_some(value)
}

fn push_duplicate_step_segment(
    name: &mut Option<String>,
    current_kind: Option<WorkflowStepDuplicateKind>,
    current_value: &mut String,
) {
    let Some(current_kind) = current_kind else {
        current_value.clear();
        return;
    };

    let value = take_segment_value(current_value);
    match current_kind {
        WorkflowStepDuplicateKind::Name => *name = value,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_workflow_dashboard_command, workflow_dashboard_help_text, WorkflowDashboardCommand,
        WorkflowDashboardOpenTarget, WorkflowDashboardState, WorkflowDashboardView,
    };

    #[test]
    fn parses_dashboard_navigation_commands() {
        assert_eq!(
            parse_workflow_dashboard_command("overview release-review"),
            Some(WorkflowDashboardCommand::Overview {
                workflow_name: Some(String::from("release-review")),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("show release-review"),
            Some(WorkflowDashboardCommand::Show {
                workflow_name: Some(String::from("release-review")),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("run ship carefully"),
            Some(WorkflowDashboardCommand::Run {
                shared_context: Some(String::from("ship carefully")),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("panel release-review 2"),
            Some(WorkflowDashboardCommand::Panel {
                workflow_name: Some(String::from("release-review")),
                step_number: Some(2),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("validate"),
            Some(WorkflowDashboardCommand::Validate {
                workflow_name: None,
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("init release-review"),
            Some(WorkflowDashboardCommand::Init {
                workflow_name: Some(String::from("release-review")),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("show-run run-123 2"),
            Some(WorkflowDashboardCommand::ShowRun {
                run_id: Some(String::from("run-123")),
                step_number: Some(2),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("last-run release-review 2"),
            Some(WorkflowDashboardCommand::LastRun {
                workflow_name: Some(String::from("release-review")),
                step_number: Some(2),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("7"),
            Some(WorkflowDashboardCommand::Open { index: 7 })
        );
    }

    #[test]
    fn parses_dashboard_authoring_commands() {
        assert_eq!(
            parse_workflow_dashboard_command("set-shared-context ship carefully"),
            Some(WorkflowDashboardCommand::SetSharedContext {
                value: Some(String::from("ship carefully")),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("clear-shared-context"),
            Some(WorkflowDashboardCommand::ClearSharedContext)
        );
        assert_eq!(
            parse_workflow_dashboard_command("enable-continue-on-error"),
            Some(WorkflowDashboardCommand::EnableContinueOnError)
        );
        assert_eq!(
            parse_workflow_dashboard_command("disable-continue-on-error"),
            Some(WorkflowDashboardCommand::DisableContinueOnError)
        );
        assert_eq!(
            parse_workflow_dashboard_command("dup 3"),
            Some(WorkflowDashboardCommand::Duplicate {
                to_step_number: Some(3),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("duplicate-step 2 --to 3 --name ship copy"),
            Some(WorkflowDashboardCommand::DuplicateStep {
                step_number: Some(2),
                to_step_number: Some(3),
                name: Some(String::from("ship copy")),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command(
                "add-step --prompt summarize findings --name summarize --index 2 --when {\"previous_status\":\"completed\"} --model mock-model --backend detached_process --step-cwd docs --background"
            ),
            Some(WorkflowDashboardCommand::AddStep {
                name: Some(String::from("summarize")),
                prompt: Some(String::from("summarize findings")),
                index: Some(2),
                when: Some(String::from("{\"previous_status\":\"completed\"}")),
                model: Some(String::from("mock-model")),
                backend: Some(String::from("detached_process")),
                step_cwd: Some(String::from("docs")),
                run_in_background: true,
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command(
                "update-step 2 --clear-name --prompt summarize findings --clear-when --clear-model --backend detached_process --clear-step-cwd --foreground"
            ),
            Some(WorkflowDashboardCommand::UpdateStep {
                step_number: Some(2),
                name: None,
                clear_name: true,
                prompt: Some(String::from("summarize findings")),
                when: None,
                clear_when: true,
                model: None,
                clear_model: true,
                backend: Some(String::from("detached_process")),
                clear_backend: false,
                step_cwd: None,
                clear_step_cwd: true,
                run_in_background: Some(false),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("move 1"),
            Some(WorkflowDashboardCommand::Move {
                to_step_number: Some(1),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("move-step 2 --to 1"),
            Some(WorkflowDashboardCommand::MoveStep {
                step_number: Some(2),
                to_step_number: Some(1),
            })
        );
        assert_eq!(
            parse_workflow_dashboard_command("rm"),
            Some(WorkflowDashboardCommand::Remove)
        );
        assert_eq!(
            parse_workflow_dashboard_command("remove-step 2"),
            Some(WorkflowDashboardCommand::RemoveStep {
                step_number: Some(2),
            })
        );
    }

    #[test]
    fn invalid_dashboard_command_shapes_return_usage_errors() {
        assert_eq!(
            parse_workflow_dashboard_command("open nope"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: open <n>"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("move"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: move <to-step-number>"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("add-step ship release"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: add-step --prompt <text> [--name <step-name>] [--index <n>] [--when <json>] [--model <name>] [--backend <name>] [--step-cwd <path>] [--background]"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("show-run run-1 nope"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: show-run <run-id> [step-number]"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("last-run release-review nope"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: last-run [workflow-name] [step-number]"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("duplicate-step nope"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: duplicate-step [step-number] [--to <n>] [--name <step-name>]"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("move-step 2"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: move-step [step-number] --to <n>"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("remove-step nope"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "Usage: remove-step [step-number]"
            )))
        );
        assert_eq!(
            parse_workflow_dashboard_command("update-step 2 --background --foreground"),
            Some(WorkflowDashboardCommand::Error(String::from(
                "choose either `--background` or `--foreground`, but not both"
            )))
        );
    }

    #[test]
    fn dashboard_state_tracks_history_and_open_targets() {
        let mut state = WorkflowDashboardState::new(WorkflowDashboardView::OverviewList);
        state.set_open_targets(vec![WorkflowDashboardOpenTarget::OverviewWorkflow(
            String::from("release-review"),
        )]);
        state.open(1).expect("open workflow");
        assert_eq!(
            state.current(),
            &WorkflowDashboardView::OverviewFocus {
                workflow_name: String::from("release-review"),
            }
        );
        assert!(state.back());
        assert_eq!(state.current(), &WorkflowDashboardView::OverviewList);
    }

    #[test]
    fn dashboard_help_mentions_open_and_authoring_commands() {
        let text = workflow_dashboard_help_text();
        assert!(text.contains("show [name]"));
        assert!(text.contains("run [shared_context]"));
        assert!(text.contains("validate [name]"));
        assert!(text.contains("add-step --prompt <text>"));
        assert!(text.contains("update-step [n]"));
        assert!(text.contains("duplicate-step [n] [--to <m>] [--name <step-name>]"));
        assert!(text.contains("move-step [n] --to <m>"));
        assert!(text.contains("remove-step [n]"));
        assert!(text.contains("set-shared-context <text>"));
        assert!(text.contains("open <n>"));
        assert!(text.contains("name <text>"));
        assert!(text.contains("background|foreground"));
        assert!(text.contains("dup [to]"));
        assert!(text.contains("move <to>"));
        assert!(text.contains("next|prev|first|last"));
        assert!(text.contains("close / quit"));
    }
}
