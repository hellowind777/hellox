use anyhow::Result;
use hellox_auth::AuthStore;
use hellox_config::HelloxConfig;
use hellox_server::{
    DirectConnectConfig, DirectConnectRequest, ServerSessionDetail, ServerSessionSummary,
};

use crate::environment::{resolve_remote_environment, ResolvedRemoteEnvironment};
use crate::transport::{HttpRemoteSessionTransport, RemoteSessionTransport};

pub fn create_remote_direct_connect(
    config: &HelloxConfig,
    auth_store: &AuthStore,
    environment_name: &str,
    request: DirectConnectRequest,
) -> Result<DirectConnectConfig> {
    remote_session_transport(config, auth_store, environment_name)?
        .create_direct_connect_session(request)
}

pub fn list_remote_sessions(
    config: &HelloxConfig,
    auth_store: &AuthStore,
    environment_name: &str,
) -> Result<Vec<ServerSessionSummary>> {
    remote_session_transport(config, auth_store, environment_name)?.list_sessions()
}

pub fn load_remote_session(
    config: &HelloxConfig,
    auth_store: &AuthStore,
    environment_name: &str,
    session_id: &str,
) -> Result<ServerSessionDetail> {
    remote_session_transport(config, auth_store, environment_name)?.load_session_detail(session_id)
}

pub fn remote_environment_access(
    config: &HelloxConfig,
    auth_store: &AuthStore,
    environment_name: &str,
) -> Result<ResolvedRemoteEnvironment> {
    resolve_remote_environment(config, auth_store, environment_name)
}

pub fn remote_session_transport(
    config: &HelloxConfig,
    auth_store: &AuthStore,
    environment_name: &str,
) -> Result<HttpRemoteSessionTransport> {
    Ok(HttpRemoteSessionTransport::new(resolve_remote_environment(
        config,
        auth_store,
        environment_name,
    )?))
}
