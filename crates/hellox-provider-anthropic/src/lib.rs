use std::env;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_auth::load_provider_key;
use hellox_config::ProviderConfig;
use hellox_core::AnthropicCompatAdapter;
use hellox_gateway_api::{
    AnthropicCompatRequest, AnthropicCompatResponse, ContentBlock, DocumentSource, ImageSource,
    MessageContent, ModelCard, SystemPrompt, ToolResultContent,
};
use reqwest::Client;

pub struct AnthropicAdapter {
    client: Client,
    base_url: String,
    anthropic_version: String,
    api_key_env: String,
    provider_name: String,
}

impl AnthropicAdapter {
    const FILES_API_BETA: &'static str = "files-api-2025-04-14";

    pub fn from_config(provider_name: &str, config: &ProviderConfig) -> Result<Self> {
        let ProviderConfig::Anthropic {
            base_url,
            anthropic_version,
            api_key_env,
        } = config
        else {
            return Err(anyhow!("provider config is not anthropic"));
        };

        Ok(Self {
            client: Client::new(),
            base_url: base_url.clone(),
            anthropic_version: anthropic_version.clone(),
            api_key_env: api_key_env.clone(),
            provider_name: provider_name.to_string(),
        })
    }

    fn api_key(&self) -> Result<String> {
        env::var(&self.api_key_env)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| load_provider_key(&self.provider_name).ok().flatten())
            .ok_or_else(|| {
                anyhow!(
                    "missing API key in env var {} or auth store provider key `{}`",
                    self.api_key_env,
                    self.provider_name
                )
            })
            .with_context(|| {
                format!(
                    "failed to resolve anthropic provider key for `{}`",
                    self.provider_name
                )
            })
    }

    fn request_contains_file_source(request: &AnthropicCompatRequest) -> bool {
        request
            .system
            .as_ref()
            .and_then(|system| match system {
                SystemPrompt::Blocks(blocks) => Some(Self::blocks_contain_file_source(blocks)),
                SystemPrompt::Text(_) => None,
            })
            .unwrap_or(false)
            || request
                .messages
                .iter()
                .any(|message| match &message.content {
                    MessageContent::Blocks(blocks) => Self::blocks_contain_file_source(blocks),
                    MessageContent::Text(_) | MessageContent::Empty => false,
                })
    }

    fn blocks_contain_file_source(blocks: &[ContentBlock]) -> bool {
        blocks.iter().any(|block| match block {
            ContentBlock::Image {
                source: ImageSource::File { .. },
            } => true,
            ContentBlock::Document {
                source: DocumentSource::File { .. },
                ..
            } => true,
            ContentBlock::ToolResult {
                content: ToolResultContent::Blocks(blocks),
                ..
            } => Self::blocks_contain_file_source(blocks),
            _ => false,
        })
    }
}

#[async_trait]
impl AnthropicCompatAdapter for AnthropicAdapter {
    async fn complete(&self, request: AnthropicCompatRequest) -> Result<AnthropicCompatResponse> {
        let api_key = self.api_key()?;
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let contains_file_source = Self::request_contains_file_source(&request);

        let mut builder = self
            .client
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", &self.anthropic_version);
        if contains_file_source {
            builder = builder.header("anthropic-beta", Self::FILES_API_BETA);
        }

        let response = builder
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<AnthropicCompatResponse>()
            .await?;

        Ok(response)
    }

    async fn list_models(&self) -> Result<Vec<ModelCard>> {
        let api_key = self.api_key()?;
        let url = format!("{}/v1/models", self.base_url.trim_end_matches('/'));

        let response = self
            .client
            .get(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", &self.anthropic_version)
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let body = response.json::<serde_json::Value>().await?;
        let cards = body
            .get("data")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                Some(ModelCard {
                    id: entry.get("id")?.as_str()?.to_string(),
                    display_name: entry
                        .get("display_name")
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string),
                    provider: Some("anthropic".to_string()),
                    capabilities: vec!["messages".to_string(), "tools".to_string()],
                })
            })
            .collect();

        Ok(cards)
    }
}
