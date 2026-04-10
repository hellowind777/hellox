use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use hellox_config::HelloxConfig;
use hellox_gateway_api::{AnthropicCompatRequest, AnthropicCompatResponse};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::Deserialize;
use tokio::fs;

use crate::telemetry::{AgentTelemetryEvent, SharedTelemetrySink};

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GatewayUploadedFile {
    pub id: String,
}

#[derive(Clone)]
pub struct GatewayClient {
    client: Client,
    base_url: String,
    provider_by_model: BTreeMap<String, String>,
    telemetry_sink: Option<SharedTelemetrySink>,
}

impl GatewayClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: normalize_base_url(base_url.into()),
            provider_by_model: BTreeMap::new(),
            telemetry_sink: None,
        }
    }

    pub fn from_config(config: &HelloxConfig, override_url: Option<String>) -> Self {
        let base_url = override_url.unwrap_or_else(|| config.gateway.listen.clone());
        let mut provider_by_model = BTreeMap::new();
        for (profile_name, profile) in &config.profiles {
            provider_by_model.insert(profile_name.clone(), profile.provider.clone());
            provider_by_model.insert(profile.upstream_model.clone(), profile.provider.clone());
        }
        Self {
            client: Client::new(),
            base_url: normalize_base_url(base_url),
            provider_by_model,
            telemetry_sink: None,
        }
    }

    pub fn with_telemetry(mut self, telemetry_sink: Option<SharedTelemetrySink>) -> Self {
        self.telemetry_sink = telemetry_sink;
        self
    }

    pub async fn complete(
        &self,
        request: &AnthropicCompatRequest,
    ) -> Result<AnthropicCompatResponse> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let provider = self.provider_by_model.get(&request.model).cloned();
        self.emit_gateway_event(
            "request_started",
            build_request_attributes(request, provider.as_deref()),
        );

        let result = self
            .client
            .post(url)
            .json(request)
            .send()
            .await
            .context("failed to send request to hellox gateway")?
            .error_for_status()
            .context("hellox gateway returned an error status")?
            .json::<AnthropicCompatResponse>()
            .await
            .context("failed to decode hellox gateway response");

        match result {
            Ok(response) => {
                let attributes = build_response_attributes(request, &response, provider.as_deref());
                self.emit_gateway_event("request_completed", attributes.clone());
                if provider.is_some() {
                    self.emit_provider_event("request_completed", attributes);
                }
                Ok(response)
            }
            Err(error) => {
                let mut attributes = build_request_attributes(request, provider.as_deref());
                attributes.insert("error".to_string(), error.to_string());
                self.emit_gateway_event("request_failed", attributes.clone());
                if provider.is_some() {
                    self.emit_provider_event("request_failed", attributes);
                }
                Err(error)
            }
        }
    }

    pub(crate) async fn upload_file_path(
        &self,
        path: &Path,
        purpose: Option<&str>,
        mime_type: Option<&str>,
    ) -> Result<GatewayUploadedFile> {
        let url = format!("{}/v1/files", self.base_url.trim_end_matches('/'));
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("upload.bin")
            .to_string();
        let bytes = fs::read(path)
            .await
            .with_context(|| format!("failed to read file {}", path.display()))?;

        let mut file_part = Part::bytes(bytes).file_name(filename.clone());
        if let Some(mime_type) = mime_type {
            file_part = file_part
                .mime_str(mime_type)
                .with_context(|| format!("invalid mime type `{mime_type}`"))?;
        }

        let mut form = Form::new().part("file", file_part);
        if let Some(purpose) = purpose {
            form = form.text("purpose", purpose.to_string());
        }

        self.client
            .post(url)
            .multipart(form)
            .send()
            .await
            .context("failed to upload file to hellox gateway")?
            .error_for_status()
            .context("hellox gateway returned an error status for file upload")?
            .json::<GatewayUploadedFile>()
            .await
            .context("failed to decode hellox gateway file upload response")
    }

    fn emit_gateway_event(&self, name: &str, attributes: BTreeMap<String, String>) {
        self.emit_event("gateway", name, attributes);
    }

    fn emit_provider_event(&self, name: &str, attributes: BTreeMap<String, String>) {
        self.emit_event("provider", name, attributes);
    }

    fn emit_event(&self, domain: &str, name: &str, attributes: BTreeMap<String, String>) {
        let Some(telemetry_sink) = &self.telemetry_sink else {
            return;
        };
        if let Err(error) = telemetry_sink.record(AgentTelemetryEvent {
            domain: domain.to_string(),
            name: name.to_string(),
            session_id: None,
            attributes,
        }) {
            eprintln!("Warning: failed to persist {domain} telemetry event: {error}");
        }
    }
}

fn build_request_attributes(
    request: &AnthropicCompatRequest,
    provider: Option<&str>,
) -> BTreeMap<String, String> {
    let mut attributes = BTreeMap::from([
        ("model".to_string(), request.model.clone()),
        (
            "message_count".to_string(),
            request.messages.len().to_string(),
        ),
        ("tool_count".to_string(), request.tools.len().to_string()),
    ]);
    if let Some(provider) = provider {
        attributes.insert("provider".to_string(), provider.to_string());
    }
    attributes
}

fn build_response_attributes(
    request: &AnthropicCompatRequest,
    response: &AnthropicCompatResponse,
    provider: Option<&str>,
) -> BTreeMap<String, String> {
    let mut attributes = build_request_attributes(request, provider);
    attributes.insert("response_model".to_string(), response.model.clone());
    attributes.insert(
        "input_tokens".to_string(),
        response.usage.input_tokens.to_string(),
    );
    attributes.insert(
        "output_tokens".to_string(),
        response.usage.output_tokens.to_string(),
    );
    attributes.insert(
        "tool_use_blocks".to_string(),
        response
            .content
            .iter()
            .filter(|block| matches!(block, hellox_gateway_api::ContentBlock::ToolUse { .. }))
            .count()
            .to_string(),
    );
    if let Some(stop_reason) = &response.stop_reason {
        attributes.insert("stop_reason".to_string(), format!("{stop_reason:?}"));
    }
    attributes
}

fn normalize_base_url(value: String) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        value
    } else {
        format!("http://{}", value)
    }
}
