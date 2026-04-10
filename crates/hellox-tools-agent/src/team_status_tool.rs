use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_tool_runtime::{LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};

use crate::shared::{optional_string, render_json};
use crate::team_storage::{load_teams, team_file_path, TeamMemberRecord, TeamRecord};

#[async_trait]
pub trait TeamStatusToolContext {
    fn working_directory(&self) -> &Path;
    async fn persist_team_runtime_reconciliation(&self, requested_name: Option<&str>)
        -> Result<()>;
    fn refresh_team_record_runtime(&self, team: &TeamRecord) -> Result<TeamRecord>;
    fn summarize_layout_runtime(&self, team: &TeamRecord) -> Value;
    fn background_agent_status_value(&self, session_id: &str) -> Value;
}

pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TeamStatusToolContext + Send + Sync + 'static,
{
    registry.register(TeamStatusTool);
}

pub struct TeamStatusTool;

#[async_trait]
impl<C> LocalTool<C> for TeamStatusTool
where
    C: TeamStatusToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TeamStatus".to_string(),
            description: Some("Inspect local teams and teammate session status.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let requested = optional_string(&input, "name");
        context
            .persist_team_runtime_reconciliation(requested.as_deref())
            .await?;
        Ok(LocalToolResult::text(render_json(team_status_value(
            context,
            requested.as_deref(),
        )?)?))
    }
}

pub fn team_status_value(
    context: &impl TeamStatusToolContext,
    requested_name: Option<&str>,
) -> Result<Value> {
    let path = team_file_path(context.working_directory());
    let teams = load_teams(&path)?;
    let filtered = match requested_name {
        Some(name) => teams
            .into_iter()
            .filter(|team| team.name == name)
            .collect::<Vec<_>>(),
        None => teams,
    };
    if filtered.is_empty() {
        return Err(anyhow!("no matching team was found"));
    }

    let teams = filtered
        .into_iter()
        .map(|team| context.refresh_team_record_runtime(&team))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|team| render_team_status(context, team))
        .collect::<Vec<_>>();

    Ok(json!({
        "teams": teams,
        "summary": summarize_teams(&teams),
    }))
}

fn render_team_status(context: &impl TeamStatusToolContext, team: TeamRecord) -> Value {
    let layout_runtime = context.summarize_layout_runtime(&team);
    let layout = json!({
        "strategy": team.layout.strategy,
        "pane_group": team.layout.pane_group,
        "runtime": layout_runtime,
        "members": team
            .members
            .iter()
            .map(|member| json!({
                "name": member.name,
                "backend": member.backend,
                "layout_slot": member.layout_slot,
                "pane_target": member.pane_target,
            }))
            .collect::<Vec<_>>(),
    });
    let members = team
        .members
        .into_iter()
        .map(|member| render_member_status(context, member))
        .collect::<Vec<_>>();
    json!({
        "name": team.name,
        "layout": layout,
        "summary": summarize_members(&members),
        "members": members,
    })
}

fn render_member_status(context: &impl TeamStatusToolContext, member: TeamMemberRecord) -> Value {
    let mut status = context.background_agent_status_value(&member.session_id);
    if let Some(status_object) = status.as_object_mut() {
        status_object.insert("backend".to_string(), json!(member.backend));
        status_object.insert("layout_slot".to_string(), json!(member.layout_slot));
        status_object.insert("pane_target".to_string(), json!(member.pane_target));
    }
    json!({
        "name": member.name,
        "agent": status,
    })
}

fn summarize_teams(teams: &[Value]) -> Value {
    let mut total = 0_u64;
    let mut running = 0_u64;
    let mut completed = 0_u64;
    let mut failed = 0_u64;
    let mut cancelled = 0_u64;
    let mut persisted = 0_u64;
    let mut missing = 0_u64;

    for team in teams {
        total += 1;
        match team
            .get("summary")
            .and_then(|summary| summary.get("overall_status"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
        {
            "running" => running += 1,
            "completed" => completed += 1,
            "failed" => failed += 1,
            "cancelled" | "partial_cancelled" => cancelled += 1,
            "persisted" => persisted += 1,
            "missing" => missing += 1,
            _ => {}
        }
    }

    json!({
        "total_teams": total,
        "running_teams": running,
        "completed_teams": completed,
        "failed_teams": failed,
        "cancelled_teams": cancelled,
        "persisted_teams": persisted,
        "missing_teams": missing,
        "overall_status": overall_status(running, failed, missing, cancelled, completed, persisted),
    })
}

fn summarize_members(members: &[Value]) -> Value {
    let mut total = 0_u64;
    let mut running = 0_u64;
    let mut completed = 0_u64;
    let mut failed = 0_u64;
    let mut cancelled = 0_u64;
    let mut persisted = 0_u64;
    let mut missing = 0_u64;

    for member in members {
        total += 1;
        match member
            .get("agent")
            .and_then(|agent| agent.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
        {
            "running" => running += 1,
            "completed" => completed += 1,
            "failed" => failed += 1,
            "cancelled" => cancelled += 1,
            "persisted" => persisted += 1,
            "missing" => missing += 1,
            _ => {}
        }
    }

    json!({
        "total_members": total,
        "running_members": running,
        "completed_members": completed,
        "failed_members": failed,
        "cancelled_members": cancelled,
        "persisted_members": persisted,
        "missing_members": missing,
        "overall_status": overall_status(running, failed, missing, cancelled, completed, persisted),
    })
}

fn overall_status(
    running: u64,
    failed: u64,
    missing: u64,
    cancelled: u64,
    completed: u64,
    persisted: u64,
) -> &'static str {
    if running > 0 {
        "running"
    } else if failed > 0 {
        "failed"
    } else if missing > 0 {
        "missing"
    } else if cancelled > 0 && completed + persisted > 0 {
        "partial_cancelled"
    } else if cancelled > 0 {
        "cancelled"
    } else if completed > 0 {
        "completed"
    } else if persisted > 0 {
        "persisted"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::team_storage::{save_teams, TeamLayoutRecord};

    struct TestContext {
        working_directory: PathBuf,
    }

    #[async_trait]
    impl TeamStatusToolContext for TestContext {
        fn working_directory(&self) -> &Path {
            &self.working_directory
        }

        async fn persist_team_runtime_reconciliation(
            &self,
            _requested_name: Option<&str>,
        ) -> Result<()> {
            Ok(())
        }

        fn refresh_team_record_runtime(&self, team: &TeamRecord) -> Result<TeamRecord> {
            Ok(team.clone())
        }

        fn summarize_layout_runtime(&self, team: &TeamRecord) -> Value {
            json!({
                "status": "skipped",
                "members": team.members.len(),
            })
        }

        fn background_agent_status_value(&self, session_id: &str) -> Value {
            json!({
                "session_id": session_id,
                "status": "completed",
            })
        }
    }

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("hellox-team-status-{unique}"));
            std::fs::create_dir_all(&root).expect("create temp dir");
            Self { root }
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn team_status_tool_reads_saved_team_registry() {
        let workspace = TestWorkspace::new();
        let path = team_file_path(&workspace.root);
        save_teams(
            &path,
            &[TeamRecord {
                name: "reviewers".to_string(),
                layout: TeamLayoutRecord {
                    strategy: "fanout".to_string(),
                    pane_group: Some("hellox-reviewers".to_string()),
                },
                members: vec![TeamMemberRecord {
                    name: "alice".to_string(),
                    session_id: "session-a".to_string(),
                    backend: Some("in_process".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: None,
                }],
            }],
        )
        .expect("save teams");
        let context = TestContext {
            working_directory: workspace.root.clone(),
        };

        let result = TeamStatusTool
            .call(json!({ "name": "reviewers" }), &context)
            .await
            .expect("team status");

        let text = match result.content {
            hellox_gateway_api::ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse result");
        assert_eq!(value["summary"]["total_teams"].as_u64(), Some(1));
        assert_eq!(
            value["teams"][0]["members"][0]["name"].as_str(),
            Some("alice")
        );
        assert_eq!(
            value["teams"][0]["members"][0]["agent"]["status"].as_str(),
            Some("completed")
        );
    }
}
