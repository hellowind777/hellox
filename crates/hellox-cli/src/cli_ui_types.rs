use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum BriefCommands {
    Show {
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Set {
        message: String,
        #[arg(long = "attachment")]
        attachments: Vec<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Clear {
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ToolsCommands {
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}
