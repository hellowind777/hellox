mod bridge_actions;
mod commands;
mod config_actions;
mod core_actions;
mod dispatch;
mod extension_actions;
mod format;
mod install_actions;
mod mcp_actions;
mod plan_actions;
mod plugin_actions;
mod remote_actions;
mod selectors;
mod style_actions;
mod task_actions;
mod ui_actions;
mod workflow_actions;

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

use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use hellox_agent::AgentSession;
use hellox_repl::{run_repl_loop, ReplLoopDriver};
pub use hellox_repl::{ReplAction, ReplExit, ReplMetadata};

use crate::auto_compact::{format_auto_compact_notice, maybe_auto_compact_session};
use crate::auto_memory::{format_auto_memory_refresh_notice, maybe_auto_refresh_session_memory};
use crate::search::DEFAULT_SEARCH_LIMIT;
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
use selectors::parse_selector_index;
use selectors::SelectorContext;
use style_actions::{
    handle_output_style_command, handle_persona_command, handle_prompt_fragment_command,
};
use task_actions::handle_task_command;
use ui_actions::{handle_brief_command, handle_tools_command};
use workflow_actions::{handle_workflow_command, resolve_dynamic_workflow_invocation};

pub async fn run_repl(session: &mut AgentSession, metadata: &ReplMetadata) -> Result<ReplExit> {
    let driver = CliReplDriver::new();
    run_repl_loop(session, metadata, &driver).await
}

#[derive(Debug, Default)]
struct CliReplDriver {
    selector_context: Mutex<Option<SelectorContext>>,
}

impl CliReplDriver {
    fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ReplLoopDriver<AgentSession> for CliReplDriver {
    fn banner_lines(&self, session: &AgentSession) -> Vec<String> {
        let mut lines = vec![
            String::from("hellox repl"),
            String::from("type `exit` or `/exit` to quit"),
        ];
        if let Some(session_id) = session.session_id() {
            lines.push(format!("session: {session_id}"));
        }
        lines
    }

    async fn handle_input(
        &self,
        input: &str,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<ReplAction> {
        self.handle_repl_input_async(input, session, metadata).await
    }

    async fn handle_submit(
        &self,
        prompt: String,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<()> {
        let result = session.run_user_prompt(prompt).await?;
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

impl CliReplDriver {
    async fn handle_repl_input_async(
        &self,
        input: &str,
        session: &mut AgentSession,
        metadata: &ReplMetadata,
    ) -> Result<ReplAction> {
        if let Some(index) = parse_selector_index(input) {
            if self.handle_selector_index(index, session, metadata).await? {
                return Ok(ReplAction::Continue);
            }
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
