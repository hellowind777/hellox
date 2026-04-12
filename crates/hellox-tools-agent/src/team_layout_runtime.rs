use anyhow::Result;
use serde_json::{json, Value};

use crate::native_pane_backend::{sync_tmux_layout_for_group, ITERM_BACKEND, TMUX_BACKEND};
use crate::native_pane_layout::tmux_layout_preset;
use crate::native_pane_runtime::{inspect_iterm_group, inspect_tmux_group, PaneGroupHostState};
use crate::team_storage::{TeamMemberRecord, TeamRecord};

pub fn summarize_layout_runtime(team: &TeamRecord) -> Value {
    let tmux_members = team
        .members
        .iter()
        .filter(|member| is_tmux_member(member))
        .collect::<Vec<_>>();
    if !tmux_members.is_empty() {
        return summarize_tmux_layout_runtime(team, &tmux_members);
    }

    let iterm_members = team
        .members
        .iter()
        .filter(|member| is_iterm_member(member))
        .collect::<Vec<_>>();
    if !iterm_members.is_empty() {
        return summarize_iterm_layout_runtime(team, &iterm_members);
    }

    json!({
        "status": "skipped",
        "reason": "no_tmux_members",
    })
}

pub fn sync_team_layout_runtime(pane_group: Option<&str>, team: &TeamRecord) -> Result<Value> {
    let Some(pane_group) = pane_group.filter(|value| !value.trim().is_empty()) else {
        return Ok(json!({
            "status": "skipped",
            "reason": "missing_pane_group",
        }));
    };
    let tmux_members = team
        .members
        .iter()
        .filter(|member| is_tmux_member(member))
        .collect::<Vec<_>>();
    if tmux_members.is_empty() {
        return Ok(json!({
            "status": "skipped",
            "reason": "no_tmux_members",
            "pane_group": pane_group,
        }));
    }

    let host_state = inspect_tmux_group(Some(pane_group));
    let tracked_targets = tracked_targets(&tmux_members);
    let live_group_targets = live_group_targets(host_state.as_ref(), &tracked_targets);
    if live_group_targets.is_empty() {
        return Ok(json!({
            "status": "skipped",
            "reason": "no_live_panes",
            "backend": TMUX_BACKEND,
            "pane_group": pane_group,
            "layout_strategy": team.layout.strategy.as_str(),
            "tracked_tmux_pane_targets": tracked_targets,
            "live_tmux_group_pane_targets": live_group_targets,
            "host_runtime_status": host_runtime_status(host_state.as_ref(), 0, tmux_members.len(), 0),
            "host_inspection_error": host_state.and_then(|state| state.inspect_error),
        }));
    }

    match sync_tmux_layout_for_group(pane_group, Some(&team.layout.strategy)) {
        Ok(Some(preset)) => Ok(json!({
            "status": "applied",
            "backend": TMUX_BACKEND,
            "pane_group": pane_group,
            "layout_strategy": team.layout.strategy.as_str(),
            "preset": preset,
            "tracked_tmux_pane_targets": tracked_targets,
            "live_tmux_group_pane_targets": live_group_targets,
            "host_runtime_status": host_runtime_status(host_state.as_ref(), 0, 0, live_group_targets.len()),
            "host_inspection_error": host_state.and_then(|state| state.inspect_error),
        })),
        Ok(None) => Ok(json!({
            "status": "skipped",
            "reason": "unsupported_layout_strategy",
            "backend": TMUX_BACKEND,
            "pane_group": pane_group,
            "layout_strategy": team.layout.strategy.as_str(),
            "tracked_tmux_pane_targets": tracked_targets,
            "live_tmux_group_pane_targets": live_group_targets,
            "host_runtime_status": host_runtime_status(host_state.as_ref(), 0, 0, live_group_targets.len()),
            "host_inspection_error": host_state.and_then(|state| state.inspect_error),
        })),
        Err(error) => Ok(json!({
            "status": "error",
            "backend": TMUX_BACKEND,
            "pane_group": pane_group,
            "layout_strategy": team.layout.strategy.as_str(),
            "tracked_tmux_pane_targets": tracked_targets,
            "live_tmux_group_pane_targets": live_group_targets,
            "host_runtime_status": host_runtime_status(host_state.as_ref(), 0, 0, live_group_targets.len()),
            "host_inspection_error": host_state.and_then(|state| state.inspect_error),
            "error": error.to_string(),
        })),
    }
}

fn summarize_tmux_layout_runtime(team: &TeamRecord, tmux_members: &[&TeamMemberRecord]) -> Value {
    let Some(preset) = tmux_layout_preset(Some(&team.layout.strategy)) else {
        return json!({
            "status": "skipped",
            "reason": "unsupported_layout_strategy",
            "layout_strategy": team.layout.strategy,
        });
    };

    let tmux_member_names = tmux_members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    let tracked_targets = tracked_targets(tmux_members);
    let host_state = inspect_tmux_group(team.layout.pane_group.as_deref());
    let live_group_targets = live_group_targets(host_state.as_ref(), &tracked_targets);
    let orphan_targets = orphan_targets(&live_group_targets, &tracked_targets);
    let live_tmux_member_names = live_member_names(tmux_members, &live_group_targets);
    let pending_tmux_member_names = pending_member_names(tmux_members, &live_group_targets);

    json!({
        "status": if live_group_targets.is_empty() { "pending_live_panes" } else { "sync_capable" },
        "backend": TMUX_BACKEND,
        "pane_group": team.layout.pane_group,
        "layout_strategy": team.layout.strategy,
        "preset": preset,
        "tmux_members": tmux_member_names.len(),
        "live_tmux_members": live_tmux_member_names.len(),
        "pending_tmux_members": pending_tmux_member_names.len(),
        "tmux_member_names": tmux_member_names,
        "live_tmux_member_names": live_tmux_member_names,
        "pending_tmux_member_names": pending_tmux_member_names,
        "tracked_tmux_pane_targets": tracked_targets,
        "live_tmux_group_panes": live_group_targets.len(),
        "live_tmux_group_pane_targets": live_group_targets,
        "orphan_tmux_pane_targets": orphan_targets.clone(),
        "host_runtime_status": host_runtime_status(
            host_state.as_ref(),
            orphan_targets.len(),
            pending_tmux_member_names.len(),
            live_tmux_member_names.len(),
        ),
        "host_inspection_error": host_state.and_then(|state| state.inspect_error),
    })
}

fn summarize_iterm_layout_runtime(team: &TeamRecord, iterm_members: &[&TeamMemberRecord]) -> Value {
    let iterm_member_names = iterm_members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    let tracked_targets = tracked_targets(iterm_members);
    let host_state = inspect_iterm_group(team.layout.pane_group.as_deref());
    let live_group_targets = live_group_targets(host_state.as_ref(), &tracked_targets);
    let orphan_targets = orphan_targets(&live_group_targets, &tracked_targets);
    let live_iterm_member_names = live_member_names(iterm_members, &live_group_targets);
    let pending_iterm_member_names = pending_member_names(iterm_members, &live_group_targets);

    json!({
        "status": if live_group_targets.is_empty() { "pending_live_panes" } else { "live_panes" },
        "backend": ITERM_BACKEND,
        "pane_group": team.layout.pane_group,
        "layout_strategy": team.layout.strategy,
        "layout_sync": "manual_split_only",
        "iterm_members": iterm_member_names.len(),
        "live_iterm_members": live_iterm_member_names.len(),
        "pending_iterm_members": pending_iterm_member_names.len(),
        "iterm_member_names": iterm_member_names,
        "live_iterm_member_names": live_iterm_member_names,
        "pending_iterm_member_names": pending_iterm_member_names,
        "tracked_iterm_pane_targets": tracked_targets,
        "live_iterm_group_panes": live_group_targets.len(),
        "live_iterm_group_pane_targets": live_group_targets,
        "orphan_iterm_pane_targets": orphan_targets.clone(),
        "host_runtime_status": host_runtime_status(
            host_state.as_ref(),
            orphan_targets.len(),
            pending_iterm_member_names.len(),
            live_iterm_member_names.len(),
        ),
        "host_inspection_error": host_state.and_then(|state| state.inspect_error),
    })
}

fn tracked_targets(members: &[&TeamMemberRecord]) -> Vec<String> {
    members
        .iter()
        .filter_map(|member| member.pane_target.clone())
        .collect()
}

fn live_group_targets(
    host_state: Option<&PaneGroupHostState>,
    tracked_targets: &[String],
) -> Vec<String> {
    match host_state {
        Some(state) if state.inspect_error.is_none() => state.live_targets.clone(),
        _ => tracked_targets.to_vec(),
    }
}

fn live_member_names(members: &[&TeamMemberRecord], live_group_targets: &[String]) -> Vec<String> {
    members
        .iter()
        .filter(|member| {
            member
                .pane_target
                .as_ref()
                .is_some_and(|target| live_group_targets.iter().any(|live| live == target))
        })
        .map(|member| member.name.clone())
        .collect()
}

fn pending_member_names(
    members: &[&TeamMemberRecord],
    live_group_targets: &[String],
) -> Vec<String> {
    members
        .iter()
        .filter(|member| {
            !member
                .pane_target
                .as_ref()
                .is_some_and(|target| live_group_targets.iter().any(|live| live == target))
        })
        .map(|member| member.name.clone())
        .collect()
}

fn orphan_targets(live_group_targets: &[String], tracked_targets: &[String]) -> Vec<String> {
    live_group_targets
        .iter()
        .filter(|target| !tracked_targets.iter().any(|tracked| tracked == *target))
        .cloned()
        .collect()
}

fn host_runtime_status(
    host_state: Option<&PaneGroupHostState>,
    orphan_count: usize,
    pending_member_count: usize,
    live_member_count: usize,
) -> &'static str {
    match host_state {
        Some(state) if state.inspect_error.is_some() => "inspection_unavailable",
        Some(_) if live_member_count == 0 => "empty",
        Some(_) if orphan_count > 0 || pending_member_count > 0 => "drifted",
        Some(_) => "aligned",
        None => "missing_pane_group",
    }
}

fn is_tmux_member(member: &TeamMemberRecord) -> bool {
    matches!(member.backend.as_deref(), Some("tmux") | Some(TMUX_BACKEND))
}

fn is_iterm_member(member: &TeamMemberRecord) -> bool {
    matches!(
        member.backend.as_deref(),
        Some("iterm") | Some(ITERM_BACKEND)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::team_storage::TeamLayoutRecord;

    #[test]
    fn summarize_layout_runtime_reports_tmux_sync_capability() {
        let team = TeamRecord {
            name: "tmux-team".to_string(),
            layout: TeamLayoutRecord {
                strategy: "horizontal".to_string(),
                pane_group: None,
            },
            members: vec![
                TeamMemberRecord {
                    name: "alice".to_string(),
                    session_id: "a".to_string(),
                    backend: Some("tmux_pane".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: Some("%1".to_string()),
                },
                TeamMemberRecord {
                    name: "bob".to_string(),
                    session_id: "b".to_string(),
                    backend: Some("tmux_pane".to_string()),
                    layout_slot: Some("right".to_string()),
                    pane_target: Some("%2".to_string()),
                },
            ],
        };

        let runtime = summarize_layout_runtime(&team);
        assert_eq!(runtime["status"].as_str(), Some("sync_capable"));
        assert_eq!(runtime["preset"].as_str(), Some("even-horizontal"));
        assert_eq!(runtime["tmux_members"].as_u64(), Some(2));
        assert_eq!(runtime["live_tmux_members"].as_u64(), Some(2));
        assert_eq!(runtime["pending_tmux_members"].as_u64(), Some(0));
    }

    #[test]
    fn summarize_layout_runtime_skips_non_tmux_teams() {
        let team = TeamRecord {
            name: "local-team".to_string(),
            layout: TeamLayoutRecord {
                strategy: "fanout".to_string(),
                pane_group: Some("hellox-local-team".to_string()),
            },
            members: vec![TeamMemberRecord {
                name: "alice".to_string(),
                session_id: "a".to_string(),
                backend: Some("in_process".to_string()),
                layout_slot: Some("primary".to_string()),
                pane_target: None,
            }],
        };

        let runtime = summarize_layout_runtime(&team);
        assert_eq!(runtime["status"].as_str(), Some("skipped"));
        assert_eq!(runtime["reason"].as_str(), Some("no_tmux_members"));
    }

    #[test]
    fn summarize_layout_runtime_reports_pending_live_panes() {
        let team = TeamRecord {
            name: "tmux-pending-team".to_string(),
            layout: TeamLayoutRecord {
                strategy: "vertical".to_string(),
                pane_group: Some("hellox-tmux-pending-team".to_string()),
            },
            members: vec![
                TeamMemberRecord {
                    name: "alice".to_string(),
                    session_id: "a".to_string(),
                    backend: Some("tmux_pane".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: None,
                },
                TeamMemberRecord {
                    name: "bob".to_string(),
                    session_id: "b".to_string(),
                    backend: Some("tmux_pane".to_string()),
                    layout_slot: Some("bottom".to_string()),
                    pane_target: None,
                },
            ],
        };

        let runtime = summarize_layout_runtime(&team);
        assert_eq!(runtime["status"].as_str(), Some("pending_live_panes"));
        assert_eq!(runtime["preset"].as_str(), Some("even-vertical"));
        assert_eq!(runtime["tmux_members"].as_u64(), Some(2));
        assert_eq!(runtime["live_tmux_members"].as_u64(), Some(0));
        assert_eq!(runtime["pending_tmux_members"].as_u64(), Some(2));
    }

    #[test]
    fn summarize_layout_runtime_reports_iterm_live_panes() {
        let team = TeamRecord {
            name: "iterm-team".to_string(),
            layout: TeamLayoutRecord {
                strategy: "horizontal".to_string(),
                pane_group: Some("hellox-iterm-team".to_string()),
            },
            members: vec![
                TeamMemberRecord {
                    name: "alice".to_string(),
                    session_id: "a".to_string(),
                    backend: Some("iterm_pane".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: Some("session-1".to_string()),
                },
                TeamMemberRecord {
                    name: "bob".to_string(),
                    session_id: "b".to_string(),
                    backend: Some("iterm_pane".to_string()),
                    layout_slot: Some("right".to_string()),
                    pane_target: Some("session-2".to_string()),
                },
            ],
        };

        let runtime = summarize_layout_runtime(&team);
        assert_eq!(runtime["status"].as_str(), Some("live_panes"));
        assert_eq!(runtime["backend"].as_str(), Some("iterm_pane"));
        assert_eq!(runtime["layout_sync"].as_str(), Some("manual_split_only"));
        assert_eq!(runtime["iterm_members"].as_u64(), Some(2));
        assert_eq!(runtime["live_iterm_members"].as_u64(), Some(2));
        assert_eq!(runtime["pending_iterm_members"].as_u64(), Some(0));
    }
}
