use std::path::{Path, PathBuf};

use hellox_agent::CompactMode;
use hellox_config::load_or_default;

use crate::repl::ReplMetadata;
use crate::startup::AppLanguage;
use crate::transcript::default_share_path;

pub(super) fn resolve_share_path(
    value: Option<&str>,
    working_directory: &Path,
    shares_root: &Path,
    session_id: Option<&str>,
) -> PathBuf {
    match value {
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                working_directory.join(path)
            }
        }
        None => default_share_path(shares_root, session_id),
    }
}

pub(super) fn runtime_config(metadata: &ReplMetadata) -> hellox_config::HelloxConfig {
    load_or_default(Some(metadata.config_path.clone())).unwrap_or_else(|_| metadata.config.clone())
}

pub(super) fn format_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

pub(super) fn compact_mode_label(mode: CompactMode, language: AppLanguage) -> &'static str {
    match (mode, language) {
        (CompactMode::Micro, AppLanguage::English) => "microcompact",
        (CompactMode::Full, AppLanguage::English) => "compact",
        (CompactMode::Micro, AppLanguage::SimplifiedChinese) => "微压缩",
        (CompactMode::Full, AppLanguage::SimplifiedChinese) => "压缩",
    }
}
