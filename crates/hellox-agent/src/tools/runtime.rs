use anyhow::{anyhow, Result};
use async_trait::async_trait;

use super::{ToolExecutionContext, ToolRegistry};
use crate::permissions::UserQuestion;

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(hellox_tool_runtime::AskUserQuestionTool);
}

#[async_trait]
impl hellox_tool_runtime::QuestionToolContext for ToolExecutionContext {
    async fn ask_questions(
        &self,
        questions: &[hellox_tool_runtime::BlockingQuestion],
    ) -> Result<Vec<String>> {
        let handler = self
            .question_handler
            .clone()
            .ok_or_else(|| anyhow!("question handler is not configured"))?;
        let questions = questions
            .iter()
            .map(|question| UserQuestion {
                question: question.question.clone(),
                header: question.header.clone(),
            })
            .collect::<Vec<_>>();
        handler.ask_questions(&questions).await
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use async_trait::async_trait;
    use hellox_gateway_api::ToolResultContent;
    use serde_json::json;
    use uuid::Uuid;

    use super::ToolExecutionContext;
    use crate::permissions::{PermissionPolicy, QuestionHandler, UserQuestion};
    use crate::planning::PlanningState;
    use crate::tools::default_tool_registry;
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
    async fn ask_user_question_bridge_returns_handler_answers() {
        let workspace = TestWorkspace::new();
        let handler: Arc<dyn QuestionHandler> = Arc::new(StaticQuestionHandler {
            answers: vec![String::from("yes"), String::from("rust")],
        });
        let ctx = context(workspace.root.clone(), Some(handler));

        let result = default_tool_registry()
            .execute(
                "AskUserQuestion",
                json!({
                    "questions": [
                        "Continue?",
                        { "header": "stack", "question": "Which runtime?" }
                    ]
                }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);

        let output = match result.content {
            ToolResultContent::Text(text) => text,
            _ => panic!("expected text output"),
        };
        assert!(output.contains("\"yes\""), "{output}");
        assert!(output.contains("\"rust\""), "{output}");
    }
}
