mod persistence;
mod prompt_state;
mod query_runtime;
mod system_prompt;
mod telemetry;
#[cfg(test)]
mod tests;
mod workspace_brief;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use hellox_compact::{compact_messages, CompactResult};
use hellox_config::{default_config_path, PermissionMode};
use hellox_gateway_api::{extract_text, Message, MessageContent, MessageRole, ThinkingConfig};
use hellox_query::{run_query_prompt, QueryTurnResult};

use crate::client::GatewayClient;
use crate::permissions::{ApprovalHandler, PermissionPolicy, QuestionHandler};
use crate::planning::PlanningState;
use crate::prompt::{
    build_default_system_prompt, OutputStylePrompt, PersonaPrompt, PromptFragment,
};
use crate::storage::StoredSession;
use crate::telemetry::{AgentTelemetryEvent, SharedTelemetrySink};
use crate::tools::{ToolExecutionContext, ToolRegistry};

#[derive(Clone, Debug)]
pub struct AgentOptions {
    pub model: String,
    pub max_turns: usize,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
    pub output_style: Option<OutputStylePrompt>,
    pub persona: Option<PersonaPrompt>,
    pub prompt_fragments: Vec<PromptFragment>,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            model: "opus".to_string(),
            max_turns: 12,
            max_tokens: Some(4_096),
            temperature: None,
            thinking: None,
            output_style: None,
            persona: None,
            prompt_fragments: Vec::new(),
        }
    }
}

pub type AgentTurnResult = QueryTurnResult;

pub struct AgentSession {
    client: GatewayClient,
    tools: ToolRegistry,
    context: ToolExecutionContext,
    options: AgentOptions,
    system_prompt: String,
    shell_name: String,
    output_style_name: Option<String>,
    persona_name: Option<String>,
    prompt_fragment_names: Vec<String>,
    messages: Vec<Message>,
    session_store: Option<StoredSession>,
    telemetry_sink: Option<SharedTelemetrySink>,
}

impl AgentSession {
    pub fn create(
        client: GatewayClient,
        tools: ToolRegistry,
        config_path: PathBuf,
        working_directory: PathBuf,
        shell_name: &str,
        options: AgentOptions,
        permission_mode: PermissionMode,
        approval_handler: Option<Arc<dyn ApprovalHandler>>,
        question_handler: Option<Arc<dyn QuestionHandler>>,
        persist: bool,
        session_id: Option<String>,
    ) -> Self {
        Self::create_with_telemetry(
            client,
            tools,
            config_path,
            working_directory,
            shell_name,
            options,
            permission_mode,
            approval_handler,
            question_handler,
            persist,
            session_id,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_with_telemetry(
        client: GatewayClient,
        tools: ToolRegistry,
        config_path: PathBuf,
        working_directory: PathBuf,
        shell_name: &str,
        options: AgentOptions,
        permission_mode: PermissionMode,
        approval_handler: Option<Arc<dyn ApprovalHandler>>,
        question_handler: Option<Arc<dyn QuestionHandler>>,
        persist: bool,
        session_id: Option<String>,
        telemetry_sink: Option<SharedTelemetrySink>,
    ) -> Self {
        let output_style_name = options
            .output_style
            .as_ref()
            .map(|style| style.name.clone());
        let persona_name = options.persona.as_ref().map(|persona| persona.name.clone());
        let prompt_fragment_names = options
            .prompt_fragments
            .iter()
            .map(|fragment| fragment.name.clone())
            .collect::<Vec<_>>();
        let system_prompt = build_default_system_prompt(
            &working_directory,
            shell_name,
            options.output_style.as_ref(),
            options.persona.as_ref(),
            &options.prompt_fragments,
        );
        let session_store = persist.then(|| {
            StoredSession::create(
                session_id,
                options.model.clone(),
                permission_mode.clone(),
                options.output_style.clone(),
                options.persona.clone(),
                options.prompt_fragments.clone(),
                &config_path,
                &working_directory,
                shell_name,
                system_prompt.clone(),
            )
        });
        Self {
            client: client.with_telemetry(telemetry_sink.clone()),
            tools,
            context: ToolExecutionContext {
                config_path,
                planning_state: Arc::new(Mutex::new(PlanningState::default())),
                permission_policy: PermissionPolicy::new(
                    permission_mode,
                    working_directory.clone(),
                ),
                approval_handler,
                question_handler,
                working_directory,
                telemetry_sink: telemetry_sink.clone(),
            },
            options,
            system_prompt,
            shell_name: shell_name.to_string(),
            output_style_name,
            persona_name,
            prompt_fragment_names,
            messages: Vec::new(),
            session_store,
            telemetry_sink,
        }
    }

    pub fn restore(
        client: GatewayClient,
        tools: ToolRegistry,
        options: AgentOptions,
        permission_mode: PermissionMode,
        approval_handler: Option<Arc<dyn ApprovalHandler>>,
        question_handler: Option<Arc<dyn QuestionHandler>>,
        session_store: StoredSession,
    ) -> Self {
        Self::restore_with_telemetry(
            client,
            tools,
            options,
            permission_mode,
            approval_handler,
            question_handler,
            session_store,
            None,
        )
    }

    pub fn restore_with_telemetry(
        client: GatewayClient,
        tools: ToolRegistry,
        mut options: AgentOptions,
        permission_mode: PermissionMode,
        approval_handler: Option<Arc<dyn ApprovalHandler>>,
        question_handler: Option<Arc<dyn QuestionHandler>>,
        mut session_store: StoredSession,
        telemetry_sink: Option<SharedTelemetrySink>,
    ) -> Self {
        let config_path = session_store
            .snapshot
            .config_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(default_config_path);
        let working_directory = PathBuf::from(&session_store.snapshot.working_directory);
        let restored_permission_mode = session_store
            .snapshot
            .permission_mode
            .clone()
            .unwrap_or(permission_mode);
        let restored_output_style_name = session_store
            .snapshot
            .output_style
            .as_ref()
            .map(|style| style.name.clone())
            .or_else(|| session_store.snapshot.output_style_name.clone());
        let restored_persona_name = session_store
            .snapshot
            .persona
            .as_ref()
            .map(|persona| persona.name.clone());
        let restored_prompt_fragment_names = session_store
            .snapshot
            .prompt_fragments
            .iter()
            .map(|fragment| fragment.name.clone())
            .collect::<Vec<_>>();
        options.output_style = prompt_state::restore_output_style(
            session_store.snapshot.output_style.clone(),
            session_store.snapshot.output_style_name.as_deref(),
            &working_directory,
        );
        options.persona = session_store.snapshot.persona.clone();
        options.prompt_fragments = session_store.snapshot.prompt_fragments.clone();
        session_store.snapshot.model = options.model.clone();
        session_store.snapshot.permission_mode = Some(restored_permission_mode.clone());
        session_store.snapshot.shell_name = session_store.snapshot.shell_name.clone();
        Self {
            client: client.with_telemetry(telemetry_sink.clone()),
            tools,
            context: ToolExecutionContext {
                config_path,
                planning_state: Arc::new(Mutex::new(session_store.snapshot.planning.clone())),
                permission_policy: PermissionPolicy::new(
                    restored_permission_mode,
                    working_directory.clone(),
                ),
                approval_handler,
                question_handler,
                working_directory,
                telemetry_sink: telemetry_sink.clone(),
            },
            system_prompt: session_store.snapshot.system_prompt.clone(),
            shell_name: session_store.snapshot.shell_name.clone(),
            output_style_name: restored_output_style_name,
            persona_name: restored_persona_name,
            prompt_fragment_names: restored_prompt_fragment_names,
            messages: session_store.restore_messages(),
            options,
            session_store: Some(session_store),
            telemetry_sink,
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_store
            .as_ref()
            .map(|session| session.session_id.as_str())
    }

    pub fn model(&self) -> &str {
        &self.options.model
    }

    pub fn permission_mode(&self) -> &PermissionMode {
        self.context.permission_policy.mode()
    }

    pub fn output_style_name(&self) -> Option<&str> {
        self.output_style_name.as_deref()
    }

    pub fn working_directory(&self) -> &Path {
        &self.context.working_directory
    }

    pub fn max_turns(&self) -> usize {
        self.options.max_turns
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn planning_state(&self) -> PlanningState {
        self.context.planning_state().unwrap_or_default()
    }

    pub fn set_planning_state(&mut self, planning: PlanningState) -> Result<()> {
        self.context.set_planning_state(planning)?;
        self.persist()
    }

    pub fn clear_messages(&mut self) -> Result<usize> {
        let cleared = self.messages.len();
        self.messages.clear();
        self.persist()?;
        Ok(cleared)
    }

    pub fn rewind_last_turn(&mut self) -> Result<usize> {
        let mut removed = 0;

        while let Some(message) = self.messages.pop() {
            removed += 1;
            if is_user_prompt_message(&message) {
                self.persist()?;
                return Ok(removed);
            }
        }

        if removed > 0 {
            self.persist()?;
        }

        Ok(removed)
    }

    /// Replace the current transcript with a compact summary message.
    pub fn compact(&mut self, instructions: Option<&str>) -> Result<CompactResult> {
        let result = compact_messages(&mut self.messages, instructions);
        self.persist()?;
        Ok(result)
    }

    pub async fn run_local_tool(&self, name: &str, input: serde_json::Value) -> Result<String> {
        let result = self.tools.execute(name, input, &self.context).await;
        self.emit_tool_event(name, result.is_error, &result.content);
        let text = match result.content {
            hellox_gateway_api::ToolResultContent::Text(text) => text,
            hellox_gateway_api::ToolResultContent::Blocks(blocks) => {
                extract_text(&MessageContent::Blocks(blocks))
            }
            hellox_gateway_api::ToolResultContent::Empty => String::new(),
        };

        if result.is_error {
            return Err(anyhow!(text));
        }

        Ok(text)
    }

    pub async fn run_user_prompt(&mut self, prompt: impl Into<String>) -> Result<AgentTurnResult> {
        let prompt = prompt.into();
        self.emit_telemetry(
            AgentTelemetryEvent::new("session", "prompt_submitted")
                .with_session_id(self.session_id())
                .with_attribute("model", self.options.model.clone())
                .with_attribute("prompt_chars", prompt.chars().count().to_string())
                .with_attribute("message_count_before", self.messages.len().to_string()),
        );

        self.maybe_inject_brief_attachments().await?;

        let result = run_query_prompt(self, prompt).await;
        match &result {
            Ok(result) => {
                self.emit_telemetry(
                    AgentTelemetryEvent::new("session", "turn_completed")
                        .with_session_id(self.session_id())
                        .with_attribute("model", self.options.model.clone())
                        .with_attribute("iterations", result.iterations.to_string())
                        .with_attribute(
                            "final_text_chars",
                            result.final_text.chars().count().to_string(),
                        ),
                );
            }
            Err(error) => {
                self.emit_telemetry(
                    AgentTelemetryEvent::new("session", "turn_failed")
                        .with_session_id(self.session_id())
                        .with_attribute("model", self.options.model.clone())
                        .with_attribute("error", error.to_string()),
                );
            }
        }
        result
    }
}

fn is_user_prompt_message(message: &Message) -> bool {
    matches!(message.role, MessageRole::User) && matches!(message.content, MessageContent::Text(_))
}
