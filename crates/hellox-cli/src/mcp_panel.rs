use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_config::{HelloxConfig, McpServerConfig, McpTransportConfig};
use hellox_tui::{render_panel, render_table, KeyValueRow, PanelSection, Table};

pub(crate) fn render_mcp_panel(
    config_path: &Path,
    config: &HelloxConfig,
    server_name: Option<&str>,
) -> Result<String> {
    let server_name = server_name.map(str::trim).filter(|value| !value.is_empty());
    match server_name {
        Some(server_name) => render_mcp_detail_panel(config_path, config, server_name),
        None => Ok(render_mcp_list_panel(config_path, config)),
    }
}

fn render_mcp_list_panel(config_path: &Path, config: &HelloxConfig) -> String {
    let total = config.mcp.servers.len();
    let enabled = config
        .mcp
        .servers
        .values()
        .filter(|server| server.enabled)
        .count();
    let oauth = config
        .mcp
        .servers
        .values()
        .filter(|server| server.oauth.is_some())
        .count();

    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("servers", total.to_string()),
        KeyValueRow::new("enabled", enabled.to_string()),
        KeyValueRow::new("oauth_configured", oauth.to_string()),
    ];
    let sections = vec![
        PanelSection::new("Servers", render_table(&build_mcp_server_table(config))),
        PanelSection::new("Action palette", mcp_list_cli_palette()),
        PanelSection::new("REPL palette", mcp_list_repl_palette()),
    ];

    render_panel("MCP panel", &metadata, &sections)
}

fn render_mcp_detail_panel(
    config_path: &Path,
    config: &HelloxConfig,
    server_name: &str,
) -> Result<String> {
    let server = config
        .mcp
        .servers
        .get(server_name)
        .ok_or_else(|| anyhow!("MCP server `{server_name}` was not found"))?;

    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("server", server_name.to_string()),
        KeyValueRow::new("enabled", yes_no(server.enabled)),
        KeyValueRow::new("scope", server.scope.to_string()),
        KeyValueRow::new("transport", server.transport.kind()),
        KeyValueRow::new(
            "description",
            server.description.as_deref().unwrap_or("(none)"),
        ),
        KeyValueRow::new("oauth", yes_no(server.oauth.is_some())),
    ];
    let sections = vec![
        PanelSection::new("Transport", transport_lines(&server.transport)),
        PanelSection::new("OAuth", oauth_lines(server)),
        PanelSection::new("Action palette", mcp_detail_cli_palette(server_name)),
        PanelSection::new("REPL palette", mcp_detail_repl_palette(server_name)),
    ];

    Ok(render_panel(
        &format!("MCP server panel: {server_name}"),
        &metadata,
        &sections,
    ))
}

fn build_mcp_server_table(config: &HelloxConfig) -> Table {
    let rows = config
        .mcp
        .servers
        .iter()
        .enumerate()
        .map(|(index, (server_name, server))| {
            vec![
                (index + 1).to_string(),
                server_name.clone(),
                yes_no(server.enabled),
                server.scope.to_string(),
                server.transport.kind().to_string(),
                yes_no(server.oauth.is_some()),
                preview_text(server.description.as_deref().unwrap_or("(none)"), 40),
                format!("hellox mcp panel {server_name}"),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "#".to_string(),
            "server".to_string(),
            "enabled".to_string(),
            "scope".to_string(),
            "transport".to_string(),
            "oauth".to_string(),
            "description".to_string(),
            "open".to_string(),
        ],
        rows,
    )
}

fn transport_lines(transport: &McpTransportConfig) -> Vec<String> {
    match transport {
        McpTransportConfig::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            let mut lines = vec![format!("command: {command}")];
            lines.push(format!(
                "args: {}",
                if args.is_empty() {
                    "(none)".to_string()
                } else {
                    args.join(" ")
                }
            ));
            lines.push(format!("cwd: {}", cwd.as_deref().unwrap_or("(none)")));
            lines.push(format!(
                "env: {}",
                if env.is_empty() {
                    "(none)".to_string()
                } else {
                    env.iter()
                        .map(|(key, value)| format!("{key}={value}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
            lines
        }
        McpTransportConfig::Sse { url, headers } | McpTransportConfig::Ws { url, headers } => {
            vec![
                format!("url: {url}"),
                format!(
                    "headers: {}",
                    if headers.is_empty() {
                        "(none)".to_string()
                    } else {
                        headers
                            .iter()
                            .map(|(key, value)| format!("{key}={value}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                ),
            ]
        }
    }
}

fn oauth_lines(server: &McpServerConfig) -> Vec<String> {
    let Some(oauth) = &server.oauth else {
        return Vec::new();
    };

    vec![
        format!(
            "provider: {}",
            oauth.provider.as_deref().unwrap_or("(none)")
        ),
        format!("client_id: {}", oauth.client_id),
        format!("authorize_url: {}", oauth.authorize_url),
        format!("token_url: {}", oauth.token_url),
        format!("redirect_url: {}", oauth.redirect_url),
        format!(
            "scopes: {}",
            if oauth.scopes.is_empty() {
                "(none)".to_string()
            } else {
                oauth.scopes.join(", ")
            }
        ),
        format!(
            "login_hint: {}",
            oauth.login_hint.as_deref().unwrap_or("(none)")
        ),
        format!(
            "account_id: {}",
            oauth.account_id.as_deref().unwrap_or("(none)")
        ),
    ]
}

fn mcp_list_cli_palette() -> Vec<String> {
    vec![
        "- open panel: `hellox mcp panel <server-name>`".to_string(),
        "- add stdio: `hellox mcp add-stdio <name> --command <cmd> --arg <arg>`".to_string(),
        "- add sse: `hellox mcp add-sse <name> --url <url>`".to_string(),
        "- add ws: `hellox mcp add-ws <name> --url <url>`".to_string(),
        "- browse registry: `hellox mcp registry-list`".to_string(),
    ]
}

fn mcp_list_repl_palette() -> Vec<String> {
    vec![
        "- open panel: `/mcp panel [server-name]`".to_string(),
        "- show detail: `/mcp show <server-name>`".to_string(),
        "- list tools: `/mcp tools <server-name>`".to_string(),
        "- browse registry: `/mcp registry list [cursor] [limit]`".to_string(),
    ]
}

fn mcp_detail_cli_palette(server_name: &str) -> Vec<String> {
    vec![
        "- back to list: `hellox mcp panel`".to_string(),
        format!("- tools: `hellox mcp tools {server_name}`"),
        format!("- resources: `hellox mcp resources {server_name}`"),
        format!("- prompts: `hellox mcp prompts {server_name}`"),
        format!("- auth: `hellox mcp auth-show {server_name}`"),
        format!("- disable: `hellox mcp disable {server_name}`"),
        format!("- remove: `hellox mcp remove {server_name}`"),
    ]
}

fn mcp_detail_repl_palette(server_name: &str) -> Vec<String> {
    vec![
        "- back to list: `/mcp panel`".to_string(),
        format!("- tools: `/mcp tools {server_name}`"),
        format!("- resources: `/mcp resources {server_name}`"),
        format!("- prompts: `/mcp prompts {server_name}`"),
        format!("- auth: `/mcp auth show {server_name}`"),
        format!("- disable: `/mcp disable {server_name}`"),
        format!("- remove: `/mcp remove {server_name}`"),
    ]
}

fn preview_text(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let head = compact
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        format!("{head}...")
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "YES".to_string()
    } else {
        "NO".to_string()
    }
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
