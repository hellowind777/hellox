use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum WorkflowCommands {
    List {
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    #[command(alias = "tui")]
    Dashboard {
        workflow_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    #[command(alias = "selector")]
    Overview {
        workflow_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    #[command(alias = "edit", alias = "board")]
    Panel {
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        step: Option<usize>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Runs {
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Validate {
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    ShowRun {
        run_id: String,
        #[arg(long)]
        step: Option<usize>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    LastRun {
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        step: Option<usize>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Show {
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Init {
        workflow_name: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        shared_context: Option<String>,
        #[arg(long)]
        continue_on_error: bool,
        #[arg(long)]
        force: bool,
    },
    AddStep {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        index: Option<usize>,
        #[arg(long)]
        when: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        backend: Option<String>,
        #[arg(long = "step-cwd")]
        step_cwd: Option<String>,
        #[arg(long)]
        run_in_background: bool,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    UpdateStep {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        step_number: usize,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        clear_name: bool,
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long)]
        when: Option<String>,
        #[arg(long)]
        clear_when: bool,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        clear_model: bool,
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        clear_backend: bool,
        #[arg(long = "step-cwd")]
        step_cwd: Option<String>,
        #[arg(long)]
        clear_step_cwd: bool,
        #[arg(long)]
        run_in_background: bool,
        #[arg(long)]
        foreground: bool,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    DuplicateStep {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        step_number: usize,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long = "to", alias = "index")]
        to_step_number: Option<usize>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    MoveStep {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        step_number: usize,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long = "to")]
        to_step_number: usize,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    RemoveStep {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        step_number: usize,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    SetSharedContext {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        value: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    ClearSharedContext {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    EnableContinueOnError {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    DisableContinueOnError {
        #[arg(long = "workflow")]
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Run {
        workflow_name: Option<String>,
        #[arg(long)]
        script_path: Option<PathBuf>,
        #[arg(long)]
        shared_context: Option<String>,
        #[arg(long)]
        continue_on_error: bool,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}
