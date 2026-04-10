use std::collections::BTreeSet;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::team_layout::{anchor_slot_for_layout_slot, assign_layout_slots};
use crate::team_storage::TeamMemberRecord;

#[derive(Debug, Clone, Deserialize)]
pub struct TeamMemberInput {
    pub name: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub max_turns: Option<u64>,
    #[serde(default)]
    pub run_in_background: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct PlannedTeamMember {
    pub input: TeamMemberInput,
    pub layout_slot: Option<String>,
}

pub fn member_input_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "prompt": { "type": "string" },
                "model": { "type": "string" },
                "backend": { "type": "string" },
                "permission_mode": { "type": "string" },
                "cwd": { "type": "string" },
                "session_id": { "type": "string" },
                "max_turns": { "type": "integer", "minimum": 1, "maximum": 64 },
                "run_in_background": { "type": "boolean" }
            },
            "required": ["name"]
        }
    })
}

pub fn parse_team_name(input: &Value) -> Result<String> {
    let name = input
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing required string field `name`"))?
        .trim()
        .to_string();
    if name.is_empty() {
        return Err(anyhow!("team name cannot be empty"));
    }
    Ok(name)
}

pub fn parse_member_inputs(
    input: &Value,
    field: &str,
    required: bool,
) -> Result<Vec<TeamMemberInput>> {
    let value = match input.get(field).cloned() {
        Some(value) => value,
        None if required => {
            return Err(anyhow!("missing required array field `{field}`"));
        }
        None => return Ok(Vec::new()),
    };
    let members = serde_json::from_value::<Vec<TeamMemberInput>>(value)
        .context(format!("failed to parse `{field}`"))?;
    if required && members.is_empty() {
        return Err(anyhow!("team must include at least one member"));
    }
    Ok(members)
}

pub fn ensure_member_names_unique(
    existing_members: &[TeamMemberRecord],
    additions: &[TeamMemberInput],
) -> Result<()> {
    let mut seen = existing_members
        .iter()
        .map(|member| member.name.clone())
        .collect::<BTreeSet<_>>();
    for member in additions {
        let name = member.name.trim();
        if name.is_empty() {
            return Err(anyhow!("team member name cannot be empty"));
        }
        if !seen.insert(name.to_string()) {
            return Err(anyhow!("team member `{name}` already exists"));
        }
    }
    Ok(())
}

pub fn partition_members(
    members: &[TeamMemberRecord],
    removals: &[String],
) -> Result<(Vec<TeamMemberRecord>, Vec<TeamMemberRecord>)> {
    if removals.is_empty() {
        return Ok((members.to_vec(), Vec::new()));
    }

    let requested = removals.iter().cloned().collect::<BTreeSet<_>>();
    let mut remaining = Vec::new();
    let mut removed = Vec::new();
    for member in members {
        if requested.contains(&member.name) {
            removed.push(member.clone());
        } else {
            remaining.push(member.clone());
        }
    }

    if removed.len() != requested.len() {
        let missing = requested
            .into_iter()
            .filter(|name| removed.iter().all(|member| member.name != *name))
            .collect::<Vec<_>>();
        return Err(anyhow!(
            "team member(s) were not found: {}",
            missing.join(", ")
        ));
    }

    Ok((remaining, removed))
}

pub fn plan_members(
    members: Vec<TeamMemberInput>,
    layout_strategy: &str,
    offset: usize,
) -> Result<Vec<PlannedTeamMember>> {
    let slots = assign_layout_slots(layout_strategy, members.len() + offset)?;
    Ok(members
        .into_iter()
        .zip(slots.into_iter().skip(offset))
        .map(|(input, slot)| PlannedTeamMember {
            input,
            layout_slot: Some(slot),
        })
        .collect())
}

pub fn apply_layout_slots(members: &mut [TeamMemberRecord], slots: &[String]) {
    for (member, slot) in members.iter_mut().zip(slots.iter()) {
        member.layout_slot = Some(slot.clone());
    }
}

pub fn resolve_pane_anchor_target(
    members: &[TeamMemberRecord],
    layout_slot: Option<&str>,
) -> Option<String> {
    let mut candidate = anchor_slot_for_layout_slot(layout_slot);
    while let Some(slot) = candidate {
        if let Some(member) = members.iter().find(|member| {
            member.layout_slot.as_deref() == Some(slot.as_str()) && member.pane_target.is_some()
        }) {
            return member.pane_target.clone();
        }
        candidate = anchor_slot_for_layout_slot(Some(&slot));
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::team_storage::TeamMemberRecord;

    #[test]
    fn parse_team_name_rejects_empty_names() {
        let error = parse_team_name(&json!({ "name": "   " })).expect_err("reject empty");
        assert!(error.to_string().contains("cannot be empty"));
    }

    #[test]
    fn plan_members_assigns_offset_layout_slots() {
        let planned = plan_members(
            vec![
                TeamMemberInput {
                    name: "alice".to_string(),
                    prompt: None,
                    model: None,
                    backend: None,
                    permission_mode: None,
                    cwd: None,
                    session_id: None,
                    max_turns: None,
                    run_in_background: None,
                },
                TeamMemberInput {
                    name: "bob".to_string(),
                    prompt: None,
                    model: None,
                    backend: None,
                    permission_mode: None,
                    cwd: None,
                    session_id: None,
                    max_turns: None,
                    run_in_background: None,
                },
            ],
            "horizontal",
            1,
        )
        .expect("plan members");
        assert_eq!(planned[0].layout_slot.as_deref(), Some("right"));
        assert_eq!(planned[1].layout_slot.as_deref(), Some("right-2"));
    }

    #[test]
    fn resolve_pane_anchor_target_walks_previous_slots() {
        let members = vec![
            TeamMemberRecord {
                name: "primary".to_string(),
                session_id: "1".to_string(),
                backend: Some("tmux_pane".to_string()),
                layout_slot: Some("primary".to_string()),
                pane_target: Some("%1".to_string()),
            },
            TeamMemberRecord {
                name: "right".to_string(),
                session_id: "2".to_string(),
                backend: Some("tmux_pane".to_string()),
                layout_slot: Some("right".to_string()),
                pane_target: Some("%2".to_string()),
            },
        ];

        assert_eq!(
            resolve_pane_anchor_target(&members, Some("right-2")).as_deref(),
            Some("%2")
        );
    }
}
