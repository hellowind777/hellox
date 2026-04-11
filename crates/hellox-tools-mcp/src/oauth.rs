use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use hellox_auth::{
    exchange_oauth_authorization_code, refresh_oauth_access_token, start_oauth_authorization,
    store_oauth_account, AuthAccount, LocalAuthStoreBackend, OAuthAuthorizationRequest,
    OAuthClientConfig,
};
use hellox_config::{McpServerConfig, McpTransportConfig};
use reqwest::Url;

const OAUTH_REFRESH_SKEW_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOAuthStatus {
    pub configured: bool,
    pub provider: Option<String>,
    pub account_id: Option<String>,
    pub resource: Option<String>,
    pub linked: bool,
    pub expires_at: Option<u64>,
    pub has_refresh_token: bool,
}

pub fn oauth_status(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<McpOAuthStatus> {
    let Some(config) = server.oauth.as_ref() else {
        return Ok(McpOAuthStatus {
            configured: false,
            provider: None,
            account_id: None,
            resource: None,
            linked: false,
            expires_at: None,
            has_refresh_token: false,
        });
    };

    let account_id = account_id_for_server(server_name, server);
    let store = backend.load_auth_store()?;
    let account = store.accounts.get(&account_id);

    Ok(McpOAuthStatus {
        configured: true,
        provider: Some(provider_for_server(server_name, config)),
        account_id: Some(account_id),
        resource: Some(resource_for_server(server)?),
        linked: account.is_some(),
        expires_at: account.and_then(|item| item.expires_at),
        has_refresh_token: account
            .and_then(|item| item.refresh_token.as_ref())
            .is_some(),
    })
}

pub fn start_server_oauth_authorization(
    server_name: &str,
    server: &McpServerConfig,
) -> Result<OAuthAuthorizationRequest> {
    let config = resolve_oauth_client_config(server_name, server)?;
    start_oauth_authorization(&config)
}

pub fn exchange_server_oauth_authorization_code(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
    code: &str,
    code_verifier: &str,
) -> Result<AuthAccount> {
    let config = resolve_oauth_client_config(server_name, server)?;
    let tokens = exchange_oauth_authorization_code(&config, code, code_verifier)?;
    let account_id = account_id_for_server(server_name, server);
    let mut store = backend.load_auth_store()?;
    store_oauth_account(&mut store, account_id.clone(), &config, &tokens);
    backend.save_auth_store(&store)?;
    store
        .accounts
        .get(&account_id)
        .cloned()
        .ok_or_else(|| anyhow!("Stored MCP OAuth account `{account_id}` was not found"))
}

pub fn refresh_server_oauth_access_token(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<AuthAccount> {
    let config = resolve_oauth_client_config(server_name, server)?;
    let account_id = account_id_for_server(server_name, server);
    let mut store = backend.load_auth_store()?;
    let account = store
        .accounts
        .get(&account_id)
        .cloned()
        .ok_or_else(|| anyhow!("MCP OAuth account `{account_id}` was not found"))?;
    let refresh_token = account
        .refresh_token
        .ok_or_else(|| anyhow!("MCP OAuth account `{account_id}` does not have a refresh token"))?;
    let tokens = refresh_oauth_access_token(&config, &refresh_token)?;
    store_oauth_account(&mut store, account_id.clone(), &config, &tokens);
    backend.save_auth_store(&store)?;
    store
        .accounts
        .get(&account_id)
        .cloned()
        .ok_or_else(|| anyhow!("Stored MCP OAuth account `{account_id}` was not found"))
}

pub fn clear_server_oauth_account(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<bool> {
    let account_id = account_id_for_server(server_name, server);
    let mut store = backend.load_auth_store()?;
    let removed = store.accounts.remove(&account_id).is_some();
    if removed {
        backend.save_auth_store(&store)?;
    }
    Ok(removed)
}

pub fn resolve_server_oauth_access_token(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<Option<String>> {
    if server.oauth.is_none() {
        return Ok(None);
    }

    let account_id = account_id_for_server(server_name, server);
    let store = backend.load_auth_store()?;
    let Some(account) = store.accounts.get(&account_id).cloned() else {
        return Ok(None);
    };

    if needs_refresh(&account) {
        let refreshed = refresh_server_oauth_access_token(backend, server_name, server)?;
        return Ok(Some(refreshed.access_token));
    }

    Ok(Some(account.access_token))
}

pub fn resolve_oauth_client_config(
    server_name: &str,
    server: &McpServerConfig,
) -> Result<OAuthClientConfig> {
    let oauth = server
        .oauth
        .as_ref()
        .ok_or_else(|| anyhow!("MCP server `{server_name}` does not have OAuth configured"))?;
    if matches!(server.transport, McpTransportConfig::Stdio { .. }) {
        return Err(anyhow!(
            "MCP OAuth is only supported for HTTP/SSE or WebSocket servers."
        ));
    }

    Ok(OAuthClientConfig {
        provider: provider_for_server(server_name, oauth),
        client_id: oauth.client_id.clone(),
        authorize_url: oauth.authorize_url.clone(),
        token_url: oauth.token_url.clone(),
        redirect_url: oauth.redirect_url.clone(),
        resource: Some(resource_for_server(server)?),
        scopes: oauth.scopes.clone(),
        login_hint: oauth.login_hint.clone(),
    })
}

fn account_id_for_server(server_name: &str, server: &McpServerConfig) -> String {
    server
        .oauth
        .as_ref()
        .and_then(|oauth| oauth.account_id.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("mcp:{}", server_name.trim()))
}

fn provider_for_server(server_name: &str, oauth: &hellox_config::McpOAuthConfig) -> String {
    oauth
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("mcp:{}", server_name.trim()))
}

fn resource_for_server(server: &McpServerConfig) -> Result<String> {
    match &server.transport {
        McpTransportConfig::Sse { url, .. } | McpTransportConfig::Ws { url, .. } => {
            canonical_resource(url)
        }
        McpTransportConfig::Stdio { .. } => Err(anyhow!(
            "MCP OAuth resource URIs are only available for HTTP/SSE or WebSocket servers."
        )),
    }
}

fn canonical_resource(endpoint: &str) -> Result<String> {
    let mut url = Url::parse(endpoint)
        .with_context(|| format!("Failed to parse MCP OAuth resource URL `{endpoint}`."))?;
    match url.scheme() {
        "ws" => {
            url.set_scheme("http")
                .map_err(|_| anyhow!("Failed to normalize MCP OAuth resource URL `{endpoint}`."))?;
        }
        "wss" => {
            url.set_scheme("https")
                .map_err(|_| anyhow!("Failed to normalize MCP OAuth resource URL `{endpoint}`."))?;
        }
        _ => {}
    }
    url.set_query(None);
    url.set_fragment(None);
    if url.path() == "/" {
        url.set_path("");
    }
    Ok(url.to_string())
}

fn needs_refresh(account: &AuthAccount) -> bool {
    account
        .expires_at
        .map(|expires_at| expires_at <= now_timestamp().saturating_add(OAUTH_REFRESH_SKEW_SECS))
        .unwrap_or(false)
}

fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hellox_auth::LocalAuthStoreBackend;
    use hellox_config::{McpOAuthConfig, McpScope};

    use crate::{build_stream_server, StreamTransportKind};

    use super::*;

    #[test]
    fn canonicalizes_ws_resource_to_https() {
        let server = build_stream_server(
            StreamTransportKind::Ws,
            String::from("wss://example.test/mcp?debug=1#fragment"),
            BTreeMap::new(),
            McpScope::User,
            None,
            Some(McpOAuthConfig {
                provider: None,
                client_id: String::from("client-123"),
                authorize_url: String::from("https://auth.example.test/authorize"),
                token_url: String::from("https://auth.example.test/token"),
                redirect_url: String::from("http://127.0.0.1:8910/callback"),
                scopes: vec![String::from("openid")],
                login_hint: None,
                account_id: None,
            }),
        )
        .expect("build stream server");

        let config = resolve_oauth_client_config("docs", &server).expect("resolve oauth config");
        assert_eq!(config.resource.as_deref(), Some("https://example.test/mcp"));
        assert_eq!(config.provider, "mcp:docs");
    }

    #[test]
    fn oauth_status_defaults_to_mcp_server_account() {
        let backend = LocalAuthStoreBackend::default();
        let server = build_stream_server(
            StreamTransportKind::Sse,
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
                scopes: vec![String::from("openid")],
                login_hint: None,
                account_id: None,
            }),
        )
        .expect("build stream server");

        let status = oauth_status(&backend, "docs", &server).expect("oauth status");
        assert!(status.configured);
        assert_eq!(status.account_id.as_deref(), Some("mcp:docs"));
        assert!(!status.linked);
    }
}
