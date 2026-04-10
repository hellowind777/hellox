use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum ServerCommands {
    Serve {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Status {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    CreateSession {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Sessions {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    ShowSession {
        session_id: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    ManagedSettingsShow {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    ManagedSettingsSet {
        config_toml_file: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        signature: Option<String>,
    },
    PolicyLimitsShow {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    PolicyLimitsSet {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long = "disable-command")]
        disabled_commands: Vec<String>,
        #[arg(long = "disable-tool")]
        disabled_tools: Vec<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    SettingsShow {
        account_id: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    TeamMemoryShow {
        account_id: String,
        repo_id: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    TeamMemoryPanel {
        account_id: String,
        repo_id: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}
