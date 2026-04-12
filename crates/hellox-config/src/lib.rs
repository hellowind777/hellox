mod lsp;
mod mcp;
mod plugin;
mod remote;
mod scheduler;
mod server;
mod skills;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use hellox_core::{ModelProfile, ProviderCapabilities, ReasoningCompatibility};
pub use lsp::{LspConfig, LspServerConfig};
pub use mcp::{McpConfig, McpOAuthConfig, McpScope, McpServerConfig, McpTransportConfig};
pub use plugin::{MarketplaceConfig, PluginConfig, PluginEntryConfig, PluginSourceConfig};
pub use remote::{RemoteConfig, RemoteEnvironmentConfig};
pub use scheduler::SchedulerConfig;
use serde::{Deserialize, Serialize};
pub use server::ServerConfig;
pub use skills::{discover_skills, find_skill, SkillDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
}

impl PermissionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "accept_edits",
            Self::BypassPermissions => "bypass_permissions",
        }
    }

    pub fn supported_values() -> &'static [&'static str] {
        &["default", "accept_edits", "bypass_permissions"]
    }
}

impl fmt::Display for PermissionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for PermissionMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "default" => Ok(Self::Default),
            "accept_edits" => Ok(Self::AcceptEdits),
            "bypass" | "bypass_permissions" => Ok(Self::BypassPermissions),
            _ => Err(format!(
                "Unsupported permission mode. Use one of: {}",
                Self::supported_values().join(", ")
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    #[serde(default)]
    pub mode: PermissionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default = "default_session_persist")]
    pub persist: bool,
    #[serde(default = "default_session_model")]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputStyleConfig {
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptCompositionConfig {
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub fragments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderConfig {
    Anthropic {
        #[serde(default = "default_anthropic_base_url")]
        base_url: String,
        #[serde(default = "default_anthropic_version")]
        anthropic_version: String,
        #[serde(default = "default_anthropic_api_key_env")]
        api_key_env: String,
    },
    OpenAiCompatible {
        #[serde(default = "default_openai_base_url")]
        base_url: String,
        #[serde(default = "default_openai_api_key_env")]
        api_key_env: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub provider: String,
    pub upstream_model: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelPricing {
    pub input_per_million_usd: f64,
    pub output_per_million_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloxConfig {
    #[serde(default = "default_gateway")]
    pub gateway: GatewayConfig,
    #[serde(default = "default_permissions")]
    pub permissions: PermissionConfig,
    #[serde(default = "default_session")]
    pub session: SessionConfig,
    #[serde(default)]
    pub output_style: OutputStyleConfig,
    #[serde(default)]
    pub prompt: PromptCompositionConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub lsp: LspConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub plugins: PluginConfig,
    #[serde(default)]
    pub remote: RemoteConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default = "default_providers")]
    pub providers: BTreeMap<String, ProviderConfig>,
    #[serde(default = "default_profiles")]
    pub profiles: BTreeMap<String, ProfileConfig>,
}

impl Default for HelloxConfig {
    fn default() -> Self {
        Self {
            gateway: default_gateway(),
            permissions: default_permissions(),
            session: default_session(),
            output_style: OutputStyleConfig::default(),
            prompt: PromptCompositionConfig::default(),
            scheduler: SchedulerConfig::default(),
            lsp: LspConfig::default(),
            mcp: McpConfig::default(),
            plugins: PluginConfig::default(),
            remote: RemoteConfig::default(),
            server: ServerConfig::default(),
            providers: default_providers(),
            profiles: default_profiles(),
        }
    }
}

pub fn config_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".hellox")
}

pub fn default_config_path() -> PathBuf {
    config_root().join("config.toml")
}

pub fn sessions_root() -> PathBuf {
    config_root().join("sessions")
}

pub fn shares_root() -> PathBuf {
    config_root().join("shares")
}

pub fn logs_root() -> PathBuf {
    config_root().join("logs")
}

pub fn tasks_root() -> PathBuf {
    config_root().join("tasks")
}

pub fn tasks_root_for(config_path: &std::path::Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("tasks")
}

pub fn scheduled_tasks_path() -> PathBuf {
    tasks_root().join("scheduled_tasks.json")
}

pub fn scheduled_tasks_path_for(config_path: &std::path::Path) -> PathBuf {
    tasks_root_for(config_path).join("scheduled_tasks.json")
}

pub fn plugins_root() -> PathBuf {
    config_root().join("plugins")
}

/// Return the root directory for user-defined output style files.
pub fn output_styles_root() -> PathBuf {
    config_root().join("output-styles")
}

pub fn personas_root() -> PathBuf {
    config_root().join("personas")
}

pub fn prompt_fragments_root() -> PathBuf {
    config_root().join("prompt-fragments")
}

/// Return the root directory for persisted memory files.
pub fn memory_root() -> PathBuf {
    config_root().join("memory")
}

pub fn telemetry_events_path() -> PathBuf {
    logs_root().join("telemetry-events.jsonl")
}

pub fn session_file_path(session_id: &str) -> PathBuf {
    sessions_root().join(format!("{session_id}.json"))
}

pub fn load_or_default(path: Option<PathBuf>) -> Result<HelloxConfig> {
    let config_path = path.unwrap_or_else(default_config_path);
    if !config_path.exists() {
        return Ok(HelloxConfig::default());
    }

    let raw = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config: {}", config_path.display()))?;
    let config = toml::from_str::<HelloxConfig>(&raw)
        .with_context(|| format!("failed to parse config: {}", config_path.display()))?;
    Ok(config)
}

pub fn save_config(path: Option<PathBuf>, config: &HelloxConfig) -> Result<PathBuf> {
    let config_path = path.unwrap_or_else(default_config_path);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config dir {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(&config_path, raw)
        .with_context(|| format!("failed to write config: {}", config_path.display()))?;
    Ok(config_path)
}

pub fn default_config_toml() -> String {
    toml::to_string_pretty(&HelloxConfig::default()).expect("default config must be serializable")
}

pub fn materialize_profiles(config: &HelloxConfig) -> BTreeMap<String, ModelProfile> {
    config
        .profiles
        .iter()
        .map(|(name, profile)| {
            let provider_config = config.providers.get(&profile.provider);
            let capabilities = provider_capabilities(provider_config);

            (
                name.clone(),
                ModelProfile {
                    name: name.clone(),
                    provider: profile.provider.clone(),
                    upstream_model: profile.upstream_model.clone(),
                    display_name: profile.display_name.clone().unwrap_or_else(|| name.clone()),
                    capabilities,
                },
            )
        })
        .collect()
}

pub fn pricing_for_model<'a>(config: &'a HelloxConfig, model: &str) -> Option<&'a ModelPricing> {
    config
        .profiles
        .get(model)
        .and_then(|profile| profile.pricing.as_ref())
        .or_else(|| {
            config
                .profiles
                .values()
                .find(|profile| profile.upstream_model == model)
                .and_then(|profile| profile.pricing.as_ref())
        })
}

pub fn estimate_cost_usd(pricing: &ModelPricing, input_tokens: u64, output_tokens: u64) -> f64 {
    let input_cost = (input_tokens as f64 / 1_000_000_f64) * pricing.input_per_million_usd;
    let output_cost = (output_tokens as f64 / 1_000_000_f64) * pricing.output_per_million_usd;
    input_cost + output_cost
}

fn provider_capabilities(provider: Option<&ProviderConfig>) -> ProviderCapabilities {
    match provider {
        Some(ProviderConfig::Anthropic { .. }) => ProviderCapabilities {
            tools: true,
            streaming: true,
            thinking: ReasoningCompatibility::Native,
            system_prompt: true,
        },
        Some(ProviderConfig::OpenAiCompatible { .. }) => ProviderCapabilities {
            tools: true,
            streaming: true,
            thinking: ReasoningCompatibility::Simulated,
            system_prompt: true,
        },
        None => ProviderCapabilities {
            tools: false,
            streaming: false,
            thinking: ReasoningCompatibility::Unsupported,
            system_prompt: false,
        },
    }
}

fn default_gateway() -> GatewayConfig {
    GatewayConfig {
        listen: default_listen(),
    }
}

fn default_permissions() -> PermissionConfig {
    PermissionConfig {
        mode: PermissionMode::Default,
    }
}

fn default_session() -> SessionConfig {
    SessionConfig {
        persist: default_session_persist(),
        model: default_session_model(),
    }
}

fn default_listen() -> String {
    "127.0.0.1:7821".to_string()
}

fn default_session_persist() -> bool {
    true
}

fn default_session_model() -> String {
    "opus".to_string()
}

fn default_anthropic_base_url() -> String {
    "https://api.anthropic.com".to_string()
}

fn default_anthropic_version() -> String {
    "2023-06-01".to_string()
}

fn default_anthropic_api_key_env() -> String {
    "ANTHROPIC_API_KEY".to_string()
}

fn default_openai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_openai_api_key_env() -> String {
    "OPENAI_API_KEY".to_string()
}

fn default_providers() -> BTreeMap<String, ProviderConfig> {
    BTreeMap::from([
        (
            "anthropic".to_string(),
            ProviderConfig::Anthropic {
                base_url: default_anthropic_base_url(),
                anthropic_version: default_anthropic_version(),
                api_key_env: default_anthropic_api_key_env(),
            },
        ),
        (
            "openai".to_string(),
            ProviderConfig::OpenAiCompatible {
                base_url: default_openai_base_url(),
                api_key_env: default_openai_api_key_env(),
            },
        ),
    ])
}

fn default_profiles() -> BTreeMap<String, ProfileConfig> {
    BTreeMap::from([
        (
            "opus".to_string(),
            ProfileConfig {
                provider: "anthropic".to_string(),
                upstream_model: "claude-opus-4-1-20250805".to_string(),
                display_name: Some("Opus".to_string()),
                pricing: Some(ModelPricing {
                    input_per_million_usd: 15.0,
                    output_per_million_usd: 75.0,
                }),
            },
        ),
        (
            "sonnet".to_string(),
            ProfileConfig {
                provider: "anthropic".to_string(),
                upstream_model: "claude-sonnet-4-5-20250929".to_string(),
                display_name: Some("Sonnet".to_string()),
                pricing: Some(ModelPricing {
                    input_per_million_usd: 3.0,
                    output_per_million_usd: 15.0,
                }),
            },
        ),
        (
            "haiku".to_string(),
            ProfileConfig {
                provider: "anthropic".to_string(),
                upstream_model: "claude-3-5-haiku-20241022".to_string(),
                display_name: Some("Haiku".to_string()),
                pricing: Some(ModelPricing {
                    input_per_million_usd: 0.8,
                    output_per_million_usd: 4.0,
                }),
            },
        ),
        (
            "openai_opus".to_string(),
            ProfileConfig {
                provider: "openai".to_string(),
                upstream_model: "gpt-5".to_string(),
                display_name: Some("OpenAI Opus".to_string()),
                pricing: Some(ModelPricing {
                    input_per_million_usd: 1.25,
                    output_per_million_usd: 10.0,
                }),
            },
        ),
        (
            "openai_sonnet".to_string(),
            ProfileConfig {
                provider: "openai".to_string(),
                upstream_model: "gpt-4.1".to_string(),
                display_name: Some("OpenAI Sonnet".to_string()),
                pricing: Some(ModelPricing {
                    input_per_million_usd: 2.0,
                    output_per_million_usd: 8.0,
                }),
            },
        ),
    ])
}
