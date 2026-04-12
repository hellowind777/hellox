use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::{ToolExecutionContext, ToolRegistry};
use super::background::{
    agent_status_value, cancelled_record, effective_background_record, list_background_records,
    store_background_record, take_abort_handle,
};
use super::native_pane_backend::{ITERM_BACKEND, TMUX_BACKEND};
use super::process_backend::terminate_backend_process;
use super::runtime_support::persisted_agent_statuses;
use super::shared::normalize_path;
use super::team::team_status_value;
use super::team_coordination_support::{
    persist_team_member_runtime_updates, persist_team_runtime_reconciliation,
    persist_team_runtime_value_for_session, resolve_team_members,
};

pub(super) use hellox_tools_agent::supervision_tool::{
    AgentListTool, AgentStopTool, TeamStopTool, TeamWaitTool,
};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register_runtime(AgentListTool);
    registry.register_runtime(AgentStopTool);
    registry.register_runtime(TeamWaitTool);
    registry.register_runtime(TeamStopTool);
}

#[async_trait]
impl hellox_tools_agent::supervision_tool::TeamSupervisionToolContext for ToolExecutionContext {
    fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    async fn persist_team_runtime_reconciliation(
        &self,
        requested_name: Option<&str>,
    ) -> Result<()> {
        persist_team_runtime_reconciliation(self, requested_name).await
    }

    fn list_background_agents_value(
        &self,
        include_completed: bool,
        include_persisted: bool,
        working_directory: Option<&std::path::Path>,
    ) -> Result<Value> {
        list_background_agents_value(include_completed, include_persisted, working_directory)
    }

    fn stop_background_agent_value(
        &self,
        session_id: &str,
        reason: Option<String>,
    ) -> Result<Value> {
        stop_background_agent_value(session_id, reason)
    }

    async fn persist_team_runtime_value_for_session(
        &self,
        session_id: &str,
        value: &Value,
    ) -> Result<()> {
        persist_team_runtime_value_for_session(self, session_id, value).await
    }

    async fn resolve_team_members(
        &self,
        team_name: &str,
        targets: Option<Vec<String>>,
    ) -> Result<Vec<hellox_tools_agent::team_storage::TeamMemberRecord>> {
        resolve_team_members(self, team_name, targets).await
    }

    async fn persist_team_member_runtime_updates(
        &self,
        team_name: &str,
        updated_members: &[hellox_tools_agent::team_storage::TeamMemberRecord],
    ) -> Result<()> {
        persist_team_member_runtime_updates(self, team_name, updated_members).await
    }

    fn team_status_value(&self, requested_name: Option<&str>) -> Result<Value> {
        team_status_value(self, requested_name)
    }

    fn agent_status_fallback(&self, session_id: &str) -> Value {
        safe_agent_status_value(session_id)
    }
}

pub(super) fn list_background_agents_value(
    include_completed: bool,
    include_persisted: bool,
    working_directory: Option<&std::path::Path>,
) -> Result<Value> {
    let working_directory = working_directory.map(normalize_path);
    let records = list_background_records()?
        .into_iter()
        .filter(|record| {
            working_directory
                .as_ref()
                .is_none_or(|path| &record.working_directory == path)
        })
        .collect::<Vec<_>>();
    let mut agents = records
        .iter()
        .map(|record| agent_status_value(&record.session_id))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|status| should_include_status(status, include_completed))
        .collect::<Vec<_>>();

    if include_persisted {
        let known_session_ids = records
            .iter()
            .map(|record| record.session_id.clone())
            .collect::<Vec<_>>();
        let persisted = persisted_agent_statuses(
            working_directory.as_deref().map(std::path::Path::new),
            &known_session_ids,
        )?;
        agents.extend(persisted);
    }

    Ok(json!({
        "agents": agents,
        "summary": summarize_background_agents(&agents),
    }))
}

pub(super) fn stop_background_agent_value(
    session_id: &str,
    reason: Option<String>,
) -> Result<Value> {
    let record = effective_background_record(session_id)?
        .ok_or_else(|| anyhow!("background agent `{session_id}` was not found"))?;
    if record.status != "running" {
        return Ok(json!({
            "session_id": session_id,
            "stopped": false,
            "agent": safe_agent_status_value(session_id),
        }));
    }

    if let Some(handle) = take_abort_handle(session_id)? {
        handle.abort();
    } else {
        terminate_backend_process(&record.backend, record.pid, record.pane_target.as_deref())?;
    }

    let mut cancelled = cancelled_record(
        &record,
        Some(
            reason
                .unwrap_or_else(|| "stopped by operator".to_string())
                .trim()
                .to_string(),
        ),
    );
    if matches!(record.backend.as_str(), TMUX_BACKEND | ITERM_BACKEND) {
        cancelled.pane_target = None;
    }
    store_background_record(cancelled)?;

    Ok(json!({
        "session_id": session_id,
        "stopped": true,
        "agent": safe_agent_status_value(session_id),
    }))
}

fn safe_agent_status_value(session_id: &str) -> Value {
    agent_status_value(session_id).unwrap_or_else(|error| {
        json!({
            "session_id": session_id,
            "status": "missing",
            "error": error.to_string(),
        })
    })
}

fn summarize_background_agents(records: &[Value]) -> Value {
    let mut running = 0_u64;
    let mut completed = 0_u64;
    let mut failed = 0_u64;
    let mut cancelled = 0_u64;
    let mut persisted = 0_u64;

    for record in records {
        match record
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
        {
            "running" => running += 1,
            "completed" => completed += 1,
            "failed" => failed += 1,
            "cancelled" => cancelled += 1,
            "persisted" => persisted += 1,
            _ => {}
        }
    }

    json!({
        "total_agents": records.len(),
        "running_agents": running,
        "completed_agents": completed,
        "failed_agents": failed,
        "cancelled_agents": cancelled,
        "persisted_agents": persisted,
        "overall_status": if running > 0 {
            "running"
        } else if failed > 0 {
            "failed"
        } else if persisted > 0 && completed + cancelled > 0 {
            "mixed"
        } else if persisted > 0 {
            "persisted"
        } else if cancelled > 0 && completed > 0 {
            "partial_cancelled"
        } else if cancelled > 0 {
            "cancelled"
        } else if completed > 0 {
            "completed"
        } else {
            "idle"
        },
    })
}

fn should_include_status(status: &Value, include_completed: bool) -> bool {
    if include_completed {
        return true;
    }

    matches!(
        status.get("status").and_then(Value::as_str),
        Some("running" | "persisted")
    )
}
