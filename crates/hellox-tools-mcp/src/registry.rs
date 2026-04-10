use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use hellox_config::{HelloxConfig, McpScope};
use reqwest::blocking::Client;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{add_server, build_stream_server, StreamTransportKind};

const REGISTRY_BASE_URL: &str = "https://registry.modelcontextprotocol.io/v0.1";
const DEFAULT_REGISTRY_LIMIT: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryServerList {
    pub servers: Vec<RegistryServerEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryServerEntry {
    pub server: RegistryServerRecord,
    #[serde(default, rename = "_meta")]
    pub meta: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryServerRecord {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub repository: Option<Value>,
    #[serde(default, rename = "websiteUrl")]
    pub website_url: Option<String>,
    #[serde(default)]
    pub remotes: Vec<RegistryRemote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryRemote {
    #[serde(rename = "type")]
    pub transport_type: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRegistryInstallResult {
    pub registry_name: String,
    pub installed_server_name: String,
    pub transport: String,
    pub url: String,
    pub version: Option<String>,
}

pub fn list_registry_servers(
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<RegistryServerList> {
    let mut url = Url::parse(&format!("{REGISTRY_BASE_URL}/servers"))
        .context("failed to build MCP registry list URL")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair(
            "limit",
            &limit.unwrap_or(DEFAULT_REGISTRY_LIMIT).max(1).to_string(),
        );
        if let Some(cursor) = cursor.map(str::trim).filter(|value| !value.is_empty()) {
            query.append_pair("cursor", cursor);
        }
    }

    let response = client()
        .get(url.clone())
        .send()
        .with_context(|| format!("failed to fetch MCP registry servers from {url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .unwrap_or_else(|_| "unable to read response body".to_string());
        return Err(anyhow!(
            "MCP registry list request failed with status {}: {}",
            status,
            body.trim()
        ));
    }

    let raw: RawRegistryServerList = response
        .json()
        .context("failed to parse MCP registry list response")?;
    Ok(RegistryServerList {
        servers: raw.servers,
        next_cursor: raw.metadata.next_cursor,
    })
}

pub fn get_registry_server_latest(name: &str) -> Result<RegistryServerEntry> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("MCP registry server name cannot be empty"));
    }

    let mut url = Url::parse(REGISTRY_BASE_URL).context("failed to build MCP registry URL")?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| anyhow!("failed to build MCP registry path"))?;
        segments.push("servers");
        segments.push(trimmed);
        segments.push("versions");
        segments.push("latest");
    }

    let response = client()
        .get(url.clone())
        .send()
        .with_context(|| format!("failed to fetch MCP registry server `{trimmed}` from {url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .unwrap_or_else(|_| "unable to read response body".to_string());
        return Err(anyhow!(
            "MCP registry detail request failed with status {}: {}",
            status,
            body.trim()
        ));
    }

    response
        .json()
        .context("failed to parse MCP registry server detail response")
}

pub fn format_registry_list(result: &RegistryServerList) -> String {
    if result.servers.is_empty() {
        return "registry_servers: (none)".to_string();
    }

    let mut lines = vec!["registry_servers:".to_string()];
    for item in &result.servers {
        let server = &item.server;
        lines.push(format!(
            "- {} | version: {} | remotes: {}{}",
            server.name,
            server.version.as_deref().unwrap_or("(unknown)"),
            format_remotes(&server.remotes),
            server
                .description
                .as_deref()
                .map(|value| format!(" | description: {value}"))
                .unwrap_or_default()
        ));
    }
    lines.push(format!(
        "next_cursor: {}",
        result.next_cursor.as_deref().unwrap_or("(none)")
    ));
    lines.join("\n")
}

pub fn format_registry_detail(entry: &RegistryServerEntry) -> String {
    let server = &entry.server;
    [
        format!("name: {}", server.name),
        format!(
            "description: {}",
            server.description.as_deref().unwrap_or("(none)")
        ),
        format!("version: {}", server.version.as_deref().unwrap_or("(none)")),
        format!(
            "website_url: {}",
            server.website_url.as_deref().unwrap_or("(none)")
        ),
        format!(
            "remotes: {}",
            if server.remotes.is_empty() {
                "(none)".to_string()
            } else {
                server
                    .remotes
                    .iter()
                    .map(|remote| format!("{}={}", remote.transport_type, remote.url))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ),
    ]
    .join("\n")
}

pub fn install_registry_server(
    config: &mut HelloxConfig,
    registry_name: &str,
    server_name_override: Option<&str>,
    scope: McpScope,
) -> Result<McpRegistryInstallResult> {
    let entry = get_registry_server_latest(registry_name)?;
    let remote = select_supported_remote(&entry.server.remotes).ok_or_else(|| {
        anyhow!(
            "MCP registry server `{}` does not expose a supported remote transport (`streamable-http`, `sse`, or `ws`).",
            entry.server.name
        )
    })?;
    let server_name = server_name_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&entry.server.name)
        .to_string();

    let server = build_stream_server(
        remote.kind,
        remote.url.clone(),
        BTreeMap::new(),
        scope,
        entry.server.description.clone(),
        None,
    )?;
    add_server(config, server_name.clone(), server)?;

    Ok(McpRegistryInstallResult {
        registry_name: entry.server.name.clone(),
        installed_server_name: server_name,
        transport: remote.label,
        url: remote.url,
        version: entry.server.version.clone(),
    })
}

fn format_remotes(remotes: &[RegistryRemote]) -> String {
    if remotes.is_empty() {
        return "(none)".to_string();
    }

    remotes
        .iter()
        .map(|remote| remote.transport_type.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn client() -> Client {
    Client::new()
}

fn select_supported_remote(remotes: &[RegistryRemote]) -> Option<SupportedRemote> {
    remotes
        .iter()
        .find_map(|remote| match remote.transport_type.as_str() {
            "streamable-http" | "sse" => Some(SupportedRemote {
                kind: StreamTransportKind::Sse,
                label: remote.transport_type.clone(),
                url: remote.url.clone(),
            }),
            "ws" | "websocket" => Some(SupportedRemote {
                kind: StreamTransportKind::Ws,
                label: remote.transport_type.clone(),
                url: remote.url.clone(),
            }),
            _ => None,
        })
}

struct SupportedRemote {
    kind: StreamTransportKind,
    label: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct RawRegistryServerList {
    #[serde(default)]
    servers: Vec<RegistryServerEntry>,
    #[serde(default)]
    metadata: RawRegistryMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct RawRegistryMetadata {
    #[serde(default, rename = "nextCursor")]
    next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_registry_list_payload() {
        let raw: RawRegistryServerList = serde_json::from_value(json!({
            "servers": [
                {
                    "server": {
                        "name": "ac.tandem/docs-mcp",
                        "description": "Docs MCP",
                        "version": "1.2.3",
                        "websiteUrl": "https://example.test/docs",
                        "remotes": [
                            { "type": "streamable-http", "url": "https://example.test/mcp" }
                        ]
                    },
                    "_meta": { "page": 1 }
                }
            ],
            "metadata": {
                "nextCursor": "cursor-2"
            }
        }))
        .expect("parse registry payload");

        let list = RegistryServerList {
            servers: raw.servers,
            next_cursor: raw.metadata.next_cursor,
        };
        assert_eq!(list.servers.len(), 1);
        assert_eq!(list.next_cursor.as_deref(), Some("cursor-2"));
    }

    #[test]
    fn formats_registry_detail() {
        let detail = format_registry_detail(&RegistryServerEntry {
            server: RegistryServerRecord {
                name: String::from("ac.tandem/docs-mcp"),
                description: Some(String::from("Docs MCP")),
                version: Some(String::from("1.2.3")),
                repository: None,
                website_url: Some(String::from("https://example.test/docs")),
                remotes: vec![RegistryRemote {
                    transport_type: String::from("streamable-http"),
                    url: String::from("https://example.test/mcp"),
                }],
            },
            meta: json!({}),
        });

        assert!(detail.contains("ac.tandem/docs-mcp"));
        assert!(detail.contains("streamable-http=https://example.test/mcp"));
    }

    #[test]
    fn selects_supported_remote_for_install() {
        let remote = select_supported_remote(&[
            RegistryRemote {
                transport_type: String::from("stdio"),
                url: String::from("npx package"),
            },
            RegistryRemote {
                transport_type: String::from("streamable-http"),
                url: String::from("https://example.test/mcp"),
            },
        ])
        .expect("supported remote");

        assert_eq!(remote.kind, StreamTransportKind::Sse);
        assert_eq!(remote.label, "streamable-http");
    }
}
