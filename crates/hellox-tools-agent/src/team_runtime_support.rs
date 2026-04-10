use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::team_layout::pane_group_for_team;
use crate::team_member_contract::resolve_pane_anchor_target;
use crate::team_storage::{TeamMemberRecord, TeamRecord};

pub fn parse_targets(value: Option<&Value>) -> Result<Option<Vec<String>>> {
    match value {
        None => Ok(None),
        Some(Value::Array(items)) => {
            let mut targets = Vec::with_capacity(items.len());
            for item in items {
                let target = item
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow!("targets must be non-empty strings"))?;
                targets.push(target.to_string());
            }
            Ok(Some(targets))
        }
        Some(_) => Err(anyhow!("targets must be an array of strings")),
    }
}

pub fn resolve_team_pane_group(team: &TeamRecord) -> String {
    team.layout
        .pane_group
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| pane_group_for_team(&team.name))
}

pub fn follow_up_backend(
    requested_backend: Option<String>,
    member_backend: Option<String>,
    run_in_background: bool,
) -> Option<String> {
    requested_backend.or_else(|| {
        if run_in_background {
            member_backend
        } else {
            None
        }
    })
}

pub fn resolve_member_pane_anchor_target(
    team: &TeamRecord,
    member: &TeamMemberRecord,
) -> Option<String> {
    resolve_pane_anchor_target(&team.members, member.layout_slot.as_deref())
}

pub fn updated_member_from_agent_value(
    member: &TeamMemberRecord,
    value: &Value,
) -> TeamMemberRecord {
    TeamMemberRecord {
        name: member.name.clone(),
        session_id: member.session_id.clone(),
        backend: value
            .get("backend")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| member.backend.clone()),
        layout_slot: value
            .get("layout_slot")
            .map(|layout_slot| {
                layout_slot
                    .as_str()
                    .map(ToString::to_string)
                    .filter(|layout_slot| !layout_slot.trim().is_empty())
            })
            .unwrap_or_else(|| member.layout_slot.clone()),
        pane_target: value
            .get("pane_target")
            .map(|pane_target| {
                pane_target
                    .as_str()
                    .map(ToString::to_string)
                    .filter(|pane_target| !pane_target.trim().is_empty())
            })
            .unwrap_or_else(|| member.pane_target.clone()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parse_targets_accepts_non_empty_strings() {
        let targets = parse_targets(Some(&json!(["alice", "bob"]))).expect("parse targets");
        assert_eq!(targets, Some(vec!["alice".to_string(), "bob".to_string()]));
    }

    #[test]
    fn updated_member_from_agent_value_prefers_non_empty_runtime_metadata() {
        let member = TeamMemberRecord {
            name: "alice".to_string(),
            session_id: "session-a".to_string(),
            backend: Some("in_process".to_string()),
            layout_slot: Some("primary".to_string()),
            pane_target: None,
        };
        let updated = updated_member_from_agent_value(
            &member,
            &json!({
                "backend": "tmux_pane",
                "layout_slot": "right",
                "pane_target": "%2"
            }),
        );
        assert_eq!(updated.backend.as_deref(), Some("tmux_pane"));
        assert_eq!(updated.layout_slot.as_deref(), Some("right"));
        assert_eq!(updated.pane_target.as_deref(), Some("%2"));
    }
}
