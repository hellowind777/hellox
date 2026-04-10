use std::path::PathBuf;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum SyncCommands {
    SettingsExport {
        output: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    SettingsImport {
        input: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    SettingsPush {
        environment_name: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    SettingsPull {
        environment_name: String,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    TeamMemoryShow {
        repo_id: String,
    },
    TeamMemoryPanel {
        repo_id: String,
    },
    TeamMemoryExport {
        repo_id: String,
        output: PathBuf,
    },
    TeamMemoryImport {
        repo_id: String,
        input: PathBuf,
    },
    TeamMemoryPut {
        repo_id: String,
        key: String,
        content: String,
    },
    TeamMemoryRemove {
        repo_id: String,
        key: String,
    },
    TeamMemorySync {
        environment_name: String,
        repo_id: String,
    },
    ManagedSettingsFetch {
        environment_name: String,
    },
    PolicyLimitsFetch {
        environment_name: String,
    },
}
