use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{LocalTool, LocalToolResult, ToolExecutionContext, ToolRegistry};
use crate::permissions::UserQuestion;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register(AskUserQuestionTool);
}

struct AskUserQuestionTool;

#[async_trait]
impl LocalTool for AskUserQuestionTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
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

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        let questions = parse_questions(&input)?;
        let handler = context
            .question_handler
            .clone()
            .ok_or_else(|| anyhow!("question handler is not configured"))?;
        let answers = handler.ask_questions(&questions).await?;

        Ok(LocalToolResult::text(
            serde_json::to_string_pretty(&json!({
                "answers": answers,
            }))
            .context("failed to serialize AskUserQuestion result")?,
        ))
    }
}

fn parse_questions(input: &Value) -> Result<Vec<UserQuestion>> {
    let questions = input
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing required array field `questions`"))?;

    questions
        .iter()
        .map(|item| match item {
            Value::String(question) => Ok(UserQuestion {
                question: question.clone(),
                header: None,
            }),
            Value::Object(_) => serde_json::from_value::<UserQuestion>(item.clone())
                .context("failed to parse question object"),
            _ => Err(anyhow!("questions must contain strings or objects")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use async_trait::async_trait;
    use serde_json::json;
    use uuid::Uuid;

    use super::{parse_questions, AskUserQuestionTool, LocalTool, ToolExecutionContext};
    use crate::permissions::{PermissionPolicy, QuestionHandler, UserQuestion};
    use crate::planning::PlanningState;
    use hellox_config::PermissionMode;

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let root = env::temp_dir().join(format!("hellox-runtime-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("create temp workspace");
            Self { root }
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    struct StaticQuestionHandler {
        answers: Vec<String>,
    }

    #[async_trait]
    impl QuestionHandler for StaticQuestionHandler {
        async fn ask_questions(&self, _questions: &[UserQuestion]) -> Result<Vec<String>> {
            Ok(self.answers.clone())
        }
    }

    fn context(
        root: PathBuf,
        question_handler: Option<Arc<dyn QuestionHandler>>,
    ) -> ToolExecutionContext {
        ToolExecutionContext {
            config_path: root.join(".hellox").join("config.toml"),
            planning_state: Arc::new(Mutex::new(PlanningState::default())),
            working_directory: root.clone(),
            permission_policy: PermissionPolicy::new(PermissionMode::BypassPermissions, root),
            approval_handler: None,
            question_handler,
            telemetry_sink: None,
        }
    }

    #[tokio::test]
    async fn ask_user_question_returns_handler_answers() {
        let workspace = TestWorkspace::new();
        let handler: Arc<dyn QuestionHandler> = Arc::new(StaticQuestionHandler {
            answers: vec![String::from("yes"), String::from("rust")],
        });
        let ctx = context(workspace.root.clone(), Some(handler));

        let result = AskUserQuestionTool
            .call(
                json!({
                    "questions": [
                        "Continue?",
                        { "header": "stack", "question": "Which runtime?" }
                    ]
                }),
                &ctx,
            )
            .await
            .expect("ask questions");

        let output = match result.content {
            hellox_gateway_api::ToolResultContent::Text(text) => text,
            _ => panic!("expected text output"),
        };
        assert!(output.contains("\"yes\""), "{output}");
        assert!(output.contains("\"rust\""), "{output}");
    }

    #[test]
    fn question_parser_supports_strings_and_objects() {
        let questions = parse_questions(&json!({
            "questions": [
                "Continue?",
                { "header": "scope", "question": "Need MCP next?" }
            ]
        }))
        .expect("parse questions");

        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].question, "Continue?");
        assert_eq!(questions[1].header.as_deref(), Some("scope"));
    }
}
