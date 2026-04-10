#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowCommand {
    List,
    Overview {
        workflow_name: Option<String>,
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
    ShowRun {
        run_id: Option<String>,
    },
    LastRun {
        workflow_name: Option<String>,
    },
    Show {
        workflow_name: Option<String>,
    },
    Init {
        workflow_name: Option<String>,
    },
    AddStep {
        workflow_name: Option<String>,
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
        workflow_name: Option<String>,
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
    DuplicateStep {
        workflow_name: Option<String>,
        step_number: Option<usize>,
        to_step_number: Option<usize>,
        name: Option<String>,
    },
    MoveStep {
        workflow_name: Option<String>,
        step_number: Option<usize>,
        to_step_number: Option<usize>,
    },
    RemoveStep {
        workflow_name: Option<String>,
        step_number: Option<usize>,
    },
    SetSharedContext {
        workflow_name: Option<String>,
        value: Option<String>,
    },
    ClearSharedContext {
        workflow_name: Option<String>,
    },
    EnableContinueOnError {
        workflow_name: Option<String>,
    },
    DisableContinueOnError {
        workflow_name: Option<String>,
    },
    Run {
        workflow_name: Option<String>,
        shared_context: Option<String>,
    },
    Help,
}
