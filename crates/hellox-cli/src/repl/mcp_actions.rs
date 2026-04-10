use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_config::{load_or_default, save_config, McpOAuthConfig, McpScope};
use hellox_tools_mcp::{
    add_server, auth_backend_for_config_path, build_stdio_server, build_stream_server,
    call_tool as mcp_call_tool, clear_bearer_token, clear_server_oauth_account,
    exchange_server_oauth_authorization_code, format_auth_status, format_prompt_get,
    format_prompt_list, format_registry_detail, format_registry_list, format_resource_list,
    format_resource_read, format_server_detail, format_server_list, format_tool_call,
    format_tool_list, get_prompt as mcp_get_prompt, get_registry_server_latest, get_server,
    install_registry_server, list_prompts as mcp_list_prompts,
    list_registry_servers as mcp_list_registry_servers, list_resources as mcp_list_resources,
    list_tools as mcp_list_tools, parse_prompt_arguments, parse_tool_call_arguments,
    read_resource as mcp_read_resource, refresh_server_oauth_access_token, remove_server,
    set_bearer_token, set_server_enabled, set_server_oauth, start_server_oauth_authorization,
    StreamTransportKind,
};

use super::commands::McpCommand;
use super::ReplMetadata;
use crate::mcp_panel::render_mcp_panel;

pub(super) fn handle_mcp_command(command: McpCommand, metadata: &ReplMetadata) -> Result<String> {
    let auth_backend = auth_backend_for_config_path(&metadata.config_path);

    match command {
        McpCommand::Help => Ok(help_text()),
        McpCommand::Panel { server_name } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            render_mcp_panel(&metadata.config_path, &config, server_name.as_deref())
        }
        McpCommand::List => Ok(format_server_list(&load_or_default(Some(
            metadata.config_path.clone(),
        ))?)),
        McpCommand::Show { server_name: None } => Ok("Usage: /mcp show <server-name>".to_string()),
        McpCommand::Show {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            Ok(format_server_detail(
                &server_name,
                get_server(&config, &server_name)?,
            ))
        }
        McpCommand::Tools { server_name: None } => {
            Ok("Usage: /mcp tools <server-name>".to_string())
        }
        McpCommand::Tools {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let result = mcp_list_tools(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
            )?;
            Ok(format_tool_list(&server_name, &result))
        }
        McpCommand::Call {
            server_name: None, ..
        } => Ok("Usage: /mcp call <server-name> <tool-name> [json-object]".to_string()),
        McpCommand::Call {
            tool_name: None, ..
        } => Ok("Usage: /mcp call <server-name> <tool-name> [json-object]".to_string()),
        McpCommand::Call {
            server_name: Some(server_name),
            tool_name: Some(tool_name),
            input,
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let arguments = parse_tool_call_arguments(input.as_deref())?;
            let result = mcp_call_tool(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
                &tool_name,
                arguments,
            )?;
            Ok(format_tool_call(&server_name, &tool_name, &result))
        }
        McpCommand::Resources { server_name: None } => {
            Ok("Usage: /mcp resources <server-name>".to_string())
        }
        McpCommand::Resources {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let result = mcp_list_resources(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
            )?;
            Ok(format_resource_list(&server_name, &result))
        }
        McpCommand::Prompts { server_name: None } => {
            Ok("Usage: /mcp prompts <server-name>".to_string())
        }
        McpCommand::Prompts {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let result = mcp_list_prompts(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
            )?;
            Ok(format_prompt_list(&server_name, &result))
        }
        McpCommand::ReadResource {
            server_name: None, ..
        } => Ok("Usage: /mcp read-resource <server-name> <uri>".to_string()),
        McpCommand::ReadResource { uri: None, .. } => {
            Ok("Usage: /mcp read-resource <server-name> <uri>".to_string())
        }
        McpCommand::ReadResource {
            server_name: Some(server_name),
            uri: Some(uri),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let result = mcp_read_resource(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
                &uri,
            )?;
            Ok(format_resource_read(&server_name, &uri, &result))
        }
        McpCommand::GetPrompt {
            server_name: None, ..
        } => Ok("Usage: /mcp get-prompt <server-name> <prompt-name> [json-object]".to_string()),
        McpCommand::GetPrompt {
            prompt_name: None, ..
        } => Ok("Usage: /mcp get-prompt <server-name> <prompt-name> [json-object]".to_string()),
        McpCommand::GetPrompt {
            server_name: Some(server_name),
            prompt_name: Some(prompt_name),
            input,
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let arguments = parse_prompt_arguments(input.as_deref())?;
            let result = mcp_get_prompt(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
                &prompt_name,
                arguments,
            )?;
            Ok(format_prompt_get(&server_name, &prompt_name, &result))
        }
        McpCommand::AuthShow { server_name: None } => {
            Ok("Usage: /mcp auth show <server-name>".to_string())
        }
        McpCommand::AuthShow {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            Ok(format_auth_status(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
            )?)
        }
        McpCommand::AuthSetToken {
            server_name: None, ..
        } => Ok("Usage: /mcp auth set-token <server-name> <bearer-token>".to_string()),
        McpCommand::AuthSetToken {
            bearer_token: None, ..
        } => Ok("Usage: /mcp auth set-token <server-name> <bearer-token>".to_string()),
        McpCommand::AuthSetToken {
            server_name: Some(server_name),
            bearer_token: Some(bearer_token),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            set_bearer_token(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
                bearer_token,
            )?;
            Ok(format!("Stored MCP bearer token for `{server_name}`."))
        }
        McpCommand::AuthClear { server_name: None } => {
            Ok("Usage: /mcp auth clear <server-name>".to_string())
        }
        McpCommand::AuthClear {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            get_server(&config, &server_name)?;
            Ok(if clear_bearer_token(&auth_backend, &server_name)? {
                format!("Cleared MCP bearer token for `{server_name}`.")
            } else {
                format!("No stored MCP bearer token found for `{server_name}`.")
            })
        }
        McpCommand::AuthOauthSet {
            server_name: None, ..
        } => Ok(
            "Usage: /mcp auth oauth-set <server-name> <client-id> <authorize-url> <token-url> <redirect-url> [scope...]"
                .to_string(),
        ),
        McpCommand::AuthOauthSet {
            client_id: None, ..
        } => Ok(
            "Usage: /mcp auth oauth-set <server-name> <client-id> <authorize-url> <token-url> <redirect-url> [scope...]"
                .to_string(),
        ),
        McpCommand::AuthOauthSet {
            authorize_url: None, ..
        } => Ok(
            "Usage: /mcp auth oauth-set <server-name> <client-id> <authorize-url> <token-url> <redirect-url> [scope...]"
                .to_string(),
        ),
        McpCommand::AuthOauthSet { token_url: None, .. } => Ok(
            "Usage: /mcp auth oauth-set <server-name> <client-id> <authorize-url> <token-url> <redirect-url> [scope...]"
                .to_string(),
        ),
        McpCommand::AuthOauthSet {
            redirect_url: None, ..
        } => Ok(
            "Usage: /mcp auth oauth-set <server-name> <client-id> <authorize-url> <token-url> <redirect-url> [scope...]"
                .to_string(),
        ),
        McpCommand::AuthOauthSet {
            server_name: Some(server_name),
            client_id: Some(client_id),
            authorize_url: Some(authorize_url),
            token_url: Some(token_url),
            redirect_url: Some(redirect_url),
            scopes,
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            set_server_oauth(
                &mut config,
                &server_name,
                McpOAuthConfig {
                    provider: None,
                    client_id,
                    authorize_url,
                    token_url,
                    redirect_url,
                    scopes,
                    login_hint: None,
                    account_id: None,
                },
            )?;
            let saved_path = save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!(
                "Configured MCP OAuth for `{server_name}` in `{}`.",
                format_path(&saved_path)
            ))
        }
        McpCommand::AuthOauthStart { server_name: None } => {
            Ok("Usage: /mcp auth oauth-start <server-name>".to_string())
        }
        McpCommand::AuthOauthStart {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let request =
                start_server_oauth_authorization(&server_name, get_server(&config, &server_name)?)?;
            Ok(format!(
                "server: {server_name}\nauthorization_url: {}\ncode_verifier: {}\nstate: {}",
                request.authorization_url, request.code_verifier, request.state
            ))
        }
        McpCommand::AuthOauthExchange {
            server_name: None, ..
        } => Ok(
            "Usage: /mcp auth oauth-exchange <server-name> <code> <code-verifier>".to_string(),
        ),
        McpCommand::AuthOauthExchange { code: None, .. } => Ok(
            "Usage: /mcp auth oauth-exchange <server-name> <code> <code-verifier>".to_string(),
        ),
        McpCommand::AuthOauthExchange {
            code_verifier: None, ..
        } => Ok(
            "Usage: /mcp auth oauth-exchange <server-name> <code> <code-verifier>".to_string(),
        ),
        McpCommand::AuthOauthExchange {
            server_name: Some(server_name),
            code: Some(code),
            code_verifier: Some(code_verifier),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let account = exchange_server_oauth_authorization_code(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
                &code,
                &code_verifier,
            )?;
            Ok(format!(
                "Stored MCP OAuth account `{}` for `{server_name}` (provider: {}).",
                account.account_id, account.provider
            ))
        }
        McpCommand::AuthOauthRefresh { server_name: None } => {
            Ok("Usage: /mcp auth oauth-refresh <server-name>".to_string())
        }
        McpCommand::AuthOauthRefresh {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            let account = refresh_server_oauth_access_token(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
            )?;
            Ok(format!(
                "Refreshed MCP OAuth account `{}` for `{server_name}`.",
                account.account_id
            ))
        }
        McpCommand::AuthOauthClear { server_name: None } => {
            Ok("Usage: /mcp auth oauth-clear <server-name>".to_string())
        }
        McpCommand::AuthOauthClear {
            server_name: Some(server_name),
        } => {
            let config = load_or_default(Some(metadata.config_path.clone()))?;
            Ok(if clear_server_oauth_account(
                &auth_backend,
                &server_name,
                get_server(&config, &server_name)?,
            )? {
                format!(
                    "Cleared linked MCP OAuth account for `{server_name}`. OAuth client config remains in `{}`.",
                    format_path(&metadata.config_path)
                )
            } else {
                format!("No linked MCP OAuth account found for `{server_name}`.")
            })
        }
        McpCommand::RegistryList { cursor, limit } => {
            Ok(format_registry_list(&mcp_list_registry_servers(
                cursor.as_deref(),
                limit,
            )?))
        }
        McpCommand::RegistryShow { name: None } => {
            Ok("Usage: /mcp registry show <name>".to_string())
        }
        McpCommand::RegistryShow { name: Some(name) } => {
            Ok(format_registry_detail(&get_registry_server_latest(&name)?))
        }
        McpCommand::RegistryInstall { name: None, .. } => {
            Ok("Usage: /mcp registry install <name> [server-name] [scope]".to_string())
        }
        McpCommand::RegistryInstall {
            name: Some(name),
            server_name,
            scope,
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            let result = install_registry_server(
                &mut config,
                &name,
                server_name.as_deref(),
                parse_scope(scope.as_deref())?,
            )?;
            let saved_path = save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!(
                "Installed MCP registry server `{}` as `{}` using `{}` -> {} in `{}`.",
                result.registry_name,
                result.installed_server_name,
                result.transport,
                result.url,
                format_path(&saved_path)
            ))
        }
        McpCommand::AddStdio {
            server_name: None, ..
        } => Ok("Usage: /mcp add stdio <server-name> <command> [args...]".to_string()),
        McpCommand::AddStdio { command: None, .. } => {
            Ok("Usage: /mcp add stdio <server-name> <command> [args...]".to_string())
        }
        McpCommand::AddStdio {
            server_name: Some(server_name),
            command: Some(command),
            args,
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            let server =
                build_stdio_server(command, args, BTreeMap::new(), None, McpScope::User, None);
            add_server(&mut config, server_name.clone(), server)?;
            let saved_path = save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!(
                "Added MCP server `{server_name}` to `{}`.",
                format_path(&saved_path)
            ))
        }
        McpCommand::AddSse {
            server_name: None, ..
        } => Ok("Usage: /mcp add sse <server-name> <url>".to_string()),
        McpCommand::AddSse { url: None, .. } => {
            Ok("Usage: /mcp add sse <server-name> <url>".to_string())
        }
        McpCommand::AddSse {
            server_name: Some(server_name),
            url: Some(url),
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            let server = build_stream_server(
                StreamTransportKind::Sse,
                url,
                BTreeMap::new(),
                McpScope::User,
                None,
                None,
            )?;
            add_server(&mut config, server_name.clone(), server)?;
            let saved_path = save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!(
                "Added MCP server `{server_name}` to `{}`.",
                format_path(&saved_path)
            ))
        }
        McpCommand::AddWs {
            server_name: None, ..
        } => Ok("Usage: /mcp add ws <server-name> <url>".to_string()),
        McpCommand::AddWs { url: None, .. } => {
            Ok("Usage: /mcp add ws <server-name> <url>".to_string())
        }
        McpCommand::AddWs {
            server_name: Some(server_name),
            url: Some(url),
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            let server = build_stream_server(
                StreamTransportKind::Ws,
                url,
                BTreeMap::new(),
                McpScope::User,
                None,
                None,
            )?;
            add_server(&mut config, server_name.clone(), server)?;
            let saved_path = save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!(
                "Added MCP server `{server_name}` to `{}`.",
                format_path(&saved_path)
            ))
        }
        McpCommand::Enable { server_name: None } => {
            Ok("Usage: /mcp enable <server-name>".to_string())
        }
        McpCommand::Enable {
            server_name: Some(server_name),
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            set_server_enabled(&mut config, &server_name, true)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Enabled MCP server `{server_name}`."))
        }
        McpCommand::Disable { server_name: None } => {
            Ok("Usage: /mcp disable <server-name>".to_string())
        }
        McpCommand::Disable {
            server_name: Some(server_name),
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            set_server_enabled(&mut config, &server_name, false)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Disabled MCP server `{server_name}`."))
        }
        McpCommand::Remove { server_name: None } => {
            Ok("Usage: /mcp remove <server-name>".to_string())
        }
        McpCommand::Remove {
            server_name: Some(server_name),
        } => {
            let mut config = load_or_default(Some(metadata.config_path.clone()))?;
            remove_server(&mut config, &server_name)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Removed MCP server `{server_name}`."))
        }
    }
}

fn help_text() -> String {
    [
        "Usage:",
        "  /mcp",
        "  /mcp panel [server-name]",
        "  /mcp show <server-name>",
        "  /mcp tools <server-name>",
        "  /mcp call <server-name> <tool-name> [json-object]",
        "  /mcp resources <server-name>",
        "  /mcp prompts <server-name>",
        "  /mcp read-resource <server-name> <uri>",
        "  /mcp get-prompt <server-name> <prompt-name> [json-object]",
        "  /mcp auth show <server-name>",
        "  /mcp auth set-token <server-name> <bearer-token>",
        "  /mcp auth clear <server-name>",
        "  /mcp auth oauth-set <server-name> <client-id> <authorize-url> <token-url> <redirect-url> [scope...]",
        "  /mcp auth oauth-start <server-name>",
        "  /mcp auth oauth-exchange <server-name> <code> <code-verifier>",
        "  /mcp auth oauth-refresh <server-name>",
        "  /mcp auth oauth-clear <server-name>",
        "  /mcp registry list [cursor] [limit]",
        "  /mcp registry show <name>",
        "  /mcp registry install <name> [server-name] [scope]",
        "  /mcp add stdio <server-name> <command> [args...]",
        "  /mcp add sse <server-name> <url>",
        "  /mcp add ws <server-name> <url>",
        "  /mcp enable <server-name>",
        "  /mcp disable <server-name>",
        "  /mcp remove <server-name>",
    ]
    .join("\n")
}

fn format_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn parse_scope(value: Option<&str>) -> Result<McpScope> {
    match value.map(str::trim).filter(|item| !item.is_empty()) {
        None => Ok(McpScope::User),
        Some(raw) => raw.parse::<McpScope>().map_err(|error| anyhow!(error)),
    }
}
