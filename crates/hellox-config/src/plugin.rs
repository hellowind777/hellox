use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PluginConfig {
    #[serde(default)]
    pub installed: BTreeMap<String, PluginEntryConfig>,
    #[serde(default)]
    pub marketplaces: BTreeMap<String, MarketplaceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginEntryConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub install_path: Option<String>,
    #[serde(default)]
    pub source: PluginSourceConfig,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PluginSourceConfig {
    LocalPath {
        path: String,
    },
    Marketplace {
        marketplace: String,
        package: String,
        #[serde(default)]
        version: Option<String>,
    },
    Builtin {
        name: String,
    },
}

impl Default for PluginSourceConfig {
    fn default() -> Self {
        Self::LocalPath {
            path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_enabled() -> bool {
    true
}
