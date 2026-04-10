mod backend;
mod oauth;
mod store;

#[cfg(test)]
mod tests;

use std::time::{SystemTime, UNIX_EPOCH};

pub use backend::{AuthStoreBackend, LocalAuthStoreBackend};
pub use oauth::{
    exchange_oauth_authorization_code, generate_code_challenge, generate_code_verifier,
    generate_state, refresh_oauth_access_token, start_oauth_authorization, store_oauth_account,
    OAuthAuthorizationRequest, OAuthClientConfig, OAuthTokenResponse,
};
pub use store::{
    auth_store_path, find_account_by_access_token, find_auth_account, find_device_by_token,
    find_trusted_device, format_account_detail, format_account_list, format_auth_summary,
    format_device_detail, format_device_list, format_provider_key_list, load_auth_store,
    login_account, logout_account, mark_device_validated, provider_keys_path, remove_provider_key,
    revoke_device, save_auth_store, set_provider_key, trust_device, trusted_devices_path,
    upsert_auth_account, validate_remote_identity, AuthAccount, AuthStore, ProviderKey,
    RemoteIdentity, TrustedDevice,
};

pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
