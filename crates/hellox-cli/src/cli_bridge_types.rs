use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum BridgeCommands {
    Status,
    Sessions,
    ShowSession { session_id: String },
    Stdio,
}

#[derive(Debug, Subcommand)]
pub(crate) enum IdeCommands {
    Status,
}
