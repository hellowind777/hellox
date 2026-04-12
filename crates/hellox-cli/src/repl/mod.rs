mod bridge_actions;
mod commands;
mod config_actions;
mod core_actions;
mod core_copy;
mod core_paths;
mod dispatch;
mod extension_actions;
mod format;
mod format_copy;
mod help_copy;
mod install_actions;
mod mcp_actions;
pub(crate) mod output_localizer;
mod plan_actions;
mod plugin_actions;
mod prompt_input;
mod prompt_shell_copy;
mod remote_actions;
mod selector_input;
mod selectors;
mod style_actions;
mod task_actions;
mod ui_actions;
mod welcome_banner;
mod workflow_actions;
mod workflow_dashboard;
mod workflow_panel_shortcuts;
mod workflow_selectors;
mod workflow_support;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_bridge;
#[cfg(test)]
mod tests_diagnostics;
#[cfg(test)]
mod tests_extensions;
#[cfg(test)]
mod tests_install;
#[cfg(test)]
mod tests_mcp;
#[cfg(test)]
mod tests_memory;
#[cfg(test)]
mod tests_plugin;
#[cfg(test)]
mod tests_remote;
#[cfg(test)]
mod tests_search;
#[cfg(test)]
mod tests_state;
#[cfg(test)]
mod tests_style;
#[cfg(test)]
mod tests_tasks;
#[cfg(test)]
mod tests_ui;
#[cfg(test)]
mod tests_workflow;
#[cfg(test)]
mod tests_workflow_shortcuts;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use hellox_agent::AgentSession;
use hellox_repl::{run_repl_loop, ReplLoopDriver, ReplPromptState};
pub use hellox_repl::{ReplAction, ReplExit, ReplMetadata};
use hellox_tui::WorkflowDashboardState;

use crate::auto_compact::{format_auto_compact_notice, maybe_auto_compact_session};
use crate::auto_memory::{format_auto_memory_refresh_notice, maybe_auto_refresh_session_memory};
use crate::search::DEFAULT_SEARCH_LIMIT;
use crate::startup::{format_prompt_submission_error, resolve_app_language, AppLanguage};
use bridge_actions::{handle_bridge_command, handle_ide_command};
use commands::{
    parse_command, MemoryCommand, ReplCommand, SessionCommand, TaskCommand, WorkflowCommand,
};
use config_actions::handle_config_command;
use core_actions::{
    handle_compact_command, handle_memory_command, handle_model_command,
    handle_permissions_command, handle_resume_command, handle_rewind_command,
    handle_session_command, handle_share_command, ResumeAction,
};
use dispatch::handle_repl_input_async_impl;
use extension_actions::{handle_hooks_command, handle_skills_command};
use format::{
    cost_text, doctor_text, help_text_for_workdir, search_text, stats_text, status_text, usage_text,
};
use install_actions::{handle_install_command, handle_upgrade_command};
use mcp_actions::handle_mcp_command;
use plan_actions::handle_plan_command;
use plugin_actions::handle_plugin_command;
use remote_actions::{
    handle_assistant_command, handle_remote_env_command, handle_teleport_command,
};
use selector_input::parse_selector_index;
use selectors::SelectorContext;
use style_actions::{
    handle_output_style_command, handle_persona_command, handle_prompt_fragment_command,
};
use task_actions::handle_task_command;
use ui_actions::{handle_brief_command, handle_tools_command};
use workflow_actions::{handle_workflow_command, resolve_dynamic_workflow_invocation};
use workflow_selectors::{WorkflowPanelFocus, WorkflowRunFocus};

#[cfg(test)]
pub(crate) async fn handle_workflow_command_for_test(
    command: hellox_repl::WorkflowCommand,
    session: &mut AgentSession,
) -> Result<String> {
    workflow_actions::handle_workflow_command(command, session).await
}

pub async fn run_repl(
    session: &mut AgentSession,
    metadata: &ReplMetadata,
    workspace_trusted: bool,
) -> Result<ReplExit> {
    let driver =
        CliReplDriver::with_language(resolve_app_language(&metadata.config), workspace_trusted);
    run_repl_loop(session, metadata, &driver).await
}

#[derive(Debug, Default)]
struct CliReplDriver {
    language: AppLanguage,
    workspace_trusted: bool,
    submit_count: AtomicUsize,
    selector_context: Mutex<Option<SelectorContext>>,
    workflow_panel_focus: Mutex<Option<WorkflowPanelFocus>>,
    workflow_run_focus: Mutex<Option<WorkflowRunFocus>>,
    workflow_dashboard_state: Mutex<Option<WorkflowDashboardState>>,
}

impl CliReplDriver {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_language(AppLanguage::English, true)
    }

    fn with_language(language: AppLanguage, workspace_trusted: bool) -> Self {
        Self {
            language,
            workspace_trusted,
            ..Self::default()
        }
    }

    fn has_prior_submit(&self) -> bool {
        self.submit_count.load(Ordering::Relaxed) > 0
    }
}

#[async_trait]
impl ReplLoopDriver<AgentSession> for CliReplDriver {
    fn banner_lines(&self, session: &AgentSession) -> Vec<String> {
        welcome_banner::welcome_banner_lines(session, self.language, self.workspace_trusted)
    }

    fn prompt_label(&self, _session: &AgentSession, _metadata: &ReplMetadata) -> String {
        default_prompt_label()
    }

    fn prompt_state(&self, session: &AgentSession, metadata: &ReplMetadata) -> ReplPromptState {
        prompt_input::prompt_state(
            session,
            metadata,
            self.language,
            self.has_prior_submit(),
            self.workspace_trusted,
        )
    }

    async fn handle_input(
        &self,
        input: &str,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<ReplAction> {
        let action = self
            .handle_repl_input_async(input, session, metadata)
            .await?;
        if matches!(action, ReplAction::Submit(_)) {
            self.submit_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(action)
    }

    async fn handle_submit(
        &self,
        prompt: String,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<()> {
        let result = match session.run_user_prompt(prompt).await {
            Ok(result) => result,
            Err(error) => {
                println!(
                    "{}",
                    format_prompt_submission_error(
                        self.language,
                        &error,
                        &metadata.config,
                        session.model(),
                        Some(&metadata.config_path),
                    )
                );
                return Ok(());
            }
        };
        println!("{}", result.final_text);
        match maybe_auto_compact_session(session, &metadata.memory_root)? {
            Some(outcome) => println!("{}", format_auto_compact_notice(&outcome)),
            None => match maybe_auto_refresh_session_memory(session, &metadata.memory_root)? {
                Some(outcome) => println!("{}", format_auto_memory_refresh_notice(&outcome)),
                None => {}
            },
        }
        Ok(())
    }
}

fn default_prompt_label() -> String {
    String::from("╰─ ❯ ")
}

impl CliReplDriver {
    async fn handle_repl_input_async(
        &self,
        input: &str,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<ReplAction> {
        if self.handle_workflow_dashboard_input(input, session).await? {
            return Ok(ReplAction::Continue);
        }
        if let Some(index) = parse_selector_index(input) {
            if self.handle_selector_index(index, session, metadata).await? {
                return Ok(ReplAction::Continue);
            }
        }
        if self.handle_workflow_panel_shortcut(input, session).await? {
            return Ok(ReplAction::Continue);
        }
        self.clear_selector_context();

        handle_repl_input_async_impl(self, input, session, metadata).await
    }
}

#[cfg(test)]
fn handle_repl_input(
    input: &str,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
) -> Result<ReplAction> {
    let driver = CliReplDriver::new();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(driver.handle_repl_input_async(input, session, metadata))
}
