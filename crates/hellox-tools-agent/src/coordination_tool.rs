use tokio::task::JoinSet;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};

use crate::shared::{optional_string, render_json, AgentRunRequest};
use crate::team_runtime_support::{
    follow_up_backend, parse_targets, resolve_member_pane_anchor_target, resolve_team_pane_group,
    updated_member_from_agent_value,
};
use crate::team_storage::{TeamMemberRecord, TeamRecord};

#[derive(Clone)]
pub struct ResolvedTeamSelection {
    pub team: TeamRecord,
    pub members: Vec<TeamMemberRecord>,
}

#[async_trait]
pub trait TeamCoordinationToolContext {
    async fn resolve_team_selection(
        &self,
        team_name: &str,
        targets: Option<Vec<String>>,
    ) -> Result<ResolvedTeamSelection>;

    async fn persist_team_member_runtime_updates(
        &self,
        team_name: &str,
        updated_members: &[TeamMemberRecord],
    ) -> Result<()>;

    async fn run_agent_prompt(&self, request: AgentRunRequest) -> Result<Value>;
}

pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TeamCoordinationToolContext + Send + Sync + Clone + 'static,
{
    registry.register(SendMessageTool);
    registry.register(TeamRunTool);
}

pub struct SendMessageTool;
pub struct TeamRunTool;

#[async_trait]
impl<C> LocalTool<C> for SendMessageTool
where
    C: TeamCoordinationToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "SendMessage".to_string(),
            description: Some(
                "Send a follow-up message to a local agent session or teammate.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string" },
                    "content": { "type": "string" },
                    "team_name": { "type": "string" },
                    "backend": { "type": "string" },
                    "max_turns": { "type": "integer", "minimum": 1, "maximum": 64 },
                    "run_in_background": { "type": "boolean" }
                },
                "required": ["to", "content"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let to = required_string(&input, "to")?.trim();
        let content = required_string(&input, "content")?.trim().to_string();
        if content.is_empty() {
            return Err(anyhow!("message content cannot be empty"));
        }
        let run_in_background = input
            .get("run_in_background")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let requested_backend = optional_string(&input, "backend");
        let team_name = optional_string(&input, "team_name");
        let mut team_update = None;
        let (
            session_id,
            backend,
            agent_name,
            pane_group,
            layout_strategy,
            layout_slot,
            pane_anchor_target,
        ) = match team_name.as_deref() {
            Some(team_name) => {
                let selection = context
                    .resolve_team_selection(team_name, Some(vec![to.to_string()]))
                    .await?;
                let member = selection
                    .members
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("team member `{to}` was not found"))?;
                let pane_anchor_target = if run_in_background {
                    resolve_member_pane_anchor_target(&selection.team, &member)
                } else {
                    None
                };
                team_update = Some((team_name.to_string(), member.clone()));
                (
                    member.session_id,
                    follow_up_backend(
                        requested_backend.clone(),
                        member.backend.clone(),
                        run_in_background,
                    ),
                    Some(member.name),
                    Some(resolve_team_pane_group(&selection.team)),
                    Some(selection.team.layout.strategy),
                    member.layout_slot,
                    pane_anchor_target,
                )
            }
            None => (
                to.to_string(),
                requested_backend,
                None,
                None,
                None,
                None,
                None,
            ),
        };

        let value = context
            .run_agent_prompt(AgentRunRequest {
                prompt: content,
                model: None,
                backend,
                permission_mode: None,
                agent_name,
                pane_group,
                layout_strategy,
                layout_slot,
                pane_anchor_target,
                cwd: None,
                session_id: Some(session_id),
                max_turns: input
                    .get("max_turns")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(8),
                run_in_background,
                allow_interaction: false,
            })
            .await?;

        if let Some((team_name, member)) = team_update {
            let updated_member = updated_member_from_agent_value(&member, &value);
            context
                .persist_team_member_runtime_updates(&team_name, &[updated_member])
                .await?;
        }

        Ok(LocalToolResult::text(render_json(value)?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TeamRunTool
where
    C: TeamCoordinationToolContext + Send + Sync + Clone + 'static,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TeamRun".to_string(),
            description: Some(
                "Run a prompt across local teammates in parallel and optionally aggregate the results."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "team_name": { "type": "string" },
                    "prompt": { "type": "string" },
                    "targets": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "backend": { "type": "string" },
                    "max_turns": { "type": "integer", "minimum": 1, "maximum": 64 },
                    "run_in_background": { "type": "boolean" },
                    "continue_on_error": { "type": "boolean" },
                    "coordinator_prompt": { "type": "string" },
                    "coordinator_model": { "type": "string" },
                    "coordinator_backend": { "type": "string" },
                    "coordinator_session_id": { "type": "string" },
                    "coordinator_max_turns": { "type": "integer", "minimum": 1, "maximum": 64 }
                },
                "required": ["team_name", "prompt"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let team_name = required_string(&input, "team_name")?.trim().to_string();
        if team_name.is_empty() {
            return Err(anyhow!("team_name cannot be empty"));
        }

        let prompt = required_string(&input, "prompt")?.trim().to_string();
        if prompt.is_empty() {
            return Err(anyhow!("team_run prompt cannot be empty"));
        }

        let selection = context
            .resolve_team_selection(&team_name, parse_targets(input.get("targets"))?)
            .await?;
        let team = selection.team.clone();
        let pane_group = resolve_team_pane_group(&team);
        let members = selection.members;
        let max_turns = input
            .get("max_turns")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(8);
        let run_in_background = input
            .get("run_in_background")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let continue_on_error = input
            .get("continue_on_error")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let requested_backend = optional_string(&input, "backend");
        let layout_strategy = team.layout.strategy.clone();

        let mut join_set = JoinSet::new();
        let mut updated_members = Vec::new();
        for member in members {
            let context = context.clone();
            let prompt = prompt.clone();
            let layout_strategy = layout_strategy.clone();
            let pane_anchor_target = if run_in_background {
                resolve_member_pane_anchor_target(&team, &member)
            } else {
                None
            };
            let backend = follow_up_backend(
                requested_backend.clone(),
                member.backend.clone(),
                run_in_background,
            );
            let pane_group = pane_group.clone();
            join_set.spawn(async move {
                let layout_slot = member.layout_slot.clone();
                let member_name = member.name.clone();
                let result = context
                    .run_agent_prompt(AgentRunRequest {
                        prompt,
                        model: None,
                        backend,
                        permission_mode: None,
                        agent_name: Some(member_name),
                        pane_group: Some(pane_group),
                        layout_strategy: Some(layout_strategy),
                        layout_slot,
                        pane_anchor_target,
                        cwd: None,
                        session_id: Some(member.session_id.clone()),
                        max_turns,
                        run_in_background,
                        allow_interaction: false,
                    })
                    .await;
                (member, result)
            });
        }

        let mut member_results = Vec::new();
        let mut failures = Vec::new();
        while let Some(joined) = join_set.join_next().await {
            let (member, result) =
                joined.map_err(|error| anyhow!("team worker failed: {error}"))?;
            match result {
                Ok(value) => {
                    updated_members.push(updated_member_from_agent_value(&member, &value));
                    member_results.push(json!({
                        "name": member.name,
                        "session_id": member.session_id,
                        "result": value,
                    }));
                }
                Err(error) => {
                    failures.push(json!({
                        "name": member.name,
                        "session_id": member.session_id,
                        "error": error.to_string(),
                    }));
                }
            }
        }
        context
            .persist_team_member_runtime_updates(&team_name, &updated_members)
            .await?;

        let mut overall_status = if failures.is_empty() {
            if run_in_background {
                "running".to_string()
            } else {
                "completed".to_string()
            }
        } else if continue_on_error && !member_results.is_empty() {
            "partial_failure".to_string()
        } else {
            "failed".to_string()
        };

        let coordinator = if failures.is_empty() && !run_in_background {
            maybe_run_coordinator(
                context,
                &input,
                &team_name,
                &pane_group,
                &prompt,
                &member_results,
            )
            .await?
        } else {
            None
        };
        if coordinator.is_some() && overall_status == "completed" {
            overall_status = "coordinated".to_string();
        }

        Ok(LocalToolResult::text(render_json(json!({
            "status": overall_status,
            "team_name": team_name,
            "members": member_results,
            "failures": failures,
            "coordinator": coordinator,
        }))?))
    }
}

async fn maybe_run_coordinator(
    context: &impl TeamCoordinationToolContext,
    input: &Value,
    team_name: &str,
    pane_group: &str,
    original_prompt: &str,
    member_results: &[Value],
) -> Result<Option<Value>> {
    let coordinator_prompt = match optional_string(input, "coordinator_prompt") {
        Some(prompt) => prompt,
        None => return Ok(None),
    };
    let coordinator_input = format!(
        "{coordinator_prompt}\n\nTeam: {team_name}\nOriginal prompt: {original_prompt}\n\nMember results:\n{}",
        serde_json::to_string_pretty(member_results)?
    );
    let result = context
        .run_agent_prompt(AgentRunRequest {
            prompt: coordinator_input,
            model: optional_string(input, "coordinator_model"),
            backend: optional_string(input, "coordinator_backend"),
            permission_mode: None,
            agent_name: Some(format!("{team_name}-coordinator")),
            pane_group: Some(pane_group.to_string()),
            layout_strategy: None,
            layout_slot: Some("primary".to_string()),
            pane_anchor_target: None,
            cwd: None,
            session_id: optional_string(input, "coordinator_session_id"),
            max_turns: input
                .get("coordinator_max_turns")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .unwrap_or(8),
            run_in_background: false,
            allow_interaction: false,
        })
        .await?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use hellox_gateway_api::ToolResultContent;

    use super::*;
    use crate::team_storage::TeamLayoutRecord;

    #[derive(Clone)]
    struct TestContext {
        team: TeamRecord,
        responses: Arc<Mutex<BTreeMap<String, std::result::Result<Value, String>>>>,
        persisted_updates: Arc<Mutex<Vec<(String, Vec<TeamMemberRecord>)>>>,
    }

    #[async_trait]
    impl TeamCoordinationToolContext for TestContext {
        async fn resolve_team_selection(
            &self,
            team_name: &str,
            targets: Option<Vec<String>>,
        ) -> Result<ResolvedTeamSelection> {
            if self.team.name != team_name {
                return Err(anyhow!("team `{team_name}` was not found"));
            }
            let members = match targets {
                Some(targets) => self
                    .team
                    .members
                    .iter()
                    .filter(|member| targets.iter().any(|target| target == &member.name))
                    .cloned()
                    .collect(),
                None => self.team.members.clone(),
            };
            Ok(ResolvedTeamSelection {
                team: self.team.clone(),
                members,
            })
        }

        async fn persist_team_member_runtime_updates(
            &self,
            team_name: &str,
            updated_members: &[TeamMemberRecord],
        ) -> Result<()> {
            self.persisted_updates
                .lock()
                .expect("lock updates")
                .push((team_name.to_string(), updated_members.to_vec()));
            Ok(())
        }

        async fn run_agent_prompt(&self, request: AgentRunRequest) -> Result<Value> {
            let key = request
                .session_id
                .clone()
                .or(request.agent_name.clone())
                .ok_or_else(|| anyhow!("missing request key"))?;
            let response = self
                .responses
                .lock()
                .expect("lock responses")
                .remove(&key)
                .ok_or_else(|| anyhow!("missing response for {key}"))?;
            response.map_err(|error| anyhow!(error))
        }
    }

    fn sample_team() -> TeamRecord {
        TeamRecord {
            name: "builders".to_string(),
            layout: TeamLayoutRecord {
                strategy: "horizontal".to_string(),
                pane_group: Some("hellox-builders".to_string()),
            },
            members: vec![
                TeamMemberRecord {
                    name: "alice".to_string(),
                    session_id: "session-alice".to_string(),
                    backend: Some("tmux_pane".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: Some("%1".to_string()),
                },
                TeamMemberRecord {
                    name: "bob".to_string(),
                    session_id: "session-bob".to_string(),
                    backend: Some("tmux_pane".to_string()),
                    layout_slot: Some("right".to_string()),
                    pane_target: Some("%2".to_string()),
                },
            ],
        }
    }

    #[tokio::test]
    async fn send_message_tool_updates_team_member_runtime() {
        let context = TestContext {
            team: sample_team(),
            responses: Arc::new(Mutex::new(BTreeMap::from([(
                "session-alice".to_string(),
                Ok(json!({
                    "session_id": "session-alice",
                    "status": "running",
                    "backend": "tmux_pane",
                    "layout_slot": "primary",
                    "pane_target": "%3"
                })),
            )]))),
            persisted_updates: Arc::new(Mutex::new(Vec::new())),
        };

        let result = SendMessageTool
            .call(
                json!({
                    "team_name": "builders",
                    "to": "alice",
                    "content": "continue",
                    "run_in_background": true
                }),
                &context,
            )
            .await
            .expect("send message");

        let text = match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse result");
        assert_eq!(value["status"].as_str(), Some("running"));

        let updates = context.persisted_updates.lock().expect("lock updates");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, "builders");
        assert_eq!(updates[0].1[0].pane_target.as_deref(), Some("%3"));
    }

    #[tokio::test]
    async fn team_run_tool_can_coordinate_member_results() {
        let context = TestContext {
            team: sample_team(),
            responses: Arc::new(Mutex::new(BTreeMap::from([
                (
                    "session-alice".to_string(),
                    Ok(json!({
                        "session_id": "session-alice",
                        "status": "completed",
                        "result": "alice done"
                    })),
                ),
                (
                    "session-bob".to_string(),
                    Ok(json!({
                        "session_id": "session-bob",
                        "status": "completed",
                        "result": "bob done"
                    })),
                ),
                (
                    "builders-coordinator".to_string(),
                    Ok(json!({
                        "session_id": "coord-1",
                        "status": "completed",
                        "result": "summary"
                    })),
                ),
            ]))),
            persisted_updates: Arc::new(Mutex::new(Vec::new())),
        };

        let result = TeamRunTool
            .call(
                json!({
                    "team_name": "builders",
                    "prompt": "inspect",
                    "coordinator_prompt": "summarize"
                }),
                &context,
            )
            .await
            .expect("team run");

        let text = match result.content {
            ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse result");
        assert_eq!(value["status"].as_str(), Some("coordinated"));
        assert_eq!(value["members"].as_array().map(Vec::len), Some(2));
        assert!(value.get("coordinator").is_some());

        let updates = context.persisted_updates.lock().expect("lock updates");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].1.len(), 2);
    }
}
