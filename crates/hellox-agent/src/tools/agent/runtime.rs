use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_config::{load_or_default, session_file_path};
use serde_json::{json, Value};

use crate::{default_tool_registry, AgentOptions, AgentSession, GatewayClient, StoredSession};

use super::super::{ToolExecutionContext, ToolRegistry};
use super::background::{
    agent_status_value, clear_abort_handle, completed_record, failed_record, is_running_session,
    register_abort_handle, running_record, store_background_record,
};
use super::process_backend::{
    launch_process_backend_agent, resolve_backend, AgentBackend, ProcessLaunchOptions,
};
use super::shared::{current_shell_name, AgentRunRequest};
use super::team_coordination_support::reconcile_team_runtime_for_session;
use super::worktree_support::resolve_child_working_directory;

pub(super) use hellox_tools_agent::runtime_tool::{AgentStatusTool, AgentTool, AgentWaitTool};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(AgentTool);
    registry.register_runtime(AgentStatusTool);
    registry.register_runtime(AgentWaitTool);
}

#[async_trait]
impl hellox_tools_agent::runtime_tool::AgentRuntimeToolContext for ToolExecutionContext {
    async fn run_agent_prompt(
        &self,
        request: hellox_tools_agent::shared::AgentRunRequest,
    ) -> Result<Value> {
        run_agent_prompt(self, request).await
    }

    async fn reconcile_team_runtime_for_session(&self, session_id: &str) -> Result<()> {
        reconcile_team_runtime_for_session(self, session_id).await
    }

    fn agent_status_value(&self, session_id: &str) -> Result<Value> {
        agent_status_value(session_id)
    }
}

pub(super) async fn run_agent_prompt(
    context: &ToolExecutionContext,
    request: AgentRunRequest,
) -> Result<Value> {
    if let Some(existing_session_id) = request.session_id.as_deref() {
        if is_running_session(existing_session_id)? {
            return Err(anyhow!(
                "agent session `{existing_session_id}` is already running"
            ));
        }
    }

    let backend = resolve_backend(request.backend.as_deref(), request.run_in_background)?;
    let (mut session, session_id, resumed) = build_child_session(
        context,
        request.model,
        request.permission_mode,
        request.cwd.as_deref(),
        request.isolation.as_deref(),
        request.worktree_name.as_deref(),
        request.worktree_base_ref.as_deref(),
        request.reuse_existing_worktree,
        request.agent_name.as_deref(),
        request.max_turns,
        request.session_id,
        request.allow_interaction,
    )?;

    if request.run_in_background {
        session.persist_now()?;
        let running = match backend {
            AgentBackend::InProcess => {
                let running = running_record(
                    &session,
                    &session_id,
                    resumed,
                    true,
                    "in_process",
                    None,
                    None,
                    request.layout_slot.clone(),
                );
                store_background_record(running.clone())?;
                let prompt = request.prompt;
                let background_session_id = session_id.clone();
                let layout_slot = request.layout_slot.clone();
                let task = tokio::spawn(async move {
                    let outcome = session.run_user_prompt(prompt).await;
                    let updated = match outcome {
                        Ok(result) => completed_record(
                            &session,
                            &background_session_id,
                            resumed,
                            true,
                            "in_process",
                            None,
                            None,
                            layout_slot.clone(),
                            result.iterations,
                            result.final_text,
                        ),
                        Err(error) => failed_record(
                            &session,
                            &background_session_id,
                            resumed,
                            true,
                            "in_process",
                            None,
                            None,
                            layout_slot,
                            error.to_string(),
                        ),
                    };
                    let _ = clear_abort_handle(&background_session_id);
                    let _ = store_background_record(updated);
                });
                register_abort_handle(&session_id, task.abort_handle())?;
                running
            }
            AgentBackend::DetachedProcess | AgentBackend::TmuxPane | AgentBackend::ITermPane => {
                launch_process_backend_agent(
                    backend,
                    &session,
                    &session_id,
                    ProcessLaunchOptions {
                        prompt: request.prompt,
                        max_turns: request.max_turns,
                        resumed,
                        config_path: &context.config_path,
                        agent_name: request.agent_name.as_deref(),
                        pane_group: request.pane_group.as_deref(),
                        layout_strategy: request.layout_strategy.as_deref(),
                        layout_slot: request.layout_slot.as_deref(),
                        pane_anchor_target: request.pane_anchor_target.as_deref(),
                    },
                )?
            }
        };

        return Ok(json!({
            "session_id": running.session_id,
            "status": running.status,
            "background": running.background,
            "resumed": running.resumed,
            "backend": running.backend,
            "pid": running.pid,
            "pane_target": running.pane_target,
            "layout_slot": running.layout_slot,
            "model": running.model,
            "working_directory": running.working_directory,
        }));
    }

    let result = session.run_user_prompt(request.prompt).await?;
    let completed = completed_record(
        &session,
        &session_id,
        resumed,
        false,
        "in_process",
        None,
        None,
        request.layout_slot.clone(),
        result.iterations,
        result.final_text.clone(),
    );
    store_background_record(completed.clone())?;
    Ok(json!({
        "session_id": completed.session_id,
        "status": completed.status,
        "background": completed.background,
        "resumed": completed.resumed,
        "backend": completed.backend,
        "pid": completed.pid,
        "pane_target": completed.pane_target,
        "layout_slot": completed.layout_slot,
        "model": completed.model,
        "working_directory": completed.working_directory,
        "iterations": completed.iterations,
        "result": completed.result,
    }))
}

pub(super) fn build_child_session(
    context: &ToolExecutionContext,
    model_override: Option<String>,
    permission_mode_override: Option<hellox_config::PermissionMode>,
    cwd_override: Option<&str>,
    isolation: Option<&str>,
    worktree_name: Option<&str>,
    worktree_base_ref: Option<&str>,
    reuse_existing_worktree: bool,
    agent_name: Option<&str>,
    max_turns: usize,
    session_id: Option<String>,
    allow_interaction: bool,
) -> Result<(AgentSession, String, bool)> {
    let config = load_or_default(Some(context.config_path.clone()))?;
    let telemetry_sink = context.telemetry_sink.clone();
    let gateway = GatewayClient::from_config(&config, None).with_telemetry(telemetry_sink.clone());
    let approval_handler = if allow_interaction {
        context.approval_handler.clone()
    } else {
        None
    };
    let question_handler = if allow_interaction {
        context.question_handler.clone()
    } else {
        None
    };

    if let Some(existing_session_id) = session_id.as_deref() {
        if session_file_path(existing_session_id).exists() {
            if cwd_override.is_some() || isolation.is_some() {
                return Err(anyhow!(
                    "cannot override `cwd` or `isolation` while resuming an existing agent session"
                ));
            }

            let mut stored = StoredSession::load(existing_session_id)?;
            if let Some(mode) = permission_mode_override.as_ref() {
                stored.snapshot.permission_mode = Some(mode.clone());
            }
            let options = AgentOptions {
                model: model_override.unwrap_or_else(|| stored.snapshot.model.clone()),
                max_turns,
                ..AgentOptions::default()
            };
            let session = AgentSession::restore_with_telemetry(
                gateway,
                default_tool_registry(),
                options,
                permission_mode_override
                    .clone()
                    .unwrap_or_else(|| context.permission_policy.mode().clone()),
                approval_handler,
                question_handler,
                stored,
                telemetry_sink,
            );
            return Ok((session, existing_session_id.to_string(), true));
        }
    }

    let working_directory = resolve_child_working_directory(
        context,
        cwd_override,
        isolation,
        worktree_name,
        worktree_base_ref,
        reuse_existing_worktree,
        agent_name,
    )?;
    let options = AgentOptions {
        app_language: config.ui.language.clone().or_else(sys_locale::get_locale),
        output_style: hellox_style::resolve_configured_output_style(&config, &working_directory)?,
        persona: hellox_style::resolve_configured_persona(&config, &working_directory)?,
        prompt_fragments: hellox_style::resolve_configured_fragments(&config, &working_directory)?,
        model: model_override.unwrap_or_else(|| config.session.model.clone()),
        max_turns,
        ..AgentOptions::default()
    };
    let session = AgentSession::create_with_telemetry(
        gateway,
        default_tool_registry(),
        context.config_path.clone(),
        working_directory,
        &current_shell_name(),
        options,
        permission_mode_override.unwrap_or_else(|| context.permission_policy.mode().clone()),
        approval_handler,
        question_handler,
        true,
        session_id,
        telemetry_sink,
    );
    let actual_session_id = session
        .session_id()
        .ok_or_else(|| anyhow!("child agent session is missing a session id"))?
        .to_string();
    Ok((session, actual_session_id, false))
}
