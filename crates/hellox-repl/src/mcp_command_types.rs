#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpCommand {
    List,
    Panel {
        server_name: Option<String>,
    },
    Show {
        server_name: Option<String>,
    },
    Tools {
        server_name: Option<String>,
    },
    Call {
        server_name: Option<String>,
        tool_name: Option<String>,
        input: Option<String>,
    },
    Resources {
        server_name: Option<String>,
    },
    Prompts {
        server_name: Option<String>,
    },
    ReadResource {
        server_name: Option<String>,
        uri: Option<String>,
    },
    GetPrompt {
        server_name: Option<String>,
        prompt_name: Option<String>,
        input: Option<String>,
    },
    AuthShow {
        server_name: Option<String>,
    },
    AuthSetToken {
        server_name: Option<String>,
        bearer_token: Option<String>,
    },
    AuthClear {
        server_name: Option<String>,
    },
    AuthOauthSet {
        server_name: Option<String>,
        client_id: Option<String>,
        authorize_url: Option<String>,
        token_url: Option<String>,
        redirect_url: Option<String>,
        scopes: Vec<String>,
    },
    AuthOauthStart {
        server_name: Option<String>,
    },
    AuthOauthExchange {
        server_name: Option<String>,
        code: Option<String>,
        code_verifier: Option<String>,
    },
    AuthOauthRefresh {
        server_name: Option<String>,
    },
    AuthOauthClear {
        server_name: Option<String>,
    },
    RegistryList {
        cursor: Option<String>,
        limit: Option<usize>,
    },
    RegistryShow {
        name: Option<String>,
    },
    RegistryInstall {
        name: Option<String>,
        server_name: Option<String>,
        scope: Option<String>,
    },
    AddStdio {
        server_name: Option<String>,
        command: Option<String>,
        args: Vec<String>,
    },
    AddSse {
        server_name: Option<String>,
        url: Option<String>,
    },
    AddWs {
        server_name: Option<String>,
        url: Option<String>,
    },
    Enable {
        server_name: Option<String>,
    },
    Disable {
        server_name: Option<String>,
    },
    Remove {
        server_name: Option<String>,
    },
    Help,
}
