use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum OutputStyleCommands {
    Panel {
        style_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    List {
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Show {
        style_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    SetDefault {
        style_name: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    ClearDefault {
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PersonaCommands {
    Panel {
        persona_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    List {
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Show {
        persona_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    SetDefault {
        persona_name: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    ClearDefault {
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PromptFragmentCommands {
    Panel {
        fragment_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    List {
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Show {
        fragment_name: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    SetDefault {
        fragment_names: Vec<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    ClearDefault {
        #[arg(long)]
        config: Option<PathBuf>,
    },
}
