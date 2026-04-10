use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum McpScopeValue {
    User,
    Project,
    Local,
    Dynamic,
    Enterprise,
    Managed,
    Claudeai,
}

#[derive(Debug, Subcommand)]
pub(crate) enum McpCommands {
    List,
    Panel {
        server_name: Option<String>,
    },
    Show {
        server_name: String,
    },
    Tools {
        server_name: String,
    },
    Call {
        server_name: String,
        tool_name: String,
        #[arg(long)]
        input: Option<String>,
    },
    Resources {
        server_name: String,
    },
    Prompts {
        server_name: String,
    },
    ReadResource {
        server_name: String,
        uri: String,
    },
    GetPrompt {
        server_name: String,
        prompt_name: String,
        #[arg(long)]
        input: Option<String>,
    },
    AuthShow {
        server_name: String,
    },
    AuthSetToken {
        server_name: String,
        #[arg(long)]
        bearer_token: String,
    },
    AuthClear {
        server_name: String,
    },
    AuthOauthSet {
        server_name: String,
        #[arg(long)]
        client_id: String,
        #[arg(long)]
        authorize_url: String,
        #[arg(long)]
        token_url: String,
        #[arg(long)]
        redirect_url: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long = "scope")]
        scopes: Vec<String>,
        #[arg(long)]
        login_hint: Option<String>,
        #[arg(long)]
        account_id: Option<String>,
    },
    AuthOauthStart {
        server_name: String,
    },
    AuthOauthExchange {
        server_name: String,
        #[arg(long)]
        code: String,
        #[arg(long)]
        code_verifier: String,
    },
    AuthOauthRefresh {
        server_name: String,
    },
    AuthOauthClear {
        server_name: String,
    },
    RegistryList {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    RegistryShow {
        name: String,
    },
    RegistryInstall {
        name: String,
        #[arg(long)]
        server_name: Option<String>,
        #[arg(long, value_enum, default_value_t = McpScopeValue::User)]
        scope: McpScopeValue,
    },
    AddStdio {
        server_name: String,
        #[arg(long)]
        command: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        #[arg(long = "env")]
        env: Vec<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = McpScopeValue::User)]
        scope: McpScopeValue,
        #[arg(long)]
        description: Option<String>,
    },
    AddSse {
        server_name: String,
        #[arg(long)]
        url: String,
        #[arg(long = "header")]
        headers: Vec<String>,
        #[arg(long)]
        oauth_client_id: Option<String>,
        #[arg(long)]
        oauth_authorize_url: Option<String>,
        #[arg(long)]
        oauth_token_url: Option<String>,
        #[arg(long)]
        oauth_redirect_url: Option<String>,
        #[arg(long)]
        oauth_provider: Option<String>,
        #[arg(long = "oauth-scope")]
        oauth_scopes: Vec<String>,
        #[arg(long)]
        oauth_login_hint: Option<String>,
        #[arg(long)]
        oauth_account_id: Option<String>,
        #[arg(long, value_enum, default_value_t = McpScopeValue::User)]
        scope: McpScopeValue,
        #[arg(long)]
        description: Option<String>,
    },
    AddWs {
        server_name: String,
        #[arg(long)]
        url: String,
        #[arg(long = "header")]
        headers: Vec<String>,
        #[arg(long)]
        oauth_client_id: Option<String>,
        #[arg(long)]
        oauth_authorize_url: Option<String>,
        #[arg(long)]
        oauth_token_url: Option<String>,
        #[arg(long)]
        oauth_redirect_url: Option<String>,
        #[arg(long)]
        oauth_provider: Option<String>,
        #[arg(long = "oauth-scope")]
        oauth_scopes: Vec<String>,
        #[arg(long)]
        oauth_login_hint: Option<String>,
        #[arg(long)]
        oauth_account_id: Option<String>,
        #[arg(long, value_enum, default_value_t = McpScopeValue::User)]
        scope: McpScopeValue,
        #[arg(long)]
        description: Option<String>,
    },
    Enable {
        server_name: String,
    },
    Disable {
        server_name: String,
    },
    Remove {
        server_name: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum PluginCommands {
    List,
    Panel {
        plugin_id: Option<String>,
    },
    Show {
        plugin_id: String,
    },
    Install {
        source: PathBuf,
        #[arg(long)]
        disabled: bool,
    },
    Enable {
        plugin_id: String,
    },
    Disable {
        plugin_id: String,
    },
    Remove {
        plugin_id: String,
    },
    Marketplace {
        #[command(subcommand)]
        command: MarketplaceCommands,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum MarketplaceCommands {
    List,
    Show {
        marketplace_name: String,
    },
    Add {
        marketplace_name: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        description: Option<String>,
    },
    Enable {
        marketplace_name: String,
    },
    Disable {
        marketplace_name: String,
    },
    Remove {
        marketplace_name: String,
    },
}
