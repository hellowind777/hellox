use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hellox_config::config_root;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetachedAgentJob {
    pub session_id: String,
    pub session_path: String,
    pub prompt: String,
    pub max_turns: usize,
    #[serde(default)]
    pub config_path: Option<String>,
}

impl DetachedAgentJob {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read detached agent job {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse detached agent job {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create detached agent job dir {}",
                    parent.display()
                )
            })?;
        }

        let raw = serde_json::to_string_pretty(self).context("failed to serialize detached job")?;
        fs::write(path, raw)
            .with_context(|| format!("failed to write detached agent job {}", path.display()))
    }
}

pub fn detached_job_path(session_id: &str) -> PathBuf {
    let launch_id = Uuid::new_v4().simple().to_string();
    config_root()
        .join("worker-jobs")
        .join(format!("{session_id}-{launch_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::detached_job_path;

    #[test]
    fn detached_job_path_is_unique_per_launch() {
        let first = detached_job_path("session-123");
        let second = detached_job_path("session-123");

        assert_ne!(first, second);
        assert!(
            first
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("session-123-")),
            "{}",
            first.display()
        );
        assert_eq!(first.extension().and_then(|ext| ext.to_str()), Some("json"));
    }
}
