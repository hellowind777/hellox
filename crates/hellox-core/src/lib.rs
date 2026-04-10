use anyhow::Result;
use async_trait::async_trait;
use hellox_gateway_api::{AnthropicCompatRequest, AnthropicCompatResponse, ModelCard};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    OpenAiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningCompatibility {
    Native,
    Simulated,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub tools: bool,
    pub streaming: bool,
    pub thinking: ReasoningCompatibility,
    pub system_prompt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub name: String,
    pub provider: String,
    pub upstream_model: String,
    pub display_name: String,
    pub capabilities: ProviderCapabilities,
}

#[async_trait]
pub trait AnthropicCompatAdapter: Send + Sync {
    async fn complete(&self, request: AnthropicCompatRequest) -> Result<AnthropicCompatResponse>;

    async fn list_models(&self) -> Result<Vec<ModelCard>>;
}
