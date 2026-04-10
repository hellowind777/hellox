use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RemoteConfig {
    #[serde(default)]
    pub environments: BTreeMap<String, RemoteEnvironmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteEnvironmentConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub server_url: String,
    #[serde(default)]
    pub token_env: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_enabled() -> bool {
    true
}
