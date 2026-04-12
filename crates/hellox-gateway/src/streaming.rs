use axum::response::sse::Event;
use hellox_gateway_api::{
    flatten_text_blocks, AnthropicCompatResponse, ContentBlock, StopReason, ToolResultContent,
};
use serde_json::{json, Value};

pub(crate) fn anthropic_sse_events(response: &AnthropicCompatResponse) -> Vec<Event> {
    let mut events = Vec::new();
    events.push(
        Event::default()
            .event("message_start")
            .json_data(json!({
                "type": "message_start",
                "message": {
                    "id": response.id,
                    "type": response.r#type,
                    "role": response.role,
                    "model": response.model,
                    "content": [],
                    "stop_reason": Value::Null,
                    "stop_sequence": Value::Null,
                    "usage": {"input_tokens": response.usage.input_tokens, "output_tokens": 0}
                }
            }))
            .expect("valid message_start event"),
    );

    for (index, block) in response.content.iter().enumerate() {
        let block_value = block_start_value(block);
        events.push(
            Event::default()
                .event("content_block_start")
                .json_data(json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": block_value
                }))
                .expect("valid block_start event"),
        );

        if let Some(delta) = block_delta_value(block) {
            events.push(
                Event::default()
                    .event("content_block_delta")
                    .json_data(json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": delta
                    }))
                    .expect("valid block_delta event"),
            );
        }

        events.push(
            Event::default()
                .event("content_block_stop")
                .json_data(json!({
                    "type": "content_block_stop",
                    "index": index
                }))
                .expect("valid block_stop event"),
        );
    }

    events.push(
        Event::default()
            .event("message_delta")
            .json_data(json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": stop_reason_to_value(&response.stop_reason),
                    "stop_sequence": response.stop_sequence
                },
                "usage": {
                    "output_tokens": response.usage.output_tokens
                }
            }))
            .expect("valid message_delta event"),
    );

    events.push(
        Event::default()
            .event("message_stop")
            .json_data(json!({ "type": "message_stop" }))
            .expect("valid message_stop event"),
    );

    events
}

fn block_start_value(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text { .. } => json!({"type": "text", "text": ""}),
        ContentBlock::Image { source } => json!({
            "type": "image",
            "source": source_to_value(source)
        }),
        ContentBlock::Document {
            source,
            title,
            context,
            citations,
        } => json!({
            "type": "document",
            "source": source_to_value(source),
            "title": title,
            "context": context,
            "citations": citations
        }),
        ContentBlock::Thinking { signature, .. } => json!({
            "type": "thinking",
            "thinking": "",
            "signature": signature
        }),
        ContentBlock::RedactedThinking { .. } => json!({"type": "redacted_thinking", "data": ""}),
        ContentBlock::ToolUse { id, name, .. } => json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": {}
        }),
        ContentBlock::ToolResult {
            tool_use_id,
            is_error,
            ..
        } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": "",
            "is_error": is_error
        }),
    }
}

fn block_delta_value(block: &ContentBlock) -> Option<Value> {
    match block {
        ContentBlock::Text { text } => Some(json!({"type": "text_delta", "text": text})),
        ContentBlock::Image { .. } | ContentBlock::Document { .. } => None,
        ContentBlock::Thinking { thinking, .. } => {
            Some(json!({"type": "thinking_delta", "thinking": thinking}))
        }
        ContentBlock::RedactedThinking { data } => {
            Some(json!({"type": "thinking_delta", "thinking": data}))
        }
        ContentBlock::ToolUse { input, .. } => Some(json!({
            "type": "input_json_delta",
            "partial_json": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string())
        })),
        ContentBlock::ToolResult {
            content: ToolResultContent::Text(text),
            ..
        } => Some(json!({"type": "text_delta", "text": text})),
        ContentBlock::ToolResult {
            content: ToolResultContent::Blocks(blocks),
            ..
        } => Some(json!({
            "type": "text_delta",
            "text": flatten_text_blocks(blocks)
        })),
        ContentBlock::ToolResult { .. } => None,
    }
}

fn source_to_value(source: &impl serde::Serialize) -> Value {
    serde_json::to_value(source).expect("content source should serialize")
}

fn stop_reason_to_value(stop_reason: &Option<StopReason>) -> Value {
    match stop_reason {
        Some(StopReason::EndTurn) => Value::String("end_turn".to_string()),
        Some(StopReason::MaxTokens) => Value::String("max_tokens".to_string()),
        Some(StopReason::StopSequence) => Value::String("stop_sequence".to_string()),
        Some(StopReason::ToolUse) => Value::String("tool_use".to_string()),
        Some(StopReason::PauseTurn) => Value::String("pause_turn".to_string()),
        Some(StopReason::Refusal) => Value::String("refusal".to_string()),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use axum::{
        body::to_bytes,
        response::{sse::Sse, IntoResponse},
    };
    use futures_util::stream;
    use serde_json::json;

    use hellox_gateway_api::{DocumentCitations, DocumentSource, ImageSource, Usage};

    use super::*;

    #[test]
    fn block_start_preserves_image_source_payload() {
        let block = ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "ZmFrZS1pbWFnZQ==".to_string(),
            },
        };

        let value = block_start_value(&block);
        assert_eq!(
            value,
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": "ZmFrZS1pbWFnZQ=="
                }
            })
        );
    }

    #[test]
    fn block_start_preserves_document_source_payload() {
        let block = ContentBlock::Document {
            source: DocumentSource::Content {
                content: vec![ContentBlock::Text {
                    text: "hello from document".to_string(),
                }],
            },
            title: Some("Doc".to_string()),
            context: Some("ctx".to_string()),
            citations: Some(DocumentCitations { enabled: true }),
        };

        let value = block_start_value(&block);
        assert_eq!(
            value,
            json!({
                "type": "document",
                "source": {
                    "type": "content",
                    "content": [
                        {
                            "type": "text",
                            "text": "hello from document"
                        }
                    ]
                },
                "title": "Doc",
                "context": "ctx",
                "citations": {
                    "enabled": true
                }
            })
        );
    }

    #[tokio::test]
    async fn sse_stream_preserves_non_text_block_source_values() {
        let response = AnthropicCompatResponse::new(
            "claude-test",
            vec![
                ContentBlock::Image {
                    source: ImageSource::File {
                        file_id: "file_123".to_string(),
                    },
                },
                ContentBlock::Document {
                    source: DocumentSource::Text {
                        media_type: "text/plain".to_string(),
                        data: "hello world".to_string(),
                    },
                    title: Some("Notes".to_string()),
                    context: None,
                    citations: None,
                },
            ],
            Usage {
                input_tokens: 10,
                output_tokens: 4,
            },
        );

        let events = anthropic_sse_events(&response);
        let rendered = String::from_utf8(
            to_bytes(
                Sse::new(stream::iter(events.into_iter().map(Ok::<_, Infallible>)))
                    .into_response()
                    .into_body(),
                usize::MAX,
            )
            .await
            .expect("serialize sse body")
            .to_vec(),
        )
        .expect("utf8 sse body");

        assert!(rendered.contains("\"file_id\":\"file_123\""), "{rendered}");
        assert!(
            rendered.contains("\"media_type\":\"text/plain\""),
            "{rendered}"
        );
        assert!(rendered.contains("\"data\":\"hello world\""), "{rendered}");
    }
}
