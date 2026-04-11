use anyhow::Result;
use hellox_agent::AgentSession;
use hellox_tui::WorkflowDashboardState;

use super::*;
use crate::workflow_dashboard::{
    complete_workflow_dashboard_run,
    handle_workflow_dashboard_input as handle_dashboard_command_input,
    initial_workflow_dashboard_state, render_workflow_dashboard_state,
    WorkflowDashboardHandleOutcome,
};
use crate::workflow_runs::execute_and_record_workflow;

impl CliReplDriver {
    pub(super) fn workflow_dashboard_state(&self) -> Option<WorkflowDashboardState> {
        self.workflow_dashboard_state
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub(super) fn set_workflow_dashboard_state(&self, state: Option<WorkflowDashboardState>) {
        if let Ok(mut guard) = self.workflow_dashboard_state.lock() {
            *guard = state;
        }
    }

    pub(super) fn clear_workflow_dashboard_state(&self) {
        self.set_workflow_dashboard_state(None);
    }

    pub(super) fn open_workflow_dashboard(
        &self,
        session: &AgentSession,
        workflow_name: Option<String>,
        script_path: Option<String>,
    ) -> Result<String> {
        let mut state = initial_workflow_dashboard_state(workflow_name, script_path);
        let text = render_workflow_dashboard_state(session.working_directory(), &mut state)?;
        self.clear_selector_context();
        self.set_workflow_dashboard_state(Some(state));
        Ok(text)
    }

    pub(super) async fn handle_workflow_dashboard_input(
        &self,
        input: &str,
        session: &mut AgentSession,
    ) -> Result<bool> {
        let Some(mut state) = self.workflow_dashboard_state() else {
            return Ok(false);
        };

        match handle_dashboard_command_input(session.working_directory(), &mut state, input)? {
            WorkflowDashboardHandleOutcome::NotHandled => Ok(false),
            WorkflowDashboardHandleOutcome::Print(text) => {
                println!("{text}");
                self.set_workflow_dashboard_state(Some(state));
                Ok(true)
            }
            WorkflowDashboardHandleOutcome::RunActiveWorkflow {
                target,
                target_label,
                shared_context,
            } => {
                match execute_and_record_workflow(session, target.clone(), shared_context, None)
                    .await
                {
                    Ok(result_text) => println!(
                        "{}",
                        complete_workflow_dashboard_run(
                            session.working_directory(),
                            &mut state,
                            &target,
                            &target_label,
                            &result_text
                        )?
                    ),
                    Err(error) => println!("{error}"),
                }
                self.set_workflow_dashboard_state(Some(state));
                Ok(true)
            }
            WorkflowDashboardHandleOutcome::Close | WorkflowDashboardHandleOutcome::Quit => {
                self.clear_workflow_dashboard_state();
                println!("Closed workflow dashboard.");
                Ok(true)
            }
        }
    }
}
