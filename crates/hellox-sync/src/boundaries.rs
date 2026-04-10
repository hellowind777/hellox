use std::path::PathBuf;

use anyhow::Result;
use hellox_config::config_root;

use crate::local::{
    export_settings_snapshot, import_settings_snapshot, load_team_memory_snapshot_from,
    merge_team_memory_snapshot_in, put_team_memory_entry_in, remove_team_memory_entry_in,
    team_memory_snapshot_path_in,
};
use crate::remote::{ManagedSettingsDocument, PolicyLimitsDocument, RemoteFetch, RemoteSyncClient};
use crate::{SettingsSyncSnapshot, TeamMemorySnapshot};

pub trait RemoteSyncTransport {
    fn push_settings(&self, snapshot: &SettingsSyncSnapshot) -> Result<SettingsSyncSnapshot>;
    fn pull_settings(&self) -> Result<Option<SettingsSyncSnapshot>>;
    fn sync_team_memory(
        &self,
        repo_id: &str,
        snapshot: &TeamMemorySnapshot,
    ) -> Result<TeamMemorySnapshot>;
    fn fetch_managed_settings_document(
        &self,
        etag: Option<&str>,
    ) -> Result<RemoteFetch<ManagedSettingsDocument>>;
    fn fetch_policy_limits_document(
        &self,
        etag: Option<&str>,
    ) -> Result<RemoteFetch<PolicyLimitsDocument>>;
}

impl RemoteSyncTransport for RemoteSyncClient {
    fn push_settings(&self, snapshot: &SettingsSyncSnapshot) -> Result<SettingsSyncSnapshot> {
        self.push_settings_snapshot(snapshot)
    }

    fn pull_settings(&self) -> Result<Option<SettingsSyncSnapshot>> {
        self.pull_settings_snapshot()
    }

    fn sync_team_memory(
        &self,
        repo_id: &str,
        snapshot: &TeamMemorySnapshot,
    ) -> Result<TeamMemorySnapshot> {
        self.sync_team_memory_snapshot(repo_id, snapshot)
    }

    fn fetch_managed_settings_document(
        &self,
        etag: Option<&str>,
    ) -> Result<RemoteFetch<ManagedSettingsDocument>> {
        self.fetch_managed_settings(etag)
    }

    fn fetch_policy_limits_document(
        &self,
        etag: Option<&str>,
    ) -> Result<RemoteFetch<PolicyLimitsDocument>> {
        self.fetch_policy_limits(etag)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSyncStore {
    root: PathBuf,
}

impl Default for LocalSyncStore {
    fn default() -> Self {
        Self::new(config_root())
    }
}

impl LocalSyncStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn export_settings_snapshot(
        &self,
        config_path: Option<PathBuf>,
    ) -> Result<SettingsSyncSnapshot> {
        export_settings_snapshot(config_path)
    }

    pub fn import_settings_snapshot(
        &self,
        config_path: Option<PathBuf>,
        snapshot: &SettingsSyncSnapshot,
    ) -> Result<PathBuf> {
        import_settings_snapshot(config_path, snapshot)
    }

    pub fn load_team_memory_snapshot(&self, repo_id: &str) -> Result<TeamMemorySnapshot> {
        load_team_memory_snapshot_from(&self.root, repo_id)
    }

    pub fn merge_team_memory_snapshot(
        &self,
        repo_id: &str,
        snapshot: TeamMemorySnapshot,
    ) -> Result<TeamMemorySnapshot> {
        merge_team_memory_snapshot_in(&self.root, repo_id, snapshot)
    }

    pub fn put_team_memory_entry(
        &self,
        repo_id: &str,
        key: String,
        content: String,
    ) -> Result<TeamMemorySnapshot> {
        put_team_memory_entry_in(&self.root, repo_id, key, content)
    }

    pub fn remove_team_memory_entry(&self, repo_id: &str, key: &str) -> Result<TeamMemorySnapshot> {
        remove_team_memory_entry_in(&self.root, repo_id, key)
    }

    pub fn team_memory_snapshot_path(&self, repo_id: &str) -> PathBuf {
        team_memory_snapshot_path_in(&self.root, repo_id)
    }
}
