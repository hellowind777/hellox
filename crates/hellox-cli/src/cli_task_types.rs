use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommands {
    Panel {
        task_id: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Show {
        task_id: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Add {
        content: String,
        #[arg(long)]
        priority: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Update {
        task_id: String,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        priority: Option<String>,
        #[arg(long)]
        clear_priority: bool,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        clear_description: bool,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        output: Option<String>,
        #[arg(long)]
        clear_output: bool,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Output {
        task_id: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Stop {
        task_id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Start {
        task_id: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Done {
        task_id: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Cancel {
        task_id: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Remove {
        task_id: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Clear {
        #[arg(long)]
        completed: bool,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}
