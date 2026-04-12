use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LspConfig {
    #[serde(default)]
    pub servers: BTreeMap<String, LspServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LspServerConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub language_id: Option<String>,
    #[serde(default)]
    pub file_extensions: Vec<String>,
    #[serde(default)]
    pub root_markers: Vec<String>,
}

impl LspServerConfig {
    pub fn matches_extension(&self, extension: &str) -> bool {
        self.file_extensions
            .iter()
            .map(|value| value.trim_start_matches('.'))
            .any(|value| value.eq_ignore_ascii_case(extension))
    }
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::LspServerConfig;

    #[test]
    fn server_matches_extension_with_or_without_dot() {
        let server = LspServerConfig {
            enabled: true,
            description: None,
            command: "rust-analyzer".to_string(),
            args: Vec::new(),
            env: Default::default(),
            cwd: None,
            language_id: Some("rust".to_string()),
            file_extensions: vec![".rs".to_string(), "ron".to_string()],
            root_markers: Vec::new(),
        };

        assert!(server.matches_extension("rs"));
        assert!(server.matches_extension("ron"));
        assert!(!server.matches_extension("py"));
    }
}
