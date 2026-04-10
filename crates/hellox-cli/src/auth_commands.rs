use anyhow::{anyhow, Result};
use hellox_auth::{
    exchange_oauth_authorization_code, find_auth_account, format_account_detail,
    format_account_list, format_auth_summary, format_device_detail, format_device_list,
    format_provider_key_list, login_account, logout_account, refresh_oauth_access_token,
    remove_provider_key, revoke_device, set_provider_key, start_oauth_authorization,
    store_oauth_account, trust_device, LocalAuthStoreBackend, OAuthClientConfig,
};

use crate::cli_types::AuthCommands;

pub fn handle_auth_command(command: AuthCommands) -> Result<()> {
    let backend = LocalAuthStoreBackend::default();
    let mut store = backend.load_auth_store()?;

    match command {
        AuthCommands::Status => {
            println!("{}", format_auth_summary(&store));
        }
        AuthCommands::Accounts => {
            println!("{}", format_account_list(&store));
        }
        AuthCommands::Devices => {
            println!("{}", format_device_list(&store));
        }
        AuthCommands::Show { account_id } => {
            let account = store
                .accounts
                .get(&account_id)
                .ok_or_else(|| anyhow!("Auth account `{account_id}` was not found"))?;
            println!("{}", format_account_detail(account));
        }
        AuthCommands::Login {
            account_id,
            provider,
            access_token,
            refresh_token,
            scopes,
        } => {
            login_account(
                &mut store,
                account_id.clone(),
                provider,
                access_token,
                refresh_token,
                scopes,
            );
            backend.save_auth_store(&store)?;
            println!("Stored auth account `{account_id}`.");
        }
        AuthCommands::Logout { account_id } => {
            logout_account(&mut store, &account_id)?;
            backend.save_auth_store(&store)?;
            println!("Removed auth account `{account_id}`.");
        }
        AuthCommands::Keys => {
            println!("{}", format_provider_key_list(&store));
        }
        AuthCommands::SetKey { provider, api_key } => {
            set_provider_key(&mut store, provider.clone(), api_key);
            backend.save_auth_store(&store)?;
            println!("Stored provider key `{provider}`.");
        }
        AuthCommands::RemoveKey { provider } => {
            remove_provider_key(&mut store, &provider)?;
            backend.save_auth_store(&store)?;
            println!("Removed provider key `{provider}`.");
        }
        AuthCommands::TrustDevice {
            account_id,
            device_name,
            scopes,
        } => {
            let device = trust_device(&mut store, &account_id, device_name, scopes)?;
            backend.save_auth_store(&store)?;
            println!("{}", format_device_detail(&device));
        }
        AuthCommands::RevokeDevice { device_id } => {
            revoke_device(&mut store, &device_id)?;
            backend.save_auth_store(&store)?;
            println!("Removed trusted device `{device_id}`.");
        }
        AuthCommands::OauthStart {
            account_id,
            provider,
            client_id,
            authorize_url,
            token_url,
            redirect_url,
            scopes,
            login_hint,
        } => {
            let request = start_oauth_authorization(&OAuthClientConfig {
                provider,
                client_id,
                authorize_url,
                token_url,
                redirect_url,
                resource: None,
                scopes,
                login_hint,
            })?;
            println!(
                "account_id: {}\nauthorization_url: {}\ncode_verifier: {}\nstate: {}",
                account_id, request.authorization_url, request.code_verifier, request.state
            );
        }
        AuthCommands::OauthExchange {
            account_id,
            provider,
            client_id,
            token_url,
            redirect_url,
            code,
            code_verifier,
            scopes,
        } => {
            let oauth = OAuthClientConfig {
                provider,
                client_id,
                authorize_url: String::new(),
                token_url,
                redirect_url,
                resource: None,
                scopes,
                login_hint: None,
            };
            let tokens = exchange_oauth_authorization_code(&oauth, &code, &code_verifier)?;
            store_oauth_account(&mut store, account_id.clone(), &oauth, &tokens);
            backend.save_auth_store(&store)?;
            println!(
                "{}",
                format_account_detail(find_auth_account(&store, &account_id)?)
            );
        }
        AuthCommands::OauthRefresh {
            account_id,
            client_id,
            token_url,
            redirect_url,
        } => {
            let account = find_auth_account(&store, &account_id)?.clone();
            let refresh_token = account.refresh_token.clone().ok_or_else(|| {
                anyhow!("Auth account `{account_id}` does not have a refresh token")
            })?;
            let oauth = OAuthClientConfig {
                provider: account.provider.clone(),
                client_id,
                authorize_url: String::new(),
                token_url,
                redirect_url,
                resource: None,
                scopes: account.scopes.clone(),
                login_hint: None,
            };
            let tokens = refresh_oauth_access_token(&oauth, &refresh_token)?;
            store_oauth_account(&mut store, account_id.clone(), &oauth, &tokens);
            backend.save_auth_store(&store)?;
            println!(
                "{}",
                format_account_detail(find_auth_account(&store, &account_id)?)
            );
        }
    }

    Ok(())
}
