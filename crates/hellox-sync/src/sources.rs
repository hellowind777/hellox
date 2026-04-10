use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

use crate::remote::{
    fetch_cached_document, managed_settings_cache_path, persist_cached_document,
    policy_limits_cache_path, CachedRemoteDocument, ManagedSettingsDocument, PolicyLimitsDocument,
    RemoteDocument,
};

/// Loads managed settings from a local-first source.
pub trait ManagedSettingsSource {
    fn load_managed_settings(&self) -> Result<Option<ManagedSettingsDocument>>;
}

/// Loads policy limits from a local-first source.
pub trait PolicyLimitsSource {
    fn load_policy_limits(&self) -> Result<Option<PolicyLimitsDocument>>;
}

/// Represents a local deployment with no managed settings document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmptyManagedSettingsSource;

impl ManagedSettingsSource for EmptyManagedSettingsSource {
    fn load_managed_settings(&self) -> Result<Option<ManagedSettingsDocument>> {
        Ok(None)
    }
}

/// Represents a local deployment with no policy limits document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmptyPolicyLimitsSource;

impl PolicyLimitsSource for EmptyPolicyLimitsSource {
    fn load_policy_limits(&self) -> Result<Option<PolicyLimitsDocument>> {
        Ok(None)
    }
}

/// Reads managed settings from a local JSON document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileManagedSettingsSource {
    path: PathBuf,
}

impl FileManagedSettingsSource {
    /// Creates a file-backed managed settings source.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the managed settings document path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ManagedSettingsSource for FileManagedSettingsSource {
    fn load_managed_settings(&self) -> Result<Option<ManagedSettingsDocument>> {
        read_document_if_exists(&self.path)
    }
}

/// Reads policy limits from a local JSON document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePolicyLimitsSource {
    path: PathBuf,
}

impl FilePolicyLimitsSource {
    /// Creates a file-backed policy limits source.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the policy limits document path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl PolicyLimitsSource for FilePolicyLimitsSource {
    fn load_policy_limits(&self) -> Result<Option<PolicyLimitsDocument>> {
        read_document_if_exists(&self.path)
    }
}

/// Reads managed settings from the local remote-document cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedManagedSettingsSource {
    environment_name: String,
    path: PathBuf,
}

impl CachedManagedSettingsSource {
    /// Creates a cache-backed managed settings source for an environment.
    pub fn new(environment_name: impl Into<String>) -> Self {
        let environment_name = environment_name.into();
        Self::with_path(
            environment_name.clone(),
            managed_settings_cache_path(&environment_name),
        )
    }

    /// Creates a cache-backed managed settings source with an explicit cache path.
    pub fn with_path(environment_name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            environment_name: environment_name.into(),
            path: path.into(),
        }
    }

    /// Returns the managed settings cache path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the cached managed settings document together with cache metadata.
    pub fn inspect(&self) -> Result<Option<CachedRemoteDocument<ManagedSettingsDocument>>> {
        fetch_cached_document(&self.path)
    }

    /// Persists the latest managed settings document into the local cache.
    pub fn persist(&self, document: &RemoteDocument<ManagedSettingsDocument>) -> Result<PathBuf> {
        persist_cached_document(&self.path, &self.environment_name, document)
    }
}

impl ManagedSettingsSource for CachedManagedSettingsSource {
    fn load_managed_settings(&self) -> Result<Option<ManagedSettingsDocument>> {
        Ok(self.inspect()?.map(|document| document.value))
    }
}

/// Reads policy limits from the local remote-document cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedPolicyLimitsSource {
    environment_name: String,
    path: PathBuf,
}

impl CachedPolicyLimitsSource {
    /// Creates a cache-backed policy limits source for an environment.
    pub fn new(environment_name: impl Into<String>) -> Self {
        let environment_name = environment_name.into();
        Self::with_path(
            environment_name.clone(),
            policy_limits_cache_path(&environment_name),
        )
    }

    /// Creates a cache-backed policy limits source with an explicit cache path.
    pub fn with_path(environment_name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            environment_name: environment_name.into(),
            path: path.into(),
        }
    }

    /// Returns the policy limits cache path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the cached policy limits document together with cache metadata.
    pub fn inspect(&self) -> Result<Option<CachedRemoteDocument<PolicyLimitsDocument>>> {
        fetch_cached_document(&self.path)
    }

    /// Persists the latest policy limits document into the local cache.
    pub fn persist(&self, document: &RemoteDocument<PolicyLimitsDocument>) -> Result<PathBuf> {
        persist_cached_document(&self.path, &self.environment_name, document)
    }
}

impl PolicyLimitsSource for CachedPolicyLimitsSource {
    fn load_policy_limits(&self) -> Result<Option<PolicyLimitsDocument>> {
        Ok(self.inspect()?.map(|document| document.value))
    }
}

fn read_document_if_exists<T>(path: &Path) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read source {}", path.display()))?;
    let document = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse source {}", path.display()))?;
    Ok(Some(document))
}
