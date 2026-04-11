use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum AuthCommands {
    Status,
    Accounts,
    Devices,
    Show {
        account_id: String,
    },
    Login {
        account_id: String,
        #[arg(long, default_value = "hellox-remote")]
        provider: String,
        #[arg(long)]
        access_token: String,
        #[arg(long)]
        refresh_token: Option<String>,
        #[arg(long = "scope")]
        scopes: Vec<String>,
    },
    Logout {
        account_id: String,
    },
    Keys,
    SetKey {
        provider: String,
        #[arg(long)]
        api_key: String,
    },
    RemoveKey {
        provider: String,
    },
    TrustDevice {
        account_id: String,
        device_name: String,
        #[arg(long = "scope")]
        scopes: Vec<String>,
    },
    RevokeDevice {
        device_id: String,
    },
    OauthStart {
        account_id: String,
        #[arg(long, default_value = "hellox-remote")]
        provider: String,
        #[arg(long)]
        client_id: String,
        #[arg(long)]
        authorize_url: String,
        #[arg(long)]
        token_url: String,
        #[arg(long)]
        redirect_url: String,
        #[arg(long = "scope")]
        scopes: Vec<String>,
        #[arg(long)]
        login_hint: Option<String>,
    },
    OauthExchange {
        account_id: String,
        #[arg(long, default_value = "hellox-remote")]
        provider: String,
        #[arg(long)]
        client_id: String,
        #[arg(long)]
        token_url: String,
        #[arg(long)]
        redirect_url: String,
        #[arg(long)]
        code: String,
        #[arg(long)]
        code_verifier: String,
        #[arg(long = "scope")]
        scopes: Vec<String>,
    },
    OauthRefresh {
        account_id: String,
        #[arg(long)]
        client_id: String,
        #[arg(long)]
        token_url: String,
        #[arg(long)]
        redirect_url: String,
    },
}
