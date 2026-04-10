use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicCompatRequest {
    pub model: String,
    #[serde(default)]
    pub system: Option<SystemPrompt>,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub thinking: Option<ThinkingConfig>,
    #[serde(default)]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    #[serde(default)]
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
    #[default]
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    Document {
        source: DocumentSource,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        context: Option<String>,
        #[serde(default)]
        citations: Option<DocumentCitations>,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: ToolResultContent,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
    #[default]
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    File { file_id: String },
    Base64 { media_type: String, data: String },
    Url { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentSource {
    File { file_id: String },
    Base64 { media_type: String, data: String },
    Url { url: String },
    Text { media_type: String, data: String },
    Content { content: Vec<ContentBlock> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCitations {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    None,
    Any,
    Tool { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicCompatResponse {
    pub id: String,
    pub r#type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

impl AnthropicCompatResponse {
    pub fn new(model: impl Into<String>, content: Vec<ContentBlock>, usage: Usage) -> Self {
        Self {
            id: format!("msg_{}", Uuid::new_v4().simple()),
            r#type: "message".to_string(),
            role: "assistant".to_string(),
            model: model.into(),
            content,
            stop_reason: Some(StopReason::EndTurn),
            stop_sequence: None,
            usage,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    PauseTurn,
    Refusal,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCard {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: ApiError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub r#type: String,
    pub message: String,
}

pub fn flatten_text_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Document {
                source,
                title,
                context,
                ..
            } => document_text_fragment(source, title.as_deref(), context.as_deref()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn extract_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Blocks(blocks) => flatten_text_blocks(blocks),
        MessageContent::Empty => String::new(),
    }
}

fn document_text_fragment<'a>(
    source: &'a DocumentSource,
    title: Option<&'a str>,
    context: Option<&'a str>,
) -> Option<&'a str> {
    match source {
        DocumentSource::Text { data, .. } => Some(data.as_str()),
        DocumentSource::Content { .. } => None,
        DocumentSource::File { .. }
        | DocumentSource::Base64 { .. }
        | DocumentSource::Url { .. } => title.or(context),
    }
}
