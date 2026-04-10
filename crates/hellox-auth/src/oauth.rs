use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest::{blocking::Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{unix_timestamp, upsert_auth_account, AuthAccount, AuthStore};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthClientConfig {
    pub provider: String,
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub redirect_url: String,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub login_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthAuthorizationRequest {
    pub authorization_url: String,
    pub code_verifier: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub id_token: Option<String>,
}

/// Start a generic OAuth 2.0 authorization-code flow with PKCE and return the browser URL.
pub fn start_oauth_authorization(config: &OAuthClientConfig) -> Result<OAuthAuthorizationRequest> {
    let code_verifier = generate_code_verifier();
    let state = generate_state();
    let mut url =
        Url::parse(&config.authorize_url).context("failed to parse OAuth authorize URL")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", &config.client_id);
        query.append_pair("redirect_uri", &config.redirect_url);
        query.append_pair("code_challenge", &generate_code_challenge(&code_verifier));
        query.append_pair("code_challenge_method", "S256");
        query.append_pair("state", &state);
        if let Some(resource) = config.resource.as_deref() {
            query.append_pair("resource", resource);
        }
        if !config.scopes.is_empty() {
            query.append_pair("scope", &config.scopes.join(" "));
        }
        if let Some(login_hint) = config.login_hint.as_deref() {
            query.append_pair("login_hint", login_hint);
        }
    }

    Ok(OAuthAuthorizationRequest {
        authorization_url: url.to_string(),
        code_verifier,
        state,
    })
}

pub fn exchange_oauth_authorization_code(
    config: &OAuthClientConfig,
    code: &str,
    code_verifier: &str,
) -> Result<OAuthTokenResponse> {
    let mut params = vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("client_id".to_string(), config.client_id.clone()),
        ("code".to_string(), code.trim().to_string()),
        ("redirect_uri".to_string(), config.redirect_url.clone()),
        (
            "code_verifier".to_string(),
            code_verifier.trim().to_string(),
        ),
    ];
    append_resource_param(&mut params, config);
    request_token(&config.token_url, params)
}

pub fn refresh_oauth_access_token(
    config: &OAuthClientConfig,
    refresh_token: &str,
) -> Result<OAuthTokenResponse> {
    let mut params = vec![
        ("grant_type".to_string(), "refresh_token".to_string()),
        ("client_id".to_string(), config.client_id.clone()),
        (
            "refresh_token".to_string(),
            refresh_token.trim().to_string(),
        ),
    ];
    append_resource_param(&mut params, config);
    request_token(&config.token_url, params)
}

pub fn store_oauth_account(
    store: &mut AuthStore,
    account_id: String,
    config: &OAuthClientConfig,
    response: &OAuthTokenResponse,
) {
    let scopes = if response.scopes.is_empty() {
        sanitize_scopes(config.scopes.clone())
    } else {
        sanitize_scopes(response.scopes.clone())
    };

    upsert_auth_account(
        store,
        AuthAccount {
            account_id,
            provider: config.provider.clone(),
            access_token: response.access_token.clone(),
            refresh_token: sanitize_optional(response.refresh_token.clone()),
            scopes,
            updated_at: unix_timestamp(),
            expires_at: response
                .expires_in
                .map(|seconds| unix_timestamp().saturating_add(seconds)),
        },
    );
}

pub fn generate_code_verifier() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn generate_code_challenge(code_verifier: &str) -> String {
    let digest = Sha256::digest(code_verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub fn generate_state() -> String {
    Uuid::new_v4().simple().to_string()
}

fn request_token(token_url: &str, form: Vec<(String, String)>) -> Result<OAuthTokenResponse> {
    let response = Client::new()
        .post(token_url)
        .form(&form)
        .send()
        .with_context(|| format!("failed to send OAuth request to {token_url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .unwrap_or_else(|_| "unable to read response body".to_string());
        return Err(anyhow!(
            "OAuth token request failed with status {}: {}",
            status,
            body.trim()
        ));
    }

    let raw: RawOAuthTokenResponse = response
        .json()
        .with_context(|| format!("failed to parse OAuth token response from {token_url}"))?;
    Ok(raw.into_response())
}

fn append_resource_param(params: &mut Vec<(String, String)>, config: &OAuthClientConfig) {
    if let Some(resource) = config.resource.as_deref() {
        params.push(("resource".to_string(), resource.to_string()));
    }
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn sanitize_scopes(scopes: Vec<String>) -> Vec<String> {
    scopes
        .into_iter()
        .flat_map(|scope| {
            scope
                .split([' ', ','])
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .map(|scope| scope.trim().to_string())
        .filter(|scope| !scope.is_empty())
        .collect()
}

#[derive(Debug, Deserialize)]
struct RawOAuthTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    id_token: Option<String>,
}

impl RawOAuthTokenResponse {
    fn into_response(self) -> OAuthTokenResponse {
        OAuthTokenResponse {
            access_token: self.access_token,
            refresh_token: sanitize_optional(self.refresh_token),
            token_type: sanitize_optional(self.token_type),
            scopes: self
                .scope
                .map(|value| sanitize_scopes(vec![value]))
                .unwrap_or_default(),
            expires_in: self.expires_in,
            id_token: sanitize_optional(self.id_token),
        }
    }
}
