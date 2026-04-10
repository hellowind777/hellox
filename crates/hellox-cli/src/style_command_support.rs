use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use hellox_config::default_config_path;

pub(crate) fn resolve_config_path(value: Option<PathBuf>) -> PathBuf {
    value.unwrap_or_else(default_config_path)
}

pub(crate) fn workspace_root(value: Option<PathBuf>) -> Result<PathBuf> {
    Ok(match value {
        Some(path) => path,
        None => env::current_dir()?,
    })
}

pub(crate) fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
