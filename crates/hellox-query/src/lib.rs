use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_gateway_api::{
    extract_text, AnthropicCompatRequest, AnthropicCompatResponse, ContentBlock, Message,
    MessageContent, SystemPrompt, ThinkingConfig, ToolChoice, ToolDefinition,
};

#[derive(Clone, Debug)]
pub struct QueryOptions {
    pub model: String,
    pub max_turns: usize,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            model: "opus".to_string(),
            max_turns: 12,
            max_tokens: Some(4_096),
            temperature: None,
            thinking: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct QueryTurnResult {
    pub final_text: String,
    pub response: AnthropicCompatResponse,
    pub iterations: usize,
}

#[async_trait]
pub trait QueryRuntime {
    fn query_options(&self) -> QueryOptions;
    fn messages(&self) -> &[Message];
    fn effective_system_prompt(&self) -> Result<String>;
    fn tool_definitions(&self) -> Vec<ToolDefinition>;
    fn push_message(&mut self, message: Message) -> Result<()>;
    fn record_response_usage(&mut self, response: &AnthropicCompatResponse);
    async fn complete_request(
        &self,
        request: &AnthropicCompatRequest,
    ) -> Result<AnthropicCompatResponse>;
    async fn execute_tool_batch(&self, response: &AnthropicCompatResponse) -> Message;
}

pub async fn run_query_prompt(
    runtime: &mut (impl QueryRuntime + Send),
    prompt: impl Into<String>,
) -> Result<QueryTurnResult> {
    runtime.push_message(Message {
        role: hellox_gateway_api::MessageRole::User,
        content: MessageContent::Text(prompt.into()),
    })?;

    let options = runtime.query_options();
    for iteration in 1..=options.max_turns {
        let request = build_request(
            &options,
            runtime.effective_system_prompt()?,
            runtime.messages(),
            runtime.tool_definitions(),
        );
        let response = runtime.complete_request(&request).await?;
        runtime.record_response_usage(&response);
        runtime.push_message(Message {
            role: hellox_gateway_api::MessageRole::Assistant,
            content: MessageContent::Blocks(response.content.clone()),
        })?;

        let has_tool_use = response
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }));

        if !has_tool_use {
            return Ok(QueryTurnResult {
                final_text: response_text(&response),
                response,
                iterations: iteration,
            });
        }

        let tool_result_message = runtime.execute_tool_batch(&response).await;
        runtime.push_message(tool_result_message)?;
    }

    Err(anyhow!(
        "agent exceeded max_turns ({}) before reaching a final answer",
        options.max_turns
    ))
}

pub fn build_request(
    options: &QueryOptions,
    system_prompt: String,
    messages: &[Message],
    tools: Vec<ToolDefinition>,
) -> AnthropicCompatRequest {
    AnthropicCompatRequest {
        model: options.model.clone(),
        system: Some(SystemPrompt::Text(system_prompt)),
        messages: messages.to_vec(),
        tool_choice: if tools.is_empty() {
            None
        } else {
            Some(ToolChoice::Auto)
        },
        tools,
        max_tokens: options.max_tokens,
        temperature: options.temperature,
        top_p: None,
        metadata: None,
        thinking: options.thinking.clone(),
        stream: Some(false),
    }
}

pub fn response_text(response: &AnthropicCompatResponse) -> String {
    extract_text(&MessageContent::Blocks(response.content.clone()))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use hellox_gateway_api::{ContentBlock, Message, MessageContent, ToolDefinition, Usage};
    use serde_json::json;

    use super::{run_query_prompt, QueryOptions, QueryRuntime};

    struct FakeRuntime {
        options: QueryOptions,
        system_prompt: String,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
        responses: Arc<Mutex<VecDeque<hellox_gateway_api::AnthropicCompatResponse>>>,
        recorded_usage: usize,
    }

    #[async_trait]
    impl QueryRuntime for FakeRuntime {
        fn query_options(&self) -> QueryOptions {
            self.options.clone()
        }

        fn messages(&self) -> &[Message] {
            &self.messages
        }

        fn effective_system_prompt(&self) -> Result<String> {
            Ok(self.system_prompt.clone())
        }

        fn tool_definitions(&self) -> Vec<ToolDefinition> {
            self.tools.clone()
        }

        fn push_message(&mut self, message: Message) -> Result<()> {
            self.messages.push(message);
            Ok(())
        }

        fn record_response_usage(
            &mut self,
            _response: &hellox_gateway_api::AnthropicCompatResponse,
        ) {
            self.recorded_usage += 1;
        }

        async fn complete_request(
            &self,
            _request: &hellox_gateway_api::AnthropicCompatRequest,
        ) -> Result<hellox_gateway_api::AnthropicCompatResponse> {
            self.responses
                .lock()
                .map_err(|_| anyhow!("responses lock poisoned"))?
                .pop_front()
                .ok_or_else(|| anyhow!("missing queued response"))
        }

        async fn execute_tool_batch(
            &self,
            response: &hellox_gateway_api::AnthropicCompatResponse,
        ) -> Message {
            let mut blocks = Vec::new();
            for block in &response.content {
                if let ContentBlock::ToolUse { id, name, .. } = block {
                    blocks.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: hellox_gateway_api::ToolResultContent::Text(format!("ran {name}")),
                        is_error: false,
                    });
                }
            }
            Message {
                role: hellox_gateway_api::MessageRole::User,
                content: MessageContent::Blocks(blocks),
            }
        }
    }

    impl FakeRuntime {
        async fn run(&mut self, prompt: &str) -> Result<super::QueryTurnResult> {
            run_query_prompt(self, prompt).await
        }
    }

    #[tokio::test]
    async fn query_prompt_returns_final_text_without_tool_use() {
        let response = hellox_gateway_api::AnthropicCompatResponse::new(
            "mock-model",
            vec![ContentBlock::Text {
                text: "done".to_string(),
            }],
            Usage {
                input_tokens: 12,
                output_tokens: 4,
            },
        );
        let mut runtime = FakeRuntime {
            options: QueryOptions {
                model: "mock-model".to_string(),
                max_turns: 2,
                ..QueryOptions::default()
            },
            system_prompt: "system".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            responses: Arc::new(Mutex::new(VecDeque::from([response]))),
            recorded_usage: 0,
        };

        let result = runtime.run("hello").await.expect("run query");
        assert_eq!(result.final_text, "done");
        assert_eq!(result.iterations, 1);
        assert_eq!(runtime.recorded_usage, 1);
    }

    #[tokio::test]
    async fn query_prompt_executes_tool_batch_before_final_answer() {
        let tool_response = hellox_gateway_api::AnthropicCompatResponse::new(
            "mock-model",
            vec![ContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "Read".to_string(),
                input: json!({ "file_path": "README.md" }),
            }],
            Usage {
                input_tokens: 10,
                output_tokens: 2,
            },
        );
        let final_response = hellox_gateway_api::AnthropicCompatResponse::new(
            "mock-model",
            vec![ContentBlock::Text {
                text: "finished".to_string(),
            }],
            Usage {
                input_tokens: 14,
                output_tokens: 3,
            },
        );
        let mut runtime = FakeRuntime {
            options: QueryOptions {
                model: "mock-model".to_string(),
                max_turns: 3,
                ..QueryOptions::default()
            },
            system_prompt: "system".to_string(),
            messages: Vec::new(),
            tools: vec![ToolDefinition {
                name: "Read".to_string(),
                description: None,
                input_schema: json!({}),
            }],
            responses: Arc::new(Mutex::new(VecDeque::from([tool_response, final_response]))),
            recorded_usage: 0,
        };

        let result = run_query_prompt(&mut runtime, "inspect")
            .await
            .expect("run query");
        assert_eq!(result.final_text, "finished");
        assert_eq!(result.iterations, 2);
        assert_eq!(runtime.recorded_usage, 2);
        assert_eq!(runtime.messages.len(), 4);
    }
}
