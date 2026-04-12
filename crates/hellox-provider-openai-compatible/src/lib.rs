use std::env;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_auth::load_provider_key;
use hellox_config::ProviderConfig;
use hellox_core::AnthropicCompatAdapter;
use hellox_gateway_api::{
    extract_text, flatten_text_blocks, AnthropicCompatRequest, AnthropicCompatResponse,
    ContentBlock, DocumentSource, ImageSource, Message, MessageContent, MessageRole, ModelCard,
    StopReason, ToolChoice, Usage,
};
use reqwest::Client;
use serde_json::{json, Value};

pub struct OpenAiCompatibleAdapter {
    client: Client,
    base_url: String,
    api_key_env: String,
    provider_name: String,
}

impl OpenAiCompatibleAdapter {
    pub fn from_config(provider_name: &str, config: &ProviderConfig) -> Result<Self> {
        let ProviderConfig::OpenAiCompatible {
            base_url,
            api_key_env,
        } = config
        else {
            return Err(anyhow!("provider config is not openai_compatible"));
        };

        Ok(Self {
            client: Client::new(),
            base_url: base_url.clone(),
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
                    "failed to resolve openai-compatible provider key for `{}`",
                    self.provider_name
                )
            })
    }

    fn map_messages(request: &AnthropicCompatRequest) -> Result<Vec<Value>> {
        let mut messages = Vec::new();

        if let Some(system) = &request.system {
            let system_text = match system {
                hellox_gateway_api::SystemPrompt::Text(text) => text.clone(),
                hellox_gateway_api::SystemPrompt::Blocks(blocks) => {
                    hellox_gateway_api::flatten_text_blocks(blocks)
                }
            };
            if !system_text.is_empty() {
                messages.push(json!({
                    "role": "system",
                    "content": system_text,
                }));
            }
        }

        for message in &request.messages {
            messages.push(map_message(message)?);
        }

        Ok(messages)
    }

    fn map_tools(request: &AnthropicCompatRequest) -> Option<Vec<Value>> {
        if request.tools.is_empty() {
            return None;
        }

        Some(
            request
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.input_schema,
                        }
                    })
                })
                .collect(),
        )
    }
}

fn map_message(message: &Message) -> Result<Value> {
    let role = match message.role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
    };

    Ok(match &message.content {
        MessageContent::Text(text) => json!({
            "role": role,
            "content": text,
        }),
        MessageContent::Blocks(blocks) => {
            let mut content = Vec::new();
            let mut tool_calls = Vec::new();

            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        content.push(json!({"type": "text", "text": text}));
                    }
                    ContentBlock::Image { source } => {
                        content.push(json!({
                            "type": "image_url",
                            "image_url": {
                                "url": image_url_for_source(source)?,
                            }
                        }));
                    }
                    ContentBlock::Document {
                        source,
                        title,
                        context,
                        ..
                    } => {
                        content.push(json!({
                            "type": "text",
                            "text": render_document_text(source, title.as_deref(), context.as_deref())?,
                        }));
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string()),
                            }
                        }));
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content: tool_content,
                        is_error,
                    } => {
                        content.push(json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": match tool_content {
                                hellox_gateway_api::ToolResultContent::Text(text) => Value::String(text.clone()),
                                hellox_gateway_api::ToolResultContent::Blocks(blocks) => {
                                    Value::String(hellox_gateway_api::flatten_text_blocks(blocks))
                                }
                                hellox_gateway_api::ToolResultContent::Empty => Value::String(String::new()),
                            },
                            "is_error": is_error,
                        }));
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        content.push(json!({"type": "text", "text": thinking}));
                    }
                    ContentBlock::RedactedThinking { data } => {
                        content.push(json!({"type": "text", "text": data}));
                    }
                }
            }

            let mut payload = json!({
                "role": role,
                "content": content,
            });
            if !tool_calls.is_empty() {
                payload["tool_calls"] = Value::Array(tool_calls);
            }
            payload
        }
        MessageContent::Empty => json!({
            "role": role,
            "content": "",
        }),
    })
}

fn image_url_for_source(source: &ImageSource) -> Result<String> {
    match source {
        ImageSource::Url { url } => Ok(url.clone()),
        ImageSource::Base64 { media_type, data } => Ok(format!("data:{media_type};base64,{data}")),
        ImageSource::File { file_id } => Err(anyhow!(
            "openai-compatible adapter cannot use unresolved file source `{file_id}`"
        )),
    }
}

fn render_document_text(
    source: &DocumentSource,
    title: Option<&str>,
    context: Option<&str>,
) -> Result<String> {
    let mut lines = Vec::new();
    if let Some(title) = title.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("Document title: {title}"));
    }
    if let Some(context) = context.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("Document context: {context}"));
    }

    match source {
        DocumentSource::Text { data, .. } => lines.push(data.clone()),
        DocumentSource::Content { content } => lines.push(flatten_text_blocks(content)),
        DocumentSource::Url { url } => {
            return Err(anyhow!(
                "openai-compatible adapter does not support document URL sources yet: {url}"
            ));
        }
        DocumentSource::Base64 { media_type, .. } => {
            return Err(anyhow!(
                "openai-compatible adapter does not support inline base64 document media type `{media_type}` yet"
            ));
        }
        DocumentSource::File { file_id } => {
            return Err(anyhow!(
                "openai-compatible adapter cannot use unresolved file source `{file_id}`"
            ));
        }
    }

    Ok(lines.join("\n\n"))
}

#[async_trait]
impl AnthropicCompatAdapter for OpenAiCompatibleAdapter {
    async fn complete(&self, request: AnthropicCompatRequest) -> Result<AnthropicCompatResponse> {
        let api_key = self.api_key()?;
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let tools = Self::map_tools(&request);
        let messages = Self::map_messages(&request)?;

        let mut payload = json!({
            "model": request.model,
            "messages": messages,
            "temperature": request.temperature,
            "top_p": request.top_p,
            "max_tokens": request.max_tokens,
            "stream": false,
        });

        if let Some(tools) = tools {
            payload["tools"] = Value::Array(tools);
        }

        if let Some(tool_choice) = request.tool_choice {
            payload["tool_choice"] = match tool_choice {
                ToolChoice::Auto => Value::String("auto".to_string()),
                ToolChoice::None => Value::String("none".to_string()),
                ToolChoice::Any => Value::String("required".to_string()),
                ToolChoice::Tool { name } => json!({
                    "type": "function",
                    "function": { "name": name }
                }),
            };
        }

        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let choice = response
            .get("choices")
            .and_then(|value| value.as_array())
            .and_then(|choices| choices.first())
            .ok_or_else(|| anyhow!("openai-compatible response missing first choice"))?;

        let message = choice
            .get("message")
            .ok_or_else(|| anyhow!("openai-compatible response missing message"))?;

        let mut content = Vec::new();
        if let Some(reasoning) = message.get("reasoning").and_then(|value| value.as_str()) {
            content.push(ContentBlock::Thinking {
                thinking: reasoning.to_string(),
                signature: None,
            });
        }

        if let Some(text) = message.get("content") {
            if let Some(text) = text.as_str() {
                if !text.is_empty() {
                    content.push(ContentBlock::Text {
                        text: text.to_string(),
                    });
                }
            } else if let Some(parts) = text.as_array() {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|value| value.as_str()) {
                        content.push(ContentBlock::Text {
                            text: text.to_string(),
                        });
                    }
                }
            }
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(|value| value.as_array()) {
            for tool_call in tool_calls {
                let id = tool_call
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("tool_call");
                let function = tool_call.get("function").cloned().unwrap_or_default();
                let name = function
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("tool");
                let input = function
                    .get("arguments")
                    .and_then(|value| value.as_str())
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                    .unwrap_or_else(|| json!({}));

                content.push(ContentBlock::ToolUse {
                    id: id.to_string(),
                    name: name.to_string(),
                    input,
                });
            }
        }

        if content.is_empty() {
            let flattened = request
                .messages
                .last()
                .map(|message| extract_text(&message.content))
                .unwrap_or_default();
            content.push(ContentBlock::Text { text: flattened });
        }

        let stop_reason = choice
            .get("finish_reason")
            .and_then(|value| value.as_str())
            .map(|reason| match reason {
                "length" => StopReason::MaxTokens,
                "tool_calls" => StopReason::ToolUse,
                _ => StopReason::EndTurn,
            })
            .unwrap_or(StopReason::EndTurn);

        let usage = Usage {
            input_tokens: response
                .get("usage")
                .and_then(|usage| usage.get("prompt_tokens"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0) as u32,
            output_tokens: response
                .get("usage")
                .and_then(|usage| usage.get("completion_tokens"))
                .and_then(|value| value.as_u64())
                .unwrap_or(0) as u32,
        };

        let mut output = AnthropicCompatResponse::new(request.model, content, usage);
        output.stop_reason = Some(stop_reason);
        Ok(output)
    }

    async fn list_models(&self) -> Result<Vec<ModelCard>> {
        let api_key = self.api_key()?;
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let response = self.client.get(url).bearer_auth(api_key).send().await?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let body = response.json::<Value>().await?;
        let models = body
            .get("data")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                Some(ModelCard {
                    id: entry.get("id")?.as_str()?.to_string(),
                    display_name: None,
                    provider: Some("openai".to_string()),
                    capabilities: vec!["messages".to_string(), "tools".to_string()],
                })
            })
            .collect();

        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::OpenAiCompatibleAdapter;
    use hellox_gateway_api::{
        AnthropicCompatRequest, ContentBlock, DocumentSource, Message, MessageContent, MessageRole,
        ToolChoice,
    };

    #[test]
    fn map_messages_renders_text_document_blocks_as_text() {
        let request = AnthropicCompatRequest {
            model: "gpt-5".to_string(),
            system: None,
            messages: vec![Message {
                role: MessageRole::User,
                content: MessageContent::Blocks(vec![ContentBlock::Document {
                    source: DocumentSource::Text {
                        media_type: "text/plain".to_string(),
                        data: "hello from file".to_string(),
                    },
                    title: Some("Readme".to_string()),
                    context: Some("Repository notes".to_string()),
                    citations: None,
                }]),
            }],
            tools: Vec::new(),
            tool_choice: Some(ToolChoice::None),
            max_tokens: Some(256),
            temperature: None,
            top_p: None,
            metadata: None,
            thinking: None,
            stream: Some(false),
        };

        let messages = OpenAiCompatibleAdapter::map_messages(&request).expect("map messages");
        assert_eq!(messages.len(), 1);
        let content = messages[0]
            .get("content")
            .and_then(|value| value.as_array())
            .expect("content parts");
        assert_eq!(content[0]["type"], json!("text"));
        let text = content[0]["text"].as_str().expect("text");
        assert!(text.contains("Document title: Readme"));
        assert!(text.contains("Repository notes"));
        assert!(text.contains("hello from file"));
    }

    #[test]
    fn render_document_text_rejects_unresolved_file_sources() {
        let error = super::render_document_text(
            &DocumentSource::File {
                file_id: "file_remote".to_string(),
            },
            None,
            None,
        )
        .expect_err("unresolved file source should fail");
        assert!(error
            .to_string()
            .contains("cannot use unresolved file source"));
    }
}
