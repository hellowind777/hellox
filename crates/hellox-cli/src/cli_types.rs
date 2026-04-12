use std::path::PathBuf;

use crate::memory::MemoryScopeSelector;
use clap::{Parser, Subcommand};

pub(crate) use crate::cli_auth_types::AuthCommands;
pub(crate) use crate::cli_bridge_types::{BridgeCommands, IdeCommands};
pub(crate) use crate::cli_extension_types::{
    MarketplaceCommands, McpCommands, McpScopeValue, PluginCommands,
};
pub(crate) use crate::cli_install_types::{InstallCommands, UpgradeCommands};
pub(crate) use crate::cli_plan_types::PlanCommands;
pub(crate) use crate::cli_remote_types::{AssistantCommands, RemoteEnvCommands, TeleportCommands};
pub(crate) use crate::cli_server_types::ServerCommands;
pub(crate) use crate::cli_style_types::{
    OutputStyleCommands, PersonaCommands, PromptFragmentCommands,
};
pub(crate) use crate::cli_sync_types::SyncCommands;
pub(crate) use crate::cli_task_types::TaskCommands;
pub(crate) use crate::cli_ui_types::{BriefCommands, ToolsCommands};
pub(crate) use crate::cli_workflow_types::WorkflowCommands;
use hellox_config::PermissionMode;

pub(crate) const DEFAULT_MAX_TURNS: usize = 12;

#[derive(Debug, Parser)]
#[command(name = "hellox")]
#[command(
    about = "Rust-native CLI and Anthropic-compatible gateway for multi-provider coding models"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    Gateway {
        #[command(subcommand)]
        command: GatewayCommands,
    },
    Brief {
        #[command(subcommand)]
        command: BriefCommands,
    },
    Tools {
        #[command(subcommand)]
        command: ToolsCommands,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Plan {
        #[command(subcommand)]
        command: PlanCommands,
    },
    OutputStyle {
        #[command(subcommand)]
        command: OutputStyleCommands,
    },
    Persona {
        #[command(subcommand)]
        command: PersonaCommands,
    },
    PromptFragment {
        #[command(subcommand)]
        command: PromptFragmentCommands,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommands,
    },
    Permissions {
        #[command(subcommand)]
        command: PermissionsCommands,
    },
    Memory {
        #[command(subcommand)]
        command: MemoryCommands,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    Sync {
        #[command(subcommand)]
        command: SyncCommands,
    },
    Doctor,
    Status,
    Usage,
    Stats,
    Cost,
    Install {
        #[command(subcommand)]
        command: Option<InstallCommands>,
    },
    Upgrade {
        #[command(subcommand)]
        command: Option<UpgradeCommands>,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = crate::search::DEFAULT_SEARCH_LIMIT)]
        limit: usize,
    },
    Skills {
        name: Option<String>,
    },
    Hooks {
        name: Option<String>,
    },
    Server {
        #[command(subcommand)]
        command: ServerCommands,
    },
    RemoteEnv {
        #[command(subcommand)]
        command: RemoteEnvCommands,
    },
    Teleport {
        #[command(subcommand)]
        command: TeleportCommands,
    },
    Assistant {
        #[command(subcommand)]
        command: AssistantCommands,
    },
    Bridge {
        #[command(subcommand)]
        command: BridgeCommands,
    },
    Ide {
        #[command(subcommand)]
        command: IdeCommands,
    },
    Tasks {
        #[command(subcommand)]
        command: TaskCommands,
    },
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommands,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    Chat {
        prompt: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        gateway_url: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_MAX_TURNS)]
        max_turns: usize,
    },
    Repl {
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        gateway_url: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long, default_value_t = DEFAULT_MAX_TURNS)]
        max_turns: usize,
    },
    #[command(hide = true)]
    WorkerRunAgent {
        #[arg(long)]
        job: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum GatewayCommands {
    Serve {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    PrintDefaultConfig,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommands {
    Path,
    Show {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Panel {
        focus_key: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Keys,
    Set {
        key: String,
        value: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Clear {
        key: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ModelCommands {
    Panel {
        profile_name: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    List {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Show {
        profile_name: Option<String>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    SetDefault {
        profile_name: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Save {
        profile_name: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        upstream_model: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        input_price: Option<f64>,
        #[arg(long)]
        output_price: Option<f64>,
        #[arg(long)]
        set_default: bool,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Remove {
        profile_name: String,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PermissionsCommands {
    Show {
        #[arg(long)]
        config: Option<PathBuf>,
    },
    Set {
        mode: PermissionMode,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum MemoryCommands {
    Panel {
        #[arg(long)]
        archived: bool,
        memory_id: Option<String>,
    },
    List {
        #[arg(long)]
        archived: bool,
    },
    Show {
        memory_id: String,
        #[arg(long)]
        archived: bool,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = crate::search::DEFAULT_SEARCH_LIMIT)]
        limit: usize,
        #[arg(long)]
        archived: bool,
    },
    Clusters {
        #[arg(long)]
        archived: bool,
        #[arg(long, default_value_t = 200)]
        limit: usize,
        #[arg(long, default_value_t = 0.18)]
        min_jaccard: f32,
        #[arg(long, default_value_t = 48)]
        max_tokens: usize,
        #[arg(long)]
        semantic: bool,
    },
    Prune {
        #[arg(long, value_enum, default_value_t = MemoryScopeSelector::All)]
        scope: MemoryScopeSelector,
        #[arg(long, default_value_t = 30)]
        older_than_days: u64,
        #[arg(long, default_value_t = 3)]
        keep_latest: usize,
        #[arg(long)]
        apply: bool,
    },
    Archive {
        #[arg(long, value_enum, default_value_t = MemoryScopeSelector::All)]
        scope: MemoryScopeSelector,
        #[arg(long, default_value_t = 30)]
        older_than_days: u64,
        #[arg(long, default_value_t = 3)]
        keep_latest: usize,
        #[arg(long)]
        apply: bool,
    },
    Decay {
        #[arg(long, value_enum, default_value_t = MemoryScopeSelector::All)]
        scope: MemoryScopeSelector,
        #[arg(long, default_value_t = 180)]
        older_than_days: u64,
        #[arg(long, default_value_t = 20)]
        keep_latest: usize,
        #[arg(long, default_value_t = 24)]
        max_summary_lines: usize,
        #[arg(long, default_value_t = 1600)]
        max_summary_chars: usize,
        #[arg(long)]
        apply: bool,
    },
    Capture {
        session_id: String,
        #[arg(long)]
        instructions: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SessionCommands {
    Panel {
        session_id: Option<String>,
    },
    List,
    Show {
        session_id: String,
    },
    Compact {
        session_id: String,
        #[arg(long)]
        instructions: Option<String>,
    },
    Share {
        session_id: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}
