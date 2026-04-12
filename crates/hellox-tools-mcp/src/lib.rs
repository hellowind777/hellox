mod auth;
mod config;
mod lsp;
mod oauth;
mod registry;
mod runtime;
mod tools;

pub use auth::{
    auth_backend_for_config_path, clear_bearer_token, default_auth_backend, format_auth_status,
    load_bearer_token, set_bearer_token, supports_bearer_token, transport_headers_with_auth,
    TransportAuthStatus,
};
pub use config::{
    add_server, build_stdio_server, build_stream_server, clear_server_oauth, format_server_detail,
    format_server_list, get_server, parse_key_value_pairs, remove_server, set_server_enabled,
    set_server_oauth, StreamTransportKind,
};
pub use lsp::LspTool;
pub use oauth::{
    clear_server_oauth_account, exchange_server_oauth_authorization_code, oauth_status,
    refresh_server_oauth_access_token, resolve_oauth_client_config,
    resolve_server_oauth_access_token, start_server_oauth_authorization, McpOAuthStatus,
};
pub use registry::{
    format_registry_detail, format_registry_list, get_registry_server_latest,
    install_registry_server, list_registry_servers, McpRegistryInstallResult, RegistryServerEntry,
    RegistryServerList,
};
pub use runtime::{
    call_tool, format_prompt_get, format_prompt_list, format_resource_list, format_resource_read,
    format_tool_call, format_tool_list, get_prompt, list_prompts, list_resources, list_tools,
    parse_prompt_arguments, parse_tool_call_arguments, read_resource,
};
pub use tools::{
    register_tools, GetMcpPromptTool, ListMcpPromptsTool, ListMcpResourcesTool, McpAuthTool,
    McpTool, McpToolContext, ReadMcpResourceTool,
};
