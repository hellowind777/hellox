use anyhow::Result;

use crate::native_pane_backend::{ITERM_BACKEND, TMUX_BACKEND};
use crate::native_pane_runtime::{inspect_iterm_group, inspect_tmux_group, PaneGroupHostState};
use crate::team_storage::{TeamMemberRecord, TeamRecord};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeamMemberRuntimeSnapshot {
    pub backend: Option<String>,
    pub layout_slot: Option<String>,
    pub pane_target: Option<String>,
}

pub trait TeamRuntimeSnapshotProvider {
    fn background_runtime_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<TeamMemberRuntimeSnapshot>>;

    fn persisted_runtime_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<TeamMemberRuntimeSnapshot>>;
}

pub fn refresh_team_record_runtime(
    provider: &impl TeamRuntimeSnapshotProvider,
    team: &TeamRecord,
) -> Result<TeamRecord> {
    Ok(reconcile_team_host_runtime(TeamRecord {
        name: team.name.clone(),
        layout: team.layout.clone(),
        members: team
            .members
            .iter()
            .map(|member| refresh_member_runtime(provider, member))
            .collect::<Result<Vec<_>>>()?,
    }))
}

pub fn changed_runtime_members(
    original_members: &[TeamMemberRecord],
    refreshed_members: &[TeamMemberRecord],
) -> Vec<TeamMemberRecord> {
    refreshed_members
        .iter()
        .filter(|refreshed| {
            original_members
                .iter()
                .find(|original| original.session_id == refreshed.session_id)
                .is_some_and(|original| {
                    original.backend != refreshed.backend
                        || original.layout_slot != refreshed.layout_slot
                        || original.pane_target != refreshed.pane_target
                })
        })
        .cloned()
        .collect()
}

fn refresh_member_runtime(
    provider: &impl TeamRuntimeSnapshotProvider,
    member: &TeamMemberRecord,
) -> Result<TeamMemberRecord> {
    let mut refreshed = member.clone();
    if let Some(snapshot) = provider.background_runtime_snapshot(&member.session_id)? {
        apply_runtime_snapshot(&mut refreshed, snapshot);
        return Ok(refreshed);
    }

    if let Some(snapshot) = provider.persisted_runtime_snapshot(&member.session_id)? {
        apply_runtime_snapshot(&mut refreshed, snapshot);
    }

    Ok(refreshed)
}

fn apply_runtime_snapshot(member: &mut TeamMemberRecord, snapshot: TeamMemberRuntimeSnapshot) {
    if let Some(backend) = snapshot.backend {
        member.backend = Some(backend);
    }
    if let Some(layout_slot) = snapshot.layout_slot {
        member.layout_slot = Some(layout_slot);
    }
    if let Some(pane_target) = snapshot.pane_target {
        member.pane_target = Some(pane_target);
    }
}

fn reconcile_team_host_runtime(mut team: TeamRecord) -> TeamRecord {
    reconcile_backend_host_targets(
        &mut team.members,
        team.layout.pane_group.as_deref(),
        &[TMUX_BACKEND, "tmux"],
        inspect_tmux_group,
    );
    reconcile_backend_host_targets(
        &mut team.members,
        team.layout.pane_group.as_deref(),
        &[ITERM_BACKEND, "iterm"],
        inspect_iterm_group,
    );
    team
}

fn reconcile_backend_host_targets(
    members: &mut [TeamMemberRecord],
    pane_group: Option<&str>,
    backends: &[&str],
    inspect: fn(Option<&str>) -> Option<PaneGroupHostState>,
) {
    let Some(host_state) = inspect(pane_group) else {
        return;
    };
    if host_state.inspect_error.is_some() {
        return;
    }

    for member in members.iter_mut().filter(|member| {
        member
            .backend
            .as_deref()
            .is_some_and(|backend| backends.iter().any(|candidate| candidate == &backend))
    }) {
        if member
            .pane_target
            .as_ref()
            .is_some_and(|target| !host_state.live_targets.iter().any(|live| live == target))
        {
            member.pane_target = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::team_storage::TeamLayoutRecord;

    struct TestProvider {
        background: BTreeMap<String, TeamMemberRuntimeSnapshot>,
        persisted: BTreeMap<String, TeamMemberRuntimeSnapshot>,
    }

    impl TeamRuntimeSnapshotProvider for TestProvider {
        fn background_runtime_snapshot(
            &self,
            session_id: &str,
        ) -> Result<Option<TeamMemberRuntimeSnapshot>> {
            Ok(self.background.get(session_id).cloned())
        }

        fn persisted_runtime_snapshot(
            &self,
            session_id: &str,
        ) -> Result<Option<TeamMemberRuntimeSnapshot>> {
            Ok(self.persisted.get(session_id).cloned())
        }
    }

    #[test]
    fn refresh_team_record_runtime_prefers_background_then_persisted_metadata() {
        let team = TeamRecord {
            name: "builders".to_string(),
            layout: TeamLayoutRecord {
                strategy: "fanout".to_string(),
                pane_group: None,
            },
            members: vec![
                TeamMemberRecord {
                    name: "alice".to_string(),
                    session_id: "session-a".to_string(),
                    backend: Some("in_process".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: None,
                },
                TeamMemberRecord {
                    name: "bob".to_string(),
                    session_id: "session-b".to_string(),
                    backend: Some("in_process".to_string()),
                    layout_slot: Some("right".to_string()),
                    pane_target: None,
                },
            ],
        };
        let provider = TestProvider {
            background: BTreeMap::from([(
                "session-a".to_string(),
                TeamMemberRuntimeSnapshot {
                    backend: Some("tmux_pane".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: Some("%1".to_string()),
                },
            )]),
            persisted: BTreeMap::from([(
                "session-b".to_string(),
                TeamMemberRuntimeSnapshot {
                    backend: Some("detached_process".to_string()),
                    layout_slot: Some("right".to_string()),
                    pane_target: None,
                },
            )]),
        };

        let refreshed = refresh_team_record_runtime(&provider, &team).expect("refresh team");
        assert_eq!(refreshed.members[0].backend.as_deref(), Some("tmux_pane"));
        assert_eq!(refreshed.members[0].pane_target.as_deref(), Some("%1"));
        assert_eq!(
            refreshed.members[1].backend.as_deref(),
            Some("detached_process")
        );
    }

    #[test]
    fn changed_runtime_members_reports_only_modified_entries() {
        let original = vec![
            TeamMemberRecord {
                name: "alice".to_string(),
                session_id: "session-a".to_string(),
                backend: Some("in_process".to_string()),
                layout_slot: Some("primary".to_string()),
                pane_target: None,
            },
            TeamMemberRecord {
                name: "bob".to_string(),
                session_id: "session-b".to_string(),
                backend: Some("in_process".to_string()),
                layout_slot: Some("right".to_string()),
                pane_target: None,
            },
        ];
        let refreshed = vec![
            TeamMemberRecord {
                pane_target: Some("%1".to_string()),
                ..original[0].clone()
            },
            original[1].clone(),
        ];

        let changed = changed_runtime_members(&original, &refreshed);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].name, "alice");
    }
}
