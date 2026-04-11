use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_config::{HelloxConfig, McpOAuthConfig, McpScope, McpServerConfig, McpTransportConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamTransportKind {
    Sse,
    Ws,
}

pub fn format_server_list(config: &HelloxConfig) -> String {
    if config.mcp.servers.is_empty() {
        return "No MCP servers configured.".to_string();
    }

    let mut lines = vec!["name\tenabled\ttransport\tscope\tdescription".to_string()];
    for (name, server) in &config.mcp.servers {
        lines.push(format!(
            "{name}\t{}\t{}\t{}\t{}",
            yes_no(server.enabled),
            server.transport.kind(),
            server.scope,
            server.description.as_deref().unwrap_or("-"),
        ));
    }
    lines.join("\n")
}

pub fn format_server_detail(server_name: &str, server: &McpServerConfig) -> String {
    let mut lines = vec![
        format!("server: {server_name}"),
        format!("enabled: {}", yes_no(server.enabled)),
        format!("scope: {}", server.scope),
        format!(
            "description: {}",
            server.description.as_deref().unwrap_or("(none)")
        ),
        format!("transport: {}", server.transport.kind()),
        format!(
            "oauth: {}",
            if server.oauth.is_some() {
                "configured"
            } else {
                "(none)"
            }
        ),
    ];

    match &server.transport {
        McpTransportConfig::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            lines.push(format!("command: {command}"));
            lines.push(format!(
                "args: {}",
                if args.is_empty() {
                    "(none)".to_string()
                } else {
                    args.join(" ")
                }
            ));
            lines.push(format!(
                "cwd: {}",
                cwd.as_deref().unwrap_or("(inherit current process)")
            ));
            lines.push(format!(
                "env: {}",
                if env.is_empty() {
                    "(none)".to_string()
                } else {
                    env.iter()
                        .map(|(key, value)| format!("{key}={}", mask_secret(value)))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        }
        McpTransportConfig::Sse { url, headers } | McpTransportConfig::Ws { url, headers } => {
            lines.push(format!("url: {url}"));
            lines.push(format!(
                "headers: {}",
                if headers.is_empty() {
                    "(none)".to_string()
                } else {
                    headers
                        .iter()
                        .map(|(key, value)| format!("{key}={}", mask_header_value(key, value)))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        }
    }

    if let Some(oauth) = &server.oauth {
        lines.push(format!(
            "oauth_provider: {}",
            oauth.provider.as_deref().unwrap_or("(default)")
        ));
        lines.push(format!("oauth_client_id: {}", oauth.client_id));
        lines.push(format!("oauth_authorize_url: {}", oauth.authorize_url));
        lines.push(format!("oauth_token_url: {}", oauth.token_url));
        lines.push(format!("oauth_redirect_url: {}", oauth.redirect_url));
        lines.push(format!(
            "oauth_account_id: {}",
            oauth.account_id.as_deref().unwrap_or("(mcp:<server-name>)")
        ));
        lines.push(format!(
            "oauth_login_hint: {}",
            oauth.login_hint.as_deref().unwrap_or("(none)")
        ));
        lines.push(format!(
            "oauth_scopes: {}",
            if oauth.scopes.is_empty() {
                "(none)".to_string()
            } else {
                oauth.scopes.join(", ")
            }
        ));
    }

    lines.join("\n")
}

pub fn get_server<'a>(config: &'a HelloxConfig, server_name: &str) -> Result<&'a McpServerConfig> {
    config
        .mcp
        .servers
        .get(server_name)
        .ok_or_else(|| anyhow!("MCP server `{server_name}` was not found"))
}

pub fn add_server(
    config: &mut HelloxConfig,
    server_name: String,
    server: McpServerConfig,
) -> Result<()> {
    let server_name = server_name.trim();
    if server_name.is_empty() {
        return Err(anyhow!("MCP server name cannot be empty"));
    }

    config.mcp.servers.insert(server_name.to_string(), server);
    Ok(())
}

pub fn remove_server(config: &mut HelloxConfig, server_name: &str) -> Result<McpServerConfig> {
    config
        .mcp
        .servers
        .remove(server_name)
        .ok_or_else(|| anyhow!("MCP server `{server_name}` was not found"))
}

pub fn set_server_enabled(
    config: &mut HelloxConfig,
    server_name: &str,
    enabled: bool,
) -> Result<()> {
    let server = config
        .mcp
        .servers
        .get_mut(server_name)
        .ok_or_else(|| anyhow!("MCP server `{server_name}` was not found"))?;
    server.enabled = enabled;
    Ok(())
}

pub fn set_server_oauth(
    config: &mut HelloxConfig,
    server_name: &str,
    oauth: McpOAuthConfig,
) -> Result<()> {
    let server = config
        .mcp
        .servers
        .get_mut(server_name)
        .ok_or_else(|| anyhow!("MCP server `{server_name}` was not found"))?;
    if matches!(server.transport, McpTransportConfig::Stdio { .. }) {
        return Err(anyhow!(
            "MCP OAuth is only supported for HTTP/SSE or WebSocket servers."
        ));
    }

    server.oauth = Some(sanitize_oauth(oauth)?);
    Ok(())
}

pub fn clear_server_oauth(config: &mut HelloxConfig, server_name: &str) -> Result<bool> {
    let server = config
        .mcp
        .servers
        .get_mut(server_name)
        .ok_or_else(|| anyhow!("MCP server `{server_name}` was not found"))?;
    Ok(server.oauth.take().is_some())
}

pub fn build_stdio_server(
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: Option<&Path>,
    scope: McpScope,
    description: Option<String>,
) -> McpServerConfig {
    McpServerConfig {
        enabled: true,
        description: sanitize_optional(description),
        scope,
        oauth: None,
        transport: McpTransportConfig::Stdio {
            command: command.trim().to_string(),
            args: sanitize_list(args),
            env: env
                .into_iter()
                .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
                .collect(),
            cwd: cwd.map(|path| path.display().to_string()),
        },
    }
}

pub fn build_stream_server(
    kind: StreamTransportKind,
    url: String,
    headers: BTreeMap<String, String>,
    scope: McpScope,
    description: Option<String>,
    oauth: Option<McpOAuthConfig>,
) -> Result<McpServerConfig> {
    let transport = match kind {
        StreamTransportKind::Sse => McpTransportConfig::Sse {
            url: url.trim().to_string(),
            headers: sanitize_map(headers),
        },
        StreamTransportKind::Ws => McpTransportConfig::Ws {
            url: url.trim().to_string(),
            headers: sanitize_map(headers),
        },
    };

    Ok(McpServerConfig {
        enabled: true,
        description: sanitize_optional(description),
        scope,
        oauth: match oauth {
            Some(value) => Some(sanitize_oauth(value)?),
            None => None,
        },
        transport,
    })
}

pub fn parse_key_value_pairs(items: &[String], label: &str) -> Result<BTreeMap<String, String>> {
    let mut entries = BTreeMap::new();
    for item in items {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| anyhow!("Invalid {label} entry `{item}`. Use KEY=VALUE format."))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow!(
                "Invalid {label} entry `{item}`. Key cannot be empty."
            ));
        }
        entries.insert(key.to_string(), value.trim().to_string());
    }
    Ok(entries)
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn sanitize_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn sanitize_map(values: BTreeMap<String, String>) -> BTreeMap<String, String> {
    values
        .into_iter()
        .filter_map(|(key, value)| {
            let key = key.trim().to_string();
            if key.is_empty() {
                None
            } else {
                Some((key, value.trim().to_string()))
            }
        })
        .collect()
}

fn sanitize_oauth(value: McpOAuthConfig) -> Result<McpOAuthConfig> {
    let client_id = value.client_id.trim().to_string();
    if client_id.is_empty() {
        return Err(anyhow!("MCP OAuth client_id cannot be empty"));
    }

    let authorize_url = value.authorize_url.trim().to_string();
    if authorize_url.is_empty() {
        return Err(anyhow!("MCP OAuth authorize_url cannot be empty"));
    }

    let token_url = value.token_url.trim().to_string();
    if token_url.is_empty() {
        return Err(anyhow!("MCP OAuth token_url cannot be empty"));
    }

    let redirect_url = value.redirect_url.trim().to_string();
    if redirect_url.is_empty() {
        return Err(anyhow!("MCP OAuth redirect_url cannot be empty"));
    }

    Ok(McpOAuthConfig {
        provider: sanitize_optional(value.provider),
        client_id,
        authorize_url,
        token_url,
        redirect_url,
        scopes: sanitize_scopes(value.scopes),
        login_hint: sanitize_optional(value.login_hint),
        account_id: sanitize_optional(value.account_id),
    })
}

fn sanitize_scopes(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .flat_map(|item| {
            item.split([' ', ','])
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn mask_header_value(key: &str, value: &str) -> String {
    if key.eq_ignore_ascii_case("authorization") {
        mask_secret(value)
    } else {
        value.to_string()
    }
}

fn mask_secret(value: &str) -> String {
    if value.len() <= 8 {
        "********".to_string()
    } else {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_value_pairs() {
        let map = parse_key_value_pairs(
            &[String::from("MODE=rw"), String::from("TOKEN=abc=123")],
            "env",
        )
        .expect("parse key value pairs");

        assert_eq!(map.get("MODE"), Some(&String::from("rw")));
        assert_eq!(map.get("TOKEN"), Some(&String::from("abc=123")));
    }

    #[test]
    fn formats_server_list() {
        let mut config = HelloxConfig::default();
        add_server(
            &mut config,
            String::from("filesystem"),
            build_stdio_server(
                String::from("npx"),
                vec![String::from("@modelcontextprotocol/server-filesystem")],
                BTreeMap::new(),
                None,
                McpScope::User,
                Some(String::from("Workspace files")),
            ),
        )
        .expect("add server");

        let text = format_server_list(&config);
        assert!(text.contains("filesystem"));
        assert!(text.contains("stdio"));
        assert!(text.contains("Workspace files"));
    }

    #[test]
    fn sets_and_clears_server_oauth() {
        let mut config = HelloxConfig::default();
        add_server(
            &mut config,
            String::from("docs"),
            build_stream_server(
                StreamTransportKind::Sse,
                String::from("https://example.test/mcp"),
                BTreeMap::new(),
                McpScope::User,
                None,
                None,
            )
            .expect("build stream server"),
        )
        .expect("add server");

        set_server_oauth(
            &mut config,
            "docs",
            McpOAuthConfig {
                provider: Some(String::from("docs-oauth")),
                client_id: String::from("client-123"),
                authorize_url: String::from("https://auth.example.test/authorize"),
                token_url: String::from("https://auth.example.test/token"),
                redirect_url: String::from("http://127.0.0.1:8910/callback"),
                scopes: vec![String::from("openid profile"), String::from("mcp:docs")],
                login_hint: Some(String::from("hello@example.test")),
                account_id: Some(String::from("mcp:docs")),
            },
        )
        .expect("set oauth");

        let server = config.mcp.servers.get("docs").expect("docs server");
        let oauth = server.oauth.as_ref().expect("oauth config");
        assert_eq!(oauth.client_id, "client-123");
        assert_eq!(
            oauth.scopes,
            vec![
                String::from("openid"),
                String::from("profile"),
                String::from("mcp:docs")
            ]
        );

        assert!(clear_server_oauth(&mut config, "docs").expect("clear oauth"));
        assert!(config
            .mcp
            .servers
            .get("docs")
            .expect("docs server")
            .oauth
            .is_none());
    }
}
