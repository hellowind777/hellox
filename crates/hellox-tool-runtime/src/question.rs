use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_gateway_api::ToolDefinition;
use serde_json::{json, Value};

use crate::{LocalTool, LocalToolResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockingQuestion {
    pub question: String,
    pub header: Option<String>,
}

#[async_trait]
pub trait QuestionToolContext: Send + Sync {
    async fn ask_questions(&self, questions: &[BlockingQuestion]) -> Result<Vec<String>>;
}

pub struct AskUserQuestionTool;

#[async_trait]
impl<C> LocalTool<C> for AskUserQuestionTool
where
    C: QuestionToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "AskUserQuestion".to_string(),
            description: Some("Ask the user one or more blocking questions".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "items": {
                            "oneOf": [
                                { "type": "string" },
                                {
                                    "type": "object",
                                    "properties": {
                                        "question": { "type": "string" },
                                        "header": { "type": "string" }
                                    },
                                    "required": ["question"]
                                }
                            ]
                        }
                    }
                },
                "required": ["questions"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let questions = parse_questions(&input)?;
        let answers = context.ask_questions(&questions).await?;

        Ok(LocalToolResult::text(
            serde_json::to_string_pretty(&json!({
                "answers": answers,
            }))
            .context("failed to serialize AskUserQuestion result")?,
        ))
    }
}

fn parse_questions(input: &Value) -> Result<Vec<BlockingQuestion>> {
    let questions = input
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing required array field `questions`"))?;

    questions
        .iter()
        .map(|item| match item {
            Value::String(question) => Ok(BlockingQuestion {
                question: question.clone(),
                header: None,
            }),
            Value::Object(object) => {
                let question = object
                    .get("question")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("failed to parse question object"))?;
                let header = object
                    .get("header")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                Ok(BlockingQuestion {
                    question: question.to_string(),
                    header,
                })
            }
            _ => Err(anyhow!("questions must contain strings or objects")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_trait::async_trait;
    use hellox_gateway_api::ToolResultContent;
    use serde_json::json;

    use crate::LocalTool;

    use super::{AskUserQuestionTool, BlockingQuestion, QuestionToolContext};

    struct TestQuestionContext {
        answers: Vec<String>,
    }

    #[async_trait]
    impl QuestionToolContext for TestQuestionContext {
        async fn ask_questions(&self, questions: &[BlockingQuestion]) -> Result<Vec<String>> {
            assert_eq!(questions.len(), 2);
            assert_eq!(questions[0].question, "Continue?");
            assert_eq!(questions[1].header.as_deref(), Some("scope"));
            Ok(self.answers.clone())
        }
    }

    #[tokio::test]
    async fn ask_user_question_returns_context_answers() {
        let result = AskUserQuestionTool
            .call(
                json!({
                    "questions": [
                        "Continue?",
                        { "header": "scope", "question": "Need MCP next?" }
                    ]
                }),
                &TestQuestionContext {
                    answers: vec!["yes".to_string(), "rust".to_string()],
                },
            )
            .await
            .expect("ask questions");

        match result.content {
            ToolResultContent::Text(text) => {
                assert!(text.contains("\"yes\""), "{text}");
                assert!(text.contains("\"rust\""), "{text}");
            }
            other => panic!("expected text result, got {other:?}"),
        }
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn ask_user_question_rejects_invalid_question_shapes() {
        let error = AskUserQuestionTool
            .call(
                json!({
                    "questions": ["Continue?", 3]
                }),
                &TestQuestionContext {
                    answers: Vec::new(),
                },
            )
            .await
            .expect_err("invalid question must fail");
        assert!(error.to_string().contains("strings or objects"));
    }
}
