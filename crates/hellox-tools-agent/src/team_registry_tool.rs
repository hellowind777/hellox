use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_config::PermissionMode;
use hellox_tool_runtime::{LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};

use crate::shared::{parse_permission_mode, render_json};
use crate::team_layout::{assign_layout_slots, pane_group_for_team, parse_layout_strategy};
use crate::team_member_contract::{
    apply_layout_slots, ensure_member_names_unique, member_input_schema, parse_member_inputs,
    parse_team_name, partition_members, plan_members,
};
use crate::team_status_tool::{team_status_value, TeamStatusToolContext};
use crate::team_storage::{
    load_teams, save_teams, team_file_path, TeamLayoutRecord, TeamMemberRecord, TeamRecord,
};

#[async_trait]
pub trait TeamRegistryToolContext: TeamStatusToolContext {
    async fn ensure_write_allowed(&self, path: &Path) -> Result<()>;
    async fn materialize_members(
        &self,
        members: Vec<crate::team_member_contract::PlannedTeamMember>,
        default_permission_mode: Option<PermissionMode>,
        pane_group: Option<&str>,
        layout_strategy: Option<&str>,
        existing_members: &[TeamMemberRecord],
    ) -> Result<(Vec<TeamMemberRecord>, Vec<Value>)>;
    fn render_removed_members(
        &self,
        members: &[TeamMemberRecord],
        stop_removed: bool,
        reason: Option<String>,
    ) -> Result<Vec<Value>>;
    fn sync_member_runtime_metadata(&self, members: &[TeamMemberRecord]) -> Result<()>;
    fn sync_member_permissions(
        &self,
        members: &[TeamMemberRecord],
        permission_mode: Option<&PermissionMode>,
    ) -> Result<Vec<Value>>;
    fn sync_team_layout_runtime(
        &self,
        pane_group: Option<&str>,
        team: &TeamRecord,
    ) -> Result<Value>;
}

pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: TeamRegistryToolContext + Send + Sync + 'static,
{
    registry.register(TeamCreateTool);
    registry.register(TeamUpdateTool);
    registry.register(TeamDeleteTool);
}

pub struct TeamCreateTool;
pub struct TeamUpdateTool;
pub struct TeamDeleteTool;

#[async_trait]
impl<C> LocalTool<C> for TeamCreateTool
where
    C: TeamRegistryToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TeamCreate".to_string(),
            description: Some(
                "Create a local teammate group and optionally launch members.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "layout": { "type": "string" },
                    "members": member_input_schema()
                },
                "required": ["name", "members"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let team_name = parse_team_name(&input)?;
        let members = parse_member_inputs(&input, "members", true)?;
        ensure_member_names_unique(&[], &members)?;
        let layout_strategy =
            parse_layout_strategy(&input, "layout")?.unwrap_or_else(|| "fanout".to_string());
        let pane_group = pane_group_for_team(&team_name);
        let planned_members = plan_members(members, &layout_strategy, 0)?;

        let path = team_file_path(context.working_directory());
        let mut teams = load_teams(&path)?;
        if teams.iter().any(|team| team.name == team_name) {
            return Err(anyhow!("team `{team_name}` already exists"));
        }

        let (member_records, results) = context
            .materialize_members(
                planned_members,
                None,
                Some(&pane_group),
                Some(&layout_strategy),
                &[],
            )
            .await?;
        context.sync_member_runtime_metadata(&member_records)?;
        let team_record = TeamRecord {
            name: team_name.clone(),
            layout: TeamLayoutRecord {
                strategy: layout_strategy,
                pane_group: Some(pane_group),
            },
            members: member_records,
        };
        let layout_runtime_sync = context
            .sync_team_layout_runtime(team_record.layout.pane_group.as_deref(), &team_record)?;
        teams.push(team_record);

        context.ensure_write_allowed(&path).await?;
        save_teams(&path, &teams)?;
        context
            .persist_team_runtime_reconciliation(Some(&team_name))
            .await?;
        let status = team_status_value(context, Some(&team_name))?;

        Ok(LocalToolResult::text(render_json(json!({
            "team": team_name,
            "members": results,
            "layout_runtime_sync": layout_runtime_sync,
            "status": status,
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TeamUpdateTool
where
    C: TeamRegistryToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TeamUpdate".to_string(),
            description: Some(
                "Add or remove local teammates and optionally stop removed background members."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "layout": { "type": "string" },
                    "force_layout_sync": { "type": "boolean" },
                    "add_members": member_input_schema(),
                    "permission_mode": { "type": "string" },
                    "remove_members": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "stop_removed": { "type": "boolean" },
                    "reason": { "type": "string" }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let team_name = parse_team_name(&input)?;
        let additions = parse_member_inputs(&input, "add_members", false)?;
        let permission_mode = parse_permission_mode(&input, "permission_mode")?;
        let layout_strategy = parse_layout_strategy(&input, "layout")?;
        let force_layout_sync = input
            .get("force_layout_sync")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let removals = parse_member_names(input.get("remove_members"))?;
        if additions.is_empty()
            && removals.is_empty()
            && permission_mode.is_none()
            && layout_strategy.is_none()
            && !force_layout_sync
        {
            return Err(anyhow!(
                "team_update requires add_members, remove_members, permission_mode, layout, or force_layout_sync"
            ));
        }

        let path = team_file_path(context.working_directory());
        let mut teams = load_teams(&path)?;
        let index = teams
            .iter()
            .position(|team| team.name == team_name)
            .ok_or_else(|| anyhow!("team `{team_name}` was not found"))?;

        let current_members = teams[index].members.clone();
        let (mut remaining_members, removed_members) =
            partition_members(&current_members, &removals)?;
        if remaining_members.is_empty() && additions.is_empty() {
            return Err(anyhow!(
                "team_update would leave `{team_name}` without any members"
            ));
        }
        ensure_member_names_unique(&remaining_members, &additions)?;
        let updated_permissions =
            context.sync_member_permissions(&remaining_members, permission_mode.as_ref())?;

        let resolved_layout =
            layout_strategy.unwrap_or_else(|| teams[index].layout.strategy.clone());
        let pane_group = teams[index]
            .layout
            .pane_group
            .clone()
            .unwrap_or_else(|| pane_group_for_team(&team_name));
        let slots =
            assign_layout_slots(&resolved_layout, remaining_members.len() + additions.len())?;
        let remaining_count = remaining_members.len();
        apply_layout_slots(&mut remaining_members, &slots[..remaining_count]);

        let stop_removed = input
            .get("stop_removed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let reason = input
            .get("reason")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let removed_results =
            context.render_removed_members(&removed_members, stop_removed, reason)?;
        let planned_additions = plan_members(additions, &resolved_layout, remaining_members.len())?;
        let (added_records, added_results) = context
            .materialize_members(
                planned_additions,
                permission_mode.clone(),
                Some(&pane_group),
                Some(&resolved_layout),
                &remaining_members,
            )
            .await?;

        let mut updated_members = remaining_members;
        updated_members.extend(added_records);
        context.sync_member_runtime_metadata(&updated_members)?;
        teams[index].layout = TeamLayoutRecord {
            strategy: resolved_layout,
            pane_group: Some(pane_group),
        };
        teams[index].members = updated_members;
        let layout_runtime_sync = context
            .sync_team_layout_runtime(teams[index].layout.pane_group.as_deref(), &teams[index])?;

        context.ensure_write_allowed(&path).await?;
        save_teams(&path, &teams)?;
        context
            .persist_team_runtime_reconciliation(Some(&team_name))
            .await?;
        let status = team_status_value(context, Some(&team_name))?;

        Ok(LocalToolResult::text(render_json(json!({
            "team": team_name,
            "added_members": added_results,
            "removed_members": removed_results,
            "updated_permissions": updated_permissions,
            "force_layout_sync": force_layout_sync,
            "layout_runtime_sync": layout_runtime_sync,
            "status": status,
        }))?))
    }
}

#[async_trait]
impl<C> LocalTool<C> for TeamDeleteTool
where
    C: TeamRegistryToolContext + Send + Sync,
{
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "TeamDelete".to_string(),
            description: Some("Delete a local team registry entry.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let team_name = parse_team_name(&input)?;
        let path = team_file_path(context.working_directory());
        let mut teams = load_teams(&path)?;
        let index = teams
            .iter()
            .position(|team| team.name == team_name)
            .ok_or_else(|| anyhow!("team `{team_name}` was not found"))?;
        let removed = teams.remove(index);

        context.ensure_write_allowed(&path).await?;
        save_teams(&path, &teams)?;

        Ok(LocalToolResult::text(render_json(json!({
            "deleted_team": removed.name,
            "layout": removed.layout,
            "orphaned_sessions": removed
                .members
                .into_iter()
                .map(|member| json!({
                    "name": member.name,
                    "session_id": member.session_id,
                    "backend": member.backend,
                    "layout_slot": member.layout_slot,
                    "pane_target": member.pane_target,
                }))
                .collect::<Vec<_>>(),
        }))?))
    }
}

fn parse_member_names(value: Option<&Value>) -> Result<Vec<String>> {
    let Some(Value::Array(items)) = value else {
        return value.map_or_else(
            || Ok(Vec::new()),
            |_| Err(anyhow!("remove_members must be an array of strings")),
        );
    };

    let mut names = Vec::with_capacity(items.len());
    for item in items {
        let name = item
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("remove_members must be non-empty strings"))?;
        names.push(name.to_string());
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::team_member_contract::PlannedTeamMember;

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

        fn summarize_layout_runtime(&self, _team: &TeamRecord) -> Value {
            json!({ "status": "skipped" })
        }

        fn background_agent_status_value(&self, session_id: &str) -> Value {
            json!({
                "session_id": session_id,
                "status": "completed",
            })
        }
    }

    #[async_trait]
    impl TeamRegistryToolContext for TestContext {
        async fn ensure_write_allowed(&self, _path: &Path) -> Result<()> {
            Ok(())
        }

        async fn materialize_members(
            &self,
            members: Vec<PlannedTeamMember>,
            _default_permission_mode: Option<PermissionMode>,
            _pane_group: Option<&str>,
            _layout_strategy: Option<&str>,
            _existing_members: &[TeamMemberRecord],
        ) -> Result<(Vec<TeamMemberRecord>, Vec<Value>)> {
            let mut records = Vec::new();
            let mut results = Vec::new();
            for member in members {
                let name = member.input.name;
                let session_id = format!("session-{name}");
                let record = TeamMemberRecord {
                    name: name.clone(),
                    session_id: session_id.clone(),
                    backend: Some("in_process".to_string()),
                    layout_slot: member.layout_slot.clone(),
                    pane_target: None,
                };
                results.push(json!({
                    "name": name,
                    "agent": {
                        "session_id": session_id,
                        "status": "completed",
                        "backend": "in_process",
                        "layout_slot": member.layout_slot,
                    }
                }));
                records.push(record);
            }
            Ok((records, results))
        }

        fn render_removed_members(
            &self,
            members: &[TeamMemberRecord],
            stop_removed: bool,
            _reason: Option<String>,
        ) -> Result<Vec<Value>> {
            Ok(members
                .iter()
                .map(|member| {
                    json!({
                        "name": member.name,
                        "session_id": member.session_id,
                        "action": {
                            "stopped": stop_removed,
                        }
                    })
                })
                .collect())
        }

        fn sync_member_runtime_metadata(&self, _members: &[TeamMemberRecord]) -> Result<()> {
            Ok(())
        }

        fn sync_member_permissions(
            &self,
            _members: &[TeamMemberRecord],
            permission_mode: Option<&PermissionMode>,
        ) -> Result<Vec<Value>> {
            Ok(permission_mode
                .map(|mode| vec![json!({ "permission_mode": mode.to_string() })])
                .unwrap_or_default())
        }

        fn sync_team_layout_runtime(
            &self,
            _pane_group: Option<&str>,
            _team: &TeamRecord,
        ) -> Result<Value> {
            Ok(json!({ "status": "skipped" }))
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
            let root = std::env::temp_dir().join(format!("hellox-team-registry-{unique}"));
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
    async fn team_create_tool_persists_team_registry() {
        let workspace = TestWorkspace::new();
        let context = TestContext {
            working_directory: workspace.root.clone(),
        };

        let result = TeamCreateTool
            .call(
                json!({
                    "name": "builders",
                    "members": [
                        { "name": "alice", "prompt": "review code" }
                    ]
                }),
                &context,
            )
            .await
            .expect("team create");

        let text = match result.content {
            hellox_gateway_api::ToolResultContent::Text(text) => text,
            other => panic!("expected text result, got {other:?}"),
        };
        let value: Value = serde_json::from_str(&text).expect("parse result");
        assert_eq!(value["team"].as_str(), Some("builders"));
        assert_eq!(value["status"]["summary"]["total_teams"].as_u64(), Some(1));

        let teams = load_teams(&team_file_path(&workspace.root)).expect("load teams");
        assert_eq!(teams.len(), 1);
        assert_eq!(teams[0].name, "builders");
        assert_eq!(teams[0].members.len(), 1);
    }
}
