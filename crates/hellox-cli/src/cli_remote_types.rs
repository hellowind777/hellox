use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum RemoteEnvCommands {
    List,
    Show {
        environment_name: String,
    },
    Add {
        environment_name: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        token_env: Option<String>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        device_id: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    Enable {
        environment_name: String,
    },
    Disable {
        environment_name: String,
    },
    Remove {
        environment_name: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeleportCommands {
    Plan {
        environment_name: String,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Connect {
        environment_name: String,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AssistantCommands {
    List {
        #[arg(long = "environment")]
        environment_name: Option<String>,
    },
    Show {
        session_id: String,
        #[arg(long = "environment")]
        environment_name: Option<String>,
    },
}
