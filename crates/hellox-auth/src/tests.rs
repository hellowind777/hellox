use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::Form;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::{
    exchange_oauth_authorization_code, find_auth_account, format_account_detail,
    format_account_list, format_auth_summary, format_device_detail, format_device_list,
    format_provider_key_list, login_account, logout_account, refresh_oauth_access_token,
    remove_provider_key, revoke_device, set_provider_key, start_oauth_authorization,
    store_oauth_account, trust_device, AuthStore, LocalAuthStoreBackend, OAuthClientConfig,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-auth-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

#[test]
fn login_list_logout_and_keys_roundtrip() {
    let root = temp_dir();
    let store_path = root.join("oauth-tokens.json");
    let keys_path = root.join("provider-keys.json");
    let backend = LocalAuthStoreBackend::new(Some(store_path.clone()), Some(keys_path.clone()));
    let mut store = AuthStore::default();

    login_account(
        &mut store,
        "account-1".to_string(),
        "hellox-cloud".to_string(),
        "access-token-123456".to_string(),
        Some("refresh-token-123456".to_string()),
        vec!["user:profile".to_string(), "user:inference".to_string()],
    );
    set_provider_key(
        &mut store,
        "openai".to_string(),
        "sk-1234567890".to_string(),
    );
    let device = trust_device(
        &mut store,
        "account-1",
        "Workstation".to_string(),
        vec!["remote:sessions".to_string()],
    )
    .expect("trust device");
    backend.save_auth_store(&store).expect("save store");

    let loaded = backend.load_auth_store().expect("load store");
    assert!(format_auth_summary(&loaded).contains("accounts: 1"));
    assert!(format_account_list(&loaded).contains("account-1"));
    assert!(
        format_account_detail(loaded.accounts.get("account-1").expect("account-1"))
            .contains("provider: hellox-cloud")
    );
    assert!(format_provider_key_list(&loaded).contains("openai"));
    assert!(format_device_list(&loaded).contains("Workstation"));
    assert!(format_device_detail(
        loaded
            .trusted_devices
            .get(&device.device_id)
            .expect("device")
    )
    .contains("account_id: account-1"));

    let mut mutable = loaded;
    logout_account(&mut mutable, "account-1").expect("logout account");
    remove_provider_key(&mut mutable, "openai").expect("remove provider key");
    revoke_device(&mut mutable, &device.device_id).expect("revoke device");
    assert!(mutable.accounts.is_empty());
    assert!(mutable.provider_keys.is_empty());
    assert!(mutable.trusted_devices.is_empty());
}

#[test]
fn start_oauth_authorization_includes_pkce_parameters() {
    let request = start_oauth_authorization(&OAuthClientConfig {
        provider: "hellox-cloud".to_string(),
        client_id: "client-123".to_string(),
        authorize_url: "https://auth.example.test/authorize".to_string(),
        token_url: "https://auth.example.test/token".to_string(),
        redirect_url: "http://127.0.0.1:8910/callback".to_string(),
        resource: Some("https://api.example.test/mcp".to_string()),
        scopes: vec!["user:profile".to_string(), "remote:sessions".to_string()],
        login_hint: Some("hello@example.test".to_string()),
    })
    .expect("start oauth");

    assert!(request.authorization_url.contains("code_challenge="));
    assert!(request.authorization_url.contains("client_id=client-123"));
    assert!(request
        .authorization_url
        .contains("resource=https%3A%2F%2Fapi.example.test%2Fmcp"));
    assert!(request
        .authorization_url
        .contains("login_hint=hello%40example.test"));
    assert!(request.code_verifier.len() > 30);
    assert!(!request.state.is_empty());
}

#[tokio::test]
async fn oauth_exchange_and_refresh_store_tokens() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test oauth server");
    let address = listener.local_addr().expect("oauth server addr");
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/token", post(oauth_token_handler)),
        )
        .await
        .expect("serve oauth test app");
    });

    let config = OAuthClientConfig {
        provider: "hellox-cloud".to_string(),
        client_id: "client-123".to_string(),
        authorize_url: format!("http://{address}/authorize"),
        token_url: format!("http://{address}/token"),
        redirect_url: "http://127.0.0.1:8910/callback".to_string(),
        resource: Some("http://127.0.0.1:7821/mcp".to_string()),
        scopes: vec!["user:profile".to_string()],
        login_hint: None,
    };

    let exchanged = tokio::task::spawn_blocking({
        let config = config.clone();
        move || {
            exchange_oauth_authorization_code(&config, "auth-code-123", "verifier-123")
                .expect("exchange code")
        }
    })
    .await
    .expect("join exchange");
    assert_eq!(exchanged.access_token, "access-token-123");
    assert_eq!(
        exchanged.refresh_token.as_deref(),
        Some("refresh-token-123")
    );
    assert_eq!(exchanged.scopes, vec![String::from("user:profile")]);

    let refreshed = tokio::task::spawn_blocking({
        let config = config.clone();
        move || refresh_oauth_access_token(&config, "refresh-token-123").expect("refresh token")
    })
    .await
    .expect("join refresh");
    assert_eq!(refreshed.access_token, "refreshed-token-456");

    let mut store = AuthStore::default();
    store_oauth_account(&mut store, "account-oauth".to_string(), &config, &refreshed);
    let account = find_auth_account(&store, "account-oauth").expect("stored account");
    assert_eq!(account.provider, "hellox-cloud");
    assert_eq!(account.access_token, "refreshed-token-456");
    assert_eq!(account.refresh_token.as_deref(), Some("refresh-token-123"));
    assert!(account.expires_at.is_some());
}

#[derive(Debug, Deserialize)]
struct OAuthTokenForm {
    grant_type: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

async fn oauth_token_handler(Form(form): Form<OAuthTokenForm>) -> Json<serde_json::Value> {
    let payload = if form.grant_type == "authorization_code" {
        json!({
            "access_token": "access-token-123",
            "refresh_token": "refresh-token-123",
            "token_type": "Bearer",
            "scope": "user:profile",
            "expires_in": 3600
        })
    } else {
        assert_eq!(form.refresh_token.as_deref(), Some("refresh-token-123"));
        json!({
            "access_token": "refreshed-token-456",
            "refresh_token": "refresh-token-123",
            "token_type": "Bearer",
            "scope": "user:profile",
            "expires_in": 7200
        })
    };
    Json(payload)
}
