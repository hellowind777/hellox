use anyhow::Result;
use async_trait::async_trait;
use hellox_gateway_api::{
    AnthropicCompatRequest, AnthropicCompatResponse, ContentBlock, Message, MessageContent,
};
use hellox_query::{QueryOptions, QueryRuntime};

use super::{AgentOptions, AgentSession};

impl AgentOptions {
    pub(super) fn query_options(&self) -> QueryOptions {
        QueryOptions {
            model: self.model.clone(),
            max_turns: self.max_turns,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            thinking: self.thinking.clone(),
        }
    }
}

impl AgentSession {
    pub(super) async fn build_tool_result_message(
        &self,
        response: &AnthropicCompatResponse,
    ) -> Message {
        let mut results = Vec::new();

        for block in &response.content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                let result = self.tools.execute(name, input.clone(), &self.context).await;
                self.emit_tool_event(name, result.is_error, &result.content);
                results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: result.content,
                    is_error: result.is_error,
                });
            }
        }

        Message {
            role: hellox_gateway_api::MessageRole::User,
            content: MessageContent::Blocks(results),
        }
    }
}

#[async_trait]
impl QueryRuntime for AgentSession {
    fn query_options(&self) -> QueryOptions {
        self.options.query_options()
    }

    fn messages(&self) -> &[Message] {
        &self.messages
    }

    fn effective_system_prompt(&self) -> Result<String> {
        Ok(self.effective_system_prompt())
    }

    fn tool_definitions(&self) -> Vec<hellox_gateway_api::ToolDefinition> {
        self.tools.definitions()
    }

    fn push_message(&mut self, message: Message) -> Result<()> {
        self.messages.push(message);
        self.persist()
    }

    fn record_response_usage(&mut self, response: &AnthropicCompatResponse) {
        self.store_response_usage(response);
    }

    async fn complete_request(
        &self,
        request: &AnthropicCompatRequest,
    ) -> Result<AnthropicCompatResponse> {
        self.client.complete(request).await
    }

    async fn execute_tool_batch(&self, response: &AnthropicCompatResponse) -> Message {
        self.build_tool_result_message(response).await
    }
}
