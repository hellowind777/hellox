use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};

use crate::shared::{optional_string, render_json};
use crate::team_runtime_support::{parse_targets, updated_member_from_agent_value};
use crate::team_storage::TeamMemberRecord;

#[async_trait]
pub trait TeamSupervisionToolContext {
    fn working_directory(&self) -> &Path;

    async fn persist_team_runtime_reconciliation(&self, requested_name: Option<&str>)
        -> Result<()>;

    fn list_background_agents_value(
        &self,
        include_completed: bool,
        include_persisted: bool,
        working_directory: Option<&Path>,
    ) -> Result<Value>;

    fn stop_background_agent_value(
        &self,
        session_id: &str,
        reason: Option<String>,
    ) -> Result<Value>;

    async fn persist_team_runtime_value_for_session(
        &self,
        session_id: &str,
        value: &Value,
    ) -> Result<()>;

    async fn resolve_team_members(
        &self,
        team_name: &str,
        targets: Option<Vec<String>>,
    ) -> Result<Vec<TeamMemberRecord>>;

    async fn persist_team_member_runtime_updates(
        &self,
        team_name: &str,
        updated_members: &[TeamMemberRecord],
    ) -> Result<()>;

    fn team_status_value(&self, requested_name: Option<&str>) -> Result<Value>;

    fn agent_status_fallback(&self, session_id: &str) -> Value;
}

pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TeamSupervisionToolContext + Send + Sync + 'static,
{
    registry.register(AgentListTool);
    registry.register(AgentStopTool);
    registry.register(TeamWaitTool);
    registry.register(TeamStopTool);
}

pub struct AgentListTool;
pub struct AgentStopTool;
pub struct TeamWaitTool;
pub struct TeamStopTool;

#[async_trait]
impl<C> LocalTool<C> for AgentListTool
where
    C: TeamSupervisionToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "AgentList".to_string(),
            description: Some("List supervised local background agent sessions.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "include_completed": { "type": "boolean" },
                    "include_persisted": { "type": "boolean" },
                    "all_workspaces": { "type": "boolean" }
                }
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let include_completed = input
            .get("include_completed")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let include_persisted = input
            .get("include_persisted")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let all_workspaces = input
            .get("all_workspaces")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !all_workspaces {
            context.persist_team_runtime_reconciliation(None).await?;
        }
        Ok(LocalToolResult::text(render_json(
            context.list_background_agents_value(
                include_completed,
                include_persisted,
                (!all_workspaces).then_some(context.working_directory()),
            )?,
        )?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for AgentStopTool
where
    C: TeamSupervisionToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "AgentStop".to_string(),
            description: Some("Stop a running local background agent session.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "reason": { "type": "string" }
                },
                "required": ["session_id"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let session_id = required_string(&input, "session_id")?;
        let reason = optional_string(&input, "reason");
        let stopped = context.stop_background_agent_value(session_id, reason)?;
        if let Some(agent_value) = stopped.get("agent") {
            context
                .persist_team_runtime_value_for_session(session_id, agent_value)
                .await?;
        }
        Ok(LocalToolResult::text(render_json(stopped)?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TeamWaitTool
where
    C: TeamSupervisionToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TeamWait".to_string(),
            description: Some(
                "Wait for all members in a local team to leave the running state.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "timeout_ms": { "type": "integer", "minimum": 1 },
                    "poll_interval_ms": { "type": "integer", "minimum": 1 }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let team_name = required_string(&input, "name")?;
        let timeout_ms = input
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(30_000);
        let poll_interval_ms = input
            .get("poll_interval_ms")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        let started = SystemTime::now();

        loop {
            context
                .persist_team_runtime_reconciliation(Some(team_name))
                .await?;
            let status = context.team_status_value(Some(team_name))?;
            let running_members = status["teams"][0]["summary"]["running_members"]
                .as_u64()
                .unwrap_or(0);
            if running_members == 0 {
                return Ok(LocalToolResult::text(render_json(status)?));
            }

            if started.elapsed().unwrap_or_default() >= Duration::from_millis(timeout_ms) {
                return Err(anyhow!("timed out waiting for team `{team_name}`"));
            }

            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
        }
    }
}

#[async_trait]
impl<C> LocalTool<C> for TeamStopTool
where
    C: TeamSupervisionToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TeamStop".to_string(),
            description: Some("Stop running members in a local team.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "targets": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "reason": { "type": "string" }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let team_name = required_string(&input, "name")?;
        let targets = parse_targets(input.get("targets"))?;
        let members = context.resolve_team_members(team_name, targets).await?;
        let reason = optional_string(&input, "reason");

        let mut actions = Vec::with_capacity(members.len());
        let mut updated_members = Vec::new();
        for member in members {
            let action = context
                .stop_background_agent_value(&member.session_id, reason.clone())
                .unwrap_or_else(|_| {
                    json!({
                        "session_id": member.session_id,
                        "stopped": false,
                        "agent": context.agent_status_fallback(&member.session_id),
                    })
                });
            if let Some(agent_value) = action.get("agent") {
                updated_members.push(updated_member_from_agent_value(&member, agent_value));
            }
            actions.push(json!({
                "name": member.name,
                "action": action,
            }));
        }

        context
            .persist_team_member_runtime_updates(team_name, &updated_members)
            .await?;

        let team_status = context.team_status_value(Some(team_name))?;
        Ok(LocalToolResult::text(render_json(json!({
            "team_name": team_name,
            "actions": actions,
            "teams": team_status["teams"].clone(),
            "summary": team_status["summary"].clone(),
        }))?))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use hellox_gateway_api::ToolResultContent;

    use super::*;

    #[derive(Clone)]
    struct TestContext {
        working_directory: PathBuf,
        stopped: Arc<Mutex<Vec<String>>>,
        persisted: Arc<Mutex<Vec<String>>>,
        members: Vec<TeamMemberRecord>,
    }

    #[async_trait]
    impl TeamSupervisionToolContext for TestContext {
        fn working_directory(&self) -> &Path {
            &self.working_directory
        }

        async fn persist_team_runtime_reconciliation(
            &self,
            _requested_name: Option<&str>,
        ) -> Result<()> {
            Ok(())
        }

        fn list_background_agents_value(
            &self,
            _include_completed: bool,
            _include_persisted: bool,
            _working_directory: Option<&Path>,
        ) -> Result<Value> {
            Ok(json!({
                "agents": [{ "session_id": "agent-1", "status": "running" }],
                "summary": { "running_agents": 1 }
            }))
        }

        fn stop_background_agent_value(
            &self,
            session_id: &str,
            _reason: Option<String>,
        ) -> Result<Value> {
            self.stopped
                .lock()
                .expect("lock stopped")
                .push(session_id.to_string());
            Ok(json!({
                "session_id": session_id,
                "stopped": true,
                "agent": {
                    "session_id": session_id,
                    "status": "cancelled"
                }
            }))
        }

        async fn persist_team_runtime_value_for_session(
            &self,
            session_id: &str,
            _value: &Value,
        ) -> Result<()> {
            self.persisted
                .lock()
                .expect("lock persisted")
                .push(session_id.to_string());
            Ok(())
        }

        async fn resolve_team_members(
            &self,
            _team_name: &str,
            _targets: Option<Vec<String>>,
        ) -> Result<Vec<TeamMemberRecord>> {
            Ok(self.members.clone())
        }

        async fn persist_team_member_runtime_updates(
            &self,
            _team_name: &str,
            updated_members: &[TeamMemberRecord],
        ) -> Result<()> {
            self.persisted.lock().expect("lock persisted").extend(
                updated_members
                    .iter()
                    .map(|member| member.session_id.clone()),
            );
            Ok(())
        }

        fn team_status_value(&self, _requested_name: Option<&str>) -> Result<Value> {
            Ok(json!({
                "teams": [{ "summary": { "running_members": 0 } }],
                "summary": { "total_teams": 1 }
            }))
        }

        fn agent_status_fallback(&self, session_id: &str) -> Value {
            json!({
                "session_id": session_id,
                "status": "missing"
            })
        }
    }

    #[tokio::test]
    async fn agent_stop_tool_persists_runtime_value() {
        let context = TestContext {
            working_directory: PathBuf::from("D:/workspace"),
            stopped: Arc::new(Mutex::new(Vec::new())),
            persisted: Arc::new(Mutex::new(Vec::new())),
            members: Vec::new(),
        };

        let result = AgentStopTool
            .call(json!({ "session_id": "agent-1" }), &context)
            .await
            .expect("agent stop");

        let text = match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse result");
        assert_eq!(value["stopped"].as_bool(), Some(true));
        assert_eq!(
            context.persisted.lock().expect("lock persisted").as_slice(),
            ["agent-1"]
        );
    }

    #[tokio::test]
    async fn team_stop_tool_updates_member_runtime_metadata() {
        let context = TestContext {
            working_directory: PathBuf::from("D:/workspace"),
            stopped: Arc::new(Mutex::new(Vec::new())),
            persisted: Arc::new(Mutex::new(Vec::new())),
            members: vec![TeamMemberRecord {
                name: "alice".to_string(),
                session_id: "agent-1".to_string(),
                backend: Some("in_process".to_string()),
                layout_slot: Some("primary".to_string()),
                pane_target: None,
            }],
        };

        let result = TeamStopTool
            .call(json!({ "name": "builders" }), &context)
            .await
            .expect("team stop");

        let text = match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse result");
        assert_eq!(
            value["actions"][0]["action"]["stopped"].as_bool(),
            Some(true)
        );
        assert_eq!(
            context.stopped.lock().expect("lock stopped").as_slice(),
            ["agent-1"]
        );
    }
}
