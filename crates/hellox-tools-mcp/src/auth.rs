use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_auth::{LocalAuthStoreBackend, ProviderKey};
use hellox_config::{McpServerConfig, McpTransportConfig};

use crate::oauth::{oauth_status, resolve_server_oauth_access_token, McpOAuthStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportAuthStatus {
    pub provider_key: String,
    pub stored_bearer_token: Option<String>,
    pub runtime_source: &'static str,
    pub config_authorization_header: bool,
    pub oauth: McpOAuthStatus,
}

pub fn default_auth_backend() -> LocalAuthStoreBackend {
    LocalAuthStoreBackend::default()
}

pub fn auth_backend_for_config_path(config_path: &Path) -> LocalAuthStoreBackend {
    let base_dir = config_path
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    LocalAuthStoreBackend::new(
        Some(base_dir.join("oauth-tokens.json")),
        Some(base_dir.join("provider-keys.json")),
    )
}

pub fn supports_bearer_token(server: &McpServerConfig) -> bool {
    matches!(
        server.transport,
        McpTransportConfig::Sse { .. } | McpTransportConfig::Ws { .. }
    )
}

pub fn load_bearer_token(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
) -> Result<Option<String>> {
    let store = backend.load_auth_store()?;
    Ok(store
        .provider_keys
        .get(&provider_key_name(server_name))
        .map(|key| key.api_key.clone()))
}

pub fn set_bearer_token(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
    bearer_token: String,
) -> Result<()> {
    if !supports_bearer_token(server) {
        return Err(anyhow!(
            "MCP bearer-token helper only supports HTTP/SSE or WebSocket servers configured with `transport = \"sse\"` or `transport = \"ws\"`."
        ));
    }

    let mut store = backend.load_auth_store()?;
    let token = bearer_token.trim();
    if token.is_empty() {
        return Err(anyhow!("Bearer token cannot be empty"));
    }
    hellox_auth::set_provider_key(
        &mut store,
        provider_key_name(server_name),
        token.to_string(),
    );
    backend.save_auth_store(&store)
}

pub fn clear_bearer_token(backend: &LocalAuthStoreBackend, server_name: &str) -> Result<bool> {
    let mut store = backend.load_auth_store()?;
    let removed = store
        .provider_keys
        .remove(&provider_key_name(server_name))
        .is_some();
    if removed {
        backend.save_auth_store(&store)?;
    }
    Ok(removed)
}

pub fn transport_headers_with_auth(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<BTreeMap<String, String>> {
    let mut headers = match &server.transport {
        McpTransportConfig::Sse { headers, .. } | McpTransportConfig::Ws { headers, .. } => {
            headers.clone()
        }
        McpTransportConfig::Stdio { .. } => BTreeMap::new(),
    };

    if !supports_bearer_token(server) || contains_header_key(&headers, "authorization") {
        return Ok(headers);
    }

    if let Some(token) = resolve_server_oauth_access_token(backend, server_name, server)? {
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
        return Ok(headers);
    }

    if let Some(token) = load_bearer_token(backend, server_name)? {
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
    }

    Ok(headers)
}

pub fn format_auth_status(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<String> {
    let status = auth_status(backend, server_name, server)?;
    let mut lines = vec![
        format!("server: {server_name}"),
        format!("transport: {}", server.transport.kind()),
        format!("provider_key: {}", status.provider_key),
        format!(
            "stored_bearer_token: {}",
            status
                .stored_bearer_token
                .as_deref()
                .map(mask_secret)
                .unwrap_or_else(|| "(none)".to_string())
        ),
        format!(
            "config_authorization_header: {}",
            yes_no(status.config_authorization_header)
        ),
        format!("oauth_configured: {}", yes_no(status.oauth.configured)),
        format!(
            "oauth_provider: {}",
            status.oauth.provider.as_deref().unwrap_or("(none)")
        ),
        format!(
            "oauth_account_id: {}",
            status.oauth.account_id.as_deref().unwrap_or("(none)")
        ),
        format!(
            "oauth_resource: {}",
            status.oauth.resource.as_deref().unwrap_or("(none)")
        ),
        format!("oauth_linked_account: {}", yes_no(status.oauth.linked)),
        format!(
            "oauth_refresh_token: {}",
            yes_no(status.oauth.has_refresh_token)
        ),
        format!(
            "oauth_expires_at: {}",
            status
                .oauth
                .expires_at
                .map(|value| value.to_string())
                .unwrap_or_else(|| "(none)".to_string())
        ),
        format!("runtime_authorization: {}", status.runtime_source),
    ];

    if matches!(server.transport, McpTransportConfig::Stdio { .. }) {
        lines.push(
            "note: stdio auth stays server-defined through command arguments or environment variables."
                .to_string(),
        );
    } else if matches!(server.transport, McpTransportConfig::Ws { .. }) {
        lines.push(
            "note: WebSocket transports inject `Authorization` during the handshake using config headers, linked OAuth access tokens, or the stored bearer token helper."
                .to_string(),
        );
    } else {
        lines.push(
            "note: HTTP/SSE transports prefer config headers, then linked OAuth access tokens, then the stored bearer token helper."
                .to_string(),
        );
    }

    Ok(lines.join("\n"))
}

fn auth_status(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<TransportAuthStatus> {
    let provider_key = provider_key_name(server_name);
    let store = backend.load_auth_store()?;
    let stored_token = store
        .provider_keys
        .get(&provider_key)
        .map(provider_key_secret);
    let config_header = match &server.transport {
        McpTransportConfig::Sse { headers, .. } | McpTransportConfig::Ws { headers, .. } => {
            contains_header_key(headers, "authorization")
        }
        McpTransportConfig::Stdio { .. } => false,
    };
    let oauth = oauth_status(backend, server_name, server)?;
    let runtime_source = match &server.transport {
        McpTransportConfig::Stdio { .. } => "stdio_env_or_command",
        McpTransportConfig::Sse { .. } | McpTransportConfig::Ws { .. } => {
            if config_header {
                "config_authorization_header"
            } else if oauth.linked {
                "linked_oauth_account"
            } else if stored_token.is_some() {
                "stored_bearer_token"
            } else {
                "none"
            }
        }
    };

    Ok(TransportAuthStatus {
        provider_key,
        stored_bearer_token: stored_token,
        runtime_source,
        config_authorization_header: config_header,
        oauth,
    })
}

fn provider_key_name(server_name: &str) -> String {
    format!("mcp:{}", server_name.trim())
}

fn provider_key_secret(key: &ProviderKey) -> String {
    key.api_key.clone()
}

fn contains_header_key(headers: &BTreeMap<String, String>, key: &str) -> bool {
    headers.keys().any(|item| item.eq_ignore_ascii_case(key))
}

fn mask_secret(value: &str) -> String {
    if value.len() <= 8 {
        "********".to_string()
    } else {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_auth::{login_account, LocalAuthStoreBackend};
    use hellox_config::{McpOAuthConfig, McpScope, McpServerConfig, McpTransportConfig};

    use crate::config::build_stream_server;

    use super::*;

    fn temp_backend() -> LocalAuthStoreBackend {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-mcp-auth-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        LocalAuthStoreBackend::new(
            Some(PathBuf::from(root.join("oauth-tokens.json"))),
            Some(PathBuf::from(root.join("provider-keys.json"))),
        )
    }

    #[test]
    fn stores_bearer_tokens_for_http_servers() {
        let backend = temp_backend();
        let server = build_stream_server(
            crate::StreamTransportKind::Sse,
            String::from("https://example.test/mcp"),
            BTreeMap::new(),
            McpScope::User,
            None,
            None,
        )
        .expect("build stream server");

        set_bearer_token(&backend, "docs", &server, String::from("token-123"))
            .expect("set bearer token");

        let headers = transport_headers_with_auth(&backend, "docs", &server)
            .expect("resolve transport headers");
        assert_eq!(
            headers.get("Authorization"),
            Some(&String::from("Bearer token-123"))
        );
    }

    #[test]
    fn rejects_stdio_bearer_tokens() {
        let backend = temp_backend();
        let server = McpServerConfig {
            enabled: true,
            description: None,
            scope: McpScope::User,
            oauth: None,
            transport: McpTransportConfig::Stdio {
                command: String::from("npx"),
                args: vec![String::from("server")],
                env: BTreeMap::new(),
                cwd: None,
            },
        };

        let error = set_bearer_token(&backend, "filesystem", &server, String::from("token"))
            .expect_err("stdio transport must reject bearer token helper");
        assert!(error
            .to_string()
            .contains("only supports HTTP/SSE or WebSocket servers"));
    }

    #[test]
    fn stores_bearer_tokens_for_ws_servers() {
        let backend = temp_backend();
        let server = build_stream_server(
            crate::StreamTransportKind::Ws,
            String::from("ws://127.0.0.1:7777/mcp"),
            BTreeMap::new(),
            McpScope::User,
            None,
            None,
        )
        .expect("build stream server");

        set_bearer_token(&backend, "docs-ws", &server, String::from("token-ws"))
            .expect("set ws bearer token");

        let headers = transport_headers_with_auth(&backend, "docs-ws", &server)
            .expect("resolve ws transport headers");
        assert_eq!(
            headers.get("Authorization"),
            Some(&String::from("Bearer token-ws"))
        );
    }

    #[test]
    fn prefers_oauth_accounts_over_stored_bearer_tokens() {
        let backend = temp_backend();
        let server = build_stream_server(
            crate::StreamTransportKind::Sse,
            String::from("https://example.test/mcp"),
            BTreeMap::new(),
            McpScope::User,
            None,
            Some(McpOAuthConfig {
                provider: Some(String::from("docs-oauth")),
                client_id: String::from("client-123"),
                authorize_url: String::from("https://auth.example.test/authorize"),
                token_url: String::from("https://auth.example.test/token"),
                redirect_url: String::from("http://127.0.0.1:8910/callback"),
                scopes: vec![String::from("openid"), String::from("mcp:docs")],
                login_hint: None,
                account_id: Some(String::from("mcp:docs")),
            }),
        )
        .expect("build stream server");

        set_bearer_token(&backend, "docs", &server, String::from("bearer-token"))
            .expect("set bearer token");
        let mut store = backend.load_auth_store().expect("load auth store");
        login_account(
            &mut store,
            String::from("mcp:docs"),
            String::from("docs-oauth"),
            String::from("oauth-access-token"),
            Some(String::from("refresh-token")),
            vec![String::from("openid")],
        );
        backend.save_auth_store(&store).expect("save auth store");

        let headers = transport_headers_with_auth(&backend, "docs", &server)
            .expect("resolve transport headers");
        assert_eq!(
            headers.get("Authorization"),
            Some(&String::from("Bearer oauth-access-token"))
        );
    }
}
