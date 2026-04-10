use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum InstallCommands {
    Status,
    Plan {
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        target: Option<PathBuf>,
    },
    Apply {
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        target: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum UpgradeCommands {
    Status,
    Plan {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        target: Option<PathBuf>,
    },
    Apply {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        target: Option<PathBuf>,
        #[arg(long)]
        backup: bool,
        #[arg(long)]
        force: bool,
    },
}
