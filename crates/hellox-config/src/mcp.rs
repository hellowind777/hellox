use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: BTreeMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub scope: McpScope,
    #[serde(default)]
    pub oauth: Option<McpOAuthConfig>,
    #[serde(flatten)]
    pub transport: McpTransportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpOAuthConfig {
    #[serde(default)]
    pub provider: Option<String>,
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub redirect_url: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub login_hint: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpScope {
    Local,
    #[default]
    User,
    Project,
    Dynamic,
    Enterprise,
    Managed,
    Claudeai,
}

impl McpScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::User => "user",
            Self::Project => "project",
            Self::Dynamic => "dynamic",
            Self::Enterprise => "enterprise",
            Self::Managed => "managed",
            Self::Claudeai => "claudeai",
        }
    }
}

impl fmt::Display for McpScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for McpScope {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "local" => Ok(Self::Local),
            "user" => Ok(Self::User),
            "project" => Ok(Self::Project),
            "dynamic" => Ok(Self::Dynamic),
            "enterprise" => Ok(Self::Enterprise),
            "managed" => Ok(Self::Managed),
            "claudeai" | "claude_ai" => Ok(Self::Claudeai),
            _ => Err("Unsupported MCP scope.".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpTransportConfig {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    Sse {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    Ws {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl McpTransportConfig {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Stdio { .. } => "stdio",
            Self::Sse { .. } => "sse",
            Self::Ws { .. } => "ws",
        }
    }
}

fn default_enabled() -> bool {
    true
}
