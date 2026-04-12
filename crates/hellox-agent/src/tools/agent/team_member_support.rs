use anyhow::{anyhow, Result};
use hellox_config::PermissionMode;
use serde_json::{json, Value};

use super::super::ToolExecutionContext;
use super::background::agent_status_value;
use super::runtime::run_agent_prompt;
use super::shared::AgentRunRequest;
use super::supervision::stop_background_agent_value;
use super::team_member_contract::{resolve_pane_anchor_target, PlannedTeamMember};
use super::team_registry_support::{parse_member_permission_mode, sync_member_permissions};
use super::team_storage::TeamMemberRecord;

pub(super) async fn materialize_members(
    context: &ToolExecutionContext,
    members: Vec<PlannedTeamMember>,
    default_permission_mode: Option<PermissionMode>,
    pane_group: Option<&str>,
    layout_strategy: Option<&str>,
    existing_members: &[TeamMemberRecord],
) -> Result<(Vec<TeamMemberRecord>, Vec<Value>)> {
    let mut member_records = Vec::with_capacity(members.len());
    let mut results = Vec::with_capacity(members.len());
    let mut known_members = existing_members.to_vec();

    for planned in members {
        let member = planned.input;
        let member_name = member.name.trim().to_string();
        let permission_mode = parse_member_permission_mode(
            member.permission_mode.as_deref(),
            default_permission_mode.as_ref(),
        )?;
        if member
            .prompt
            .as_deref()
            .is_none_or(|prompt| prompt.trim().is_empty())
            && member.session_id.is_none()
        {
            return Err(anyhow!(
                "team member `{member_name}` requires either `prompt` or `session_id`"
            ));
        }

        let value = if let Some(prompt) = member.prompt {
            let pane_anchor_target =
                resolve_pane_anchor_target(&known_members, planned.layout_slot.as_deref());
            run_agent_prompt(
                context,
                AgentRunRequest {
                    prompt,
                    model: member.model,
                    backend: member.backend,
                    isolation: None,
                    worktree_name: None,
                    worktree_base_ref: None,
                    permission_mode: permission_mode.clone(),
                    agent_name: Some(member_name.clone()),
                    pane_group: pane_group.map(ToString::to_string),
                    layout_strategy: layout_strategy.map(ToString::to_string),
                    layout_slot: planned.layout_slot.clone(),
                    pane_anchor_target,
                    cwd: member.cwd,
                    session_id: member.session_id,
                    max_turns: member.max_turns.map(|value| value as usize).unwrap_or(8),
                    reuse_existing_worktree: false,
                    run_in_background: member.run_in_background.unwrap_or(true),
                    allow_interaction: false,
                },
            )
            .await?
        } else {
            let session_id = member.session_id.expect("validated above");
            let _ = sync_member_permissions(
                context,
                &[TeamMemberRecord {
                    name: member_name.clone(),
                    session_id: session_id.clone(),
                    backend: None,
                    layout_slot: planned.layout_slot.clone(),
                    pane_target: None,
                }],
                permission_mode.as_ref(),
            )?;
            agent_status_value(&session_id)?
        };

        let session_id = value
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("team member `{member_name}` is missing `session_id`"))?
            .to_string();
        member_records.push(TeamMemberRecord {
            name: member_name.clone(),
            session_id,
            backend: value
                .get("backend")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            layout_slot: value
                .get("layout_slot")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .or(planned.layout_slot),
            pane_target: value
                .get("pane_target")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        });
        known_members.push(
            member_records
                .last()
                .expect("member record inserted")
                .clone(),
        );
        results.push(json!({
            "name": member_name,
            "agent": value,
        }));
    }

    Ok((member_records, results))
}

pub(super) fn render_removed_members(
    members: &[TeamMemberRecord],
    stop_removed: bool,
    reason: Option<String>,
) -> Result<Vec<Value>> {
    members
        .iter()
        .map(|member| {
            let action = if stop_removed {
                stop_background_agent_value(&member.session_id, reason.clone())?
            } else {
                json!({
                    "session_id": member.session_id,
                    "stopped": false,
                    "agent": safe_agent_status_value(&member.session_id),
                })
            };
            Ok(json!({
                "name": member.name,
                "session_id": member.session_id,
                "backend": member.backend,
                "layout_slot": member.layout_slot,
                "pane_target": member.pane_target,
                "action": action,
            }))
        })
        .collect()
}

pub(super) fn safe_agent_status_value(session_id: &str) -> Value {
    agent_status_value(session_id).unwrap_or_else(|error| {
        json!({
            "session_id": session_id,
            "status": "missing",
            "error": error.to_string(),
        })
    })
}
