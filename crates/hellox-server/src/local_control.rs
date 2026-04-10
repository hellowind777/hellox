use std::path::PathBuf;

use anyhow::Result;
use hellox_sync::{
    ManagedSettingsDocument, PolicyLimitsDocument, SettingsSyncSnapshot, TeamMemorySnapshot,
};
use tracing::info;

use crate::state;
use crate::types::{
    DirectConnectConfig, DirectConnectRequest, ServerSessionDetail, ServerSessionSummary,
    ServerStatus,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalServerControlPlane {
    config_path: Option<PathBuf>,
}

impl LocalServerControlPlane {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        Self { config_path }
    }

    pub fn config_path(&self) -> Option<&PathBuf> {
        self.config_path.as_ref()
    }

    pub async fn serve(&self) -> Result<()> {
        let state = state::build_state(self.config_path.clone())?;
        let listen = state.config.server.listen.clone();
        let app = crate::http::router(state);
        let listener = tokio::net::TcpListener::bind(&listen).await?;
        info!("hellox server listening on {}", listen);
        axum::serve(listener, app).await?;
        Ok(())
    }

    pub fn inspect_status(&self) -> Result<ServerStatus> {
        let state = state::build_state(self.config_path.clone())?;
        Ok(state::build_server_status(
            &state.config,
            &state.runtime_paths,
        ))
    }

    pub fn create_direct_connect(
        &self,
        request: DirectConnectRequest,
    ) -> Result<DirectConnectConfig> {
        let state = state::build_state(self.config_path.clone())?;
        let snapshot = match request.session_id.as_deref() {
            Some(session_id) => Some(state::read_session_snapshot(
                &state.runtime_paths,
                session_id,
            )?),
            None => None,
        };
        Ok(state::build_direct_connect_config(
            &state.config,
            snapshot.as_ref(),
            request,
        ))
    }

    pub fn inspect_registered_sessions(&self) -> Result<Vec<ServerSessionSummary>> {
        let state = state::build_state(self.config_path.clone())?;
        state::inspect_registered_sessions(&state)
    }

    pub fn inspect_registered_session(&self, session_id: &str) -> Result<ServerSessionDetail> {
        let state = state::build_state(self.config_path.clone())?;
        state::inspect_registered_session(&state, session_id)
    }

    pub fn inspect_managed_settings(&self) -> Result<Option<ManagedSettingsDocument>> {
        let state = state::build_state(self.config_path.clone())?;
        state::inspect_managed_settings(&state)
    }

    pub fn set_managed_settings(
        &self,
        config_toml: String,
        signature: Option<String>,
    ) -> Result<ManagedSettingsDocument> {
        let state = state::build_state(self.config_path.clone())?;
        state::save_managed_settings(&state, config_toml, signature)
    }

    pub fn inspect_policy_limits(&self) -> Result<Option<PolicyLimitsDocument>> {
        let state = state::build_state(self.config_path.clone())?;
        state::inspect_policy_limits(&state)
    }

    pub fn set_policy_limits(
        &self,
        disabled_commands: Vec<String>,
        disabled_tools: Vec<String>,
        notes: Option<String>,
    ) -> Result<PolicyLimitsDocument> {
        let state = state::build_state(self.config_path.clone())?;
        state::save_policy_limits(&state, disabled_commands, disabled_tools, notes)
    }

    pub fn inspect_synced_settings(
        &self,
        account_id: &str,
    ) -> Result<Option<SettingsSyncSnapshot>> {
        let state = state::build_state(self.config_path.clone())?;
        state::inspect_settings_snapshot(&state, account_id)
    }

    pub fn inspect_synced_team_memory(
        &self,
        account_id: &str,
        repo_id: &str,
    ) -> Result<Option<TeamMemorySnapshot>> {
        let state = state::build_state(self.config_path.clone())?;
        state::inspect_team_memory_snapshot(&state, account_id, repo_id)
    }
}
