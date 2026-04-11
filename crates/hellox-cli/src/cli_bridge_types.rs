use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum BridgeCommands {
    Status,
    Panel { session_id: Option<String> },
    Sessions,
    ShowSession { session_id: String },
    Stdio,
}

#[derive(Debug, Subcommand)]
pub(crate) enum IdeCommands {
    Status,
    Panel,
}
