use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::team_runtime_reconciliation::{
    changed_runtime_members, refresh_team_record_runtime, TeamRuntimeSnapshotProvider,
};
use crate::team_runtime_support::updated_member_from_agent_value;
use crate::team_storage::{load_teams, save_teams, team_file_path, TeamMemberRecord, TeamRecord};

#[derive(Clone)]
pub struct ResolvedTeamSelection {
    pub team: TeamRecord,
    pub members: Vec<TeamMemberRecord>,
}

#[async_trait]
pub trait TeamRuntimePersistenceContext: TeamRuntimeSnapshotProvider + Send + Sync {
    fn working_directory(&self) -> &Path;
    async fn ensure_write_allowed(&self, path: &Path) -> Result<()>;
    fn reconcile_member_runtime_metadata(&self, updated_members: &[TeamMemberRecord])
        -> Result<()>;
}

pub async fn resolve_team_members(
    context: &impl TeamRuntimePersistenceContext,
    team_name: &str,
    targets: Option<Vec<String>>,
) -> Result<Vec<TeamMemberRecord>> {
    Ok(resolve_team_selection(context, team_name, targets)
        .await?
        .members)
}

pub async fn resolve_team_selection(
    context: &impl TeamRuntimePersistenceContext,
    team_name: &str,
    targets: Option<Vec<String>>,
) -> Result<ResolvedTeamSelection> {
    persist_team_runtime_reconciliation(context, Some(team_name)).await?;

    let path = team_file_path(context.working_directory());
    let mut teams = load_teams(&path)?;
    let index = teams
        .iter()
        .position(|team| team.name == team_name)
        .ok_or_else(|| anyhow!("team `{team_name}` was not found"))?;
    let original_team = teams[index].clone();
    let team = refresh_team_record_runtime(context, &original_team)?;
    let runtime_updates = changed_runtime_members(&original_team.members, &team.members);
    if !runtime_updates.is_empty() {
        teams[index] = team.clone();
        context.ensure_write_allowed(&path).await?;
        save_teams(&path, &teams)?;
        context.reconcile_member_runtime_metadata(&runtime_updates)?;
    }

    let members = match targets {
        Some(targets) => targets
            .into_iter()
            .map(|target| {
                team.members
                    .iter()
                    .find(|member| member.name == target)
                    .cloned()
                    .ok_or_else(|| anyhow!("team member `{target}` was not found in `{team_name}`"))
            })
            .collect::<Result<Vec<_>>>()?,
        None => team.members.clone(),
    };

    if members.is_empty() {
        return Err(anyhow!(
            "team `{team_name}` does not contain any runnable members"
        ));
    }

    Ok(ResolvedTeamSelection { team, members })
}

pub async fn persist_team_member_runtime_updates(
    context: &impl TeamRuntimePersistenceContext,
    team_name: &str,
    updated_members: &[TeamMemberRecord],
) -> Result<()> {
    if updated_members.is_empty() {
        return Ok(());
    }

    let path = team_file_path(context.working_directory());
    let mut teams = load_teams(&path)?;
    let Some(index) = teams.iter().position(|team| team.name == team_name) else {
        return Ok(());
    };

    let mut changed = false;
    for member in &mut teams[index].members {
        if let Some(updated) = updated_members
            .iter()
            .find(|candidate| candidate.session_id == member.session_id)
        {
            if member.backend != updated.backend
                || member.layout_slot != updated.layout_slot
                || member.pane_target != updated.pane_target
            {
                *member = updated.clone();
                changed = true;
            }
        }
    }

    if changed {
        context.ensure_write_allowed(&path).await?;
        save_teams(&path, &teams)?;
    }
    Ok(())
}

pub async fn persist_team_runtime_reconciliation(
    context: &impl TeamRuntimePersistenceContext,
    requested_name: Option<&str>,
) -> Result<()> {
    let path = team_file_path(context.working_directory());
    let mut teams = load_teams(&path)?;
    let mut changed = false;
    let mut runtime_updates = Vec::new();

    for team in &mut teams {
        if requested_name.is_some_and(|name| team.name != name) {
            continue;
        }
        let refreshed = refresh_team_record_runtime(context, team)?;
        let member_updates = changed_runtime_members(&team.members, &refreshed.members);
        if !member_updates.is_empty() {
            runtime_updates.extend(member_updates);
            *team = refreshed;
            changed = true;
        }
    }

    if changed {
        context.ensure_write_allowed(&path).await?;
        save_teams(&path, &teams)?;
        context.reconcile_member_runtime_metadata(&runtime_updates)?;
    }

    Ok(())
}

pub async fn reconcile_team_runtime_for_session(
    context: &impl TeamRuntimePersistenceContext,
    session_id: &str,
) -> Result<()> {
    let path = team_file_path(context.working_directory());
    let teams = load_teams(&path)?;
    let team_names = teams
        .iter()
        .filter(|team| {
            team.members
                .iter()
                .any(|member| member.session_id == session_id)
        })
        .map(|team| team.name.clone())
        .collect::<BTreeSet<_>>();

    for team_name in team_names {
        persist_team_runtime_reconciliation(context, Some(&team_name)).await?;
    }

    Ok(())
}

pub async fn persist_team_runtime_value_for_session(
    context: &impl TeamRuntimePersistenceContext,
    session_id: &str,
    value: &Value,
) -> Result<()> {
    let path = team_file_path(context.working_directory());
    let mut teams = load_teams(&path)?;
    let mut changed = false;

    for team in &mut teams {
        for member in &mut team.members {
            if member.session_id != session_id {
                continue;
            }
            let updated = updated_member_from_agent_value(member, value);
            if member.backend != updated.backend
                || member.layout_slot != updated.layout_slot
                || member.pane_target != updated.pane_target
            {
                *member = updated;
                changed = true;
            }
        }
    }

    if changed {
        context.ensure_write_allowed(&path).await?;
        save_teams(&path, &teams)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::team_runtime_reconciliation::TeamMemberRuntimeSnapshot;
    use crate::team_storage::{save_teams, TeamLayoutRecord};

    #[derive(Clone)]
    struct TestContext {
        root: PathBuf,
        background: Arc<Mutex<BTreeMap<String, TeamMemberRuntimeSnapshot>>>,
        reconciled: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl TeamRuntimePersistenceContext for TestContext {
        fn working_directory(&self) -> &Path {
            &self.root
        }

        async fn ensure_write_allowed(&self, _path: &Path) -> Result<()> {
            Ok(())
        }

        fn reconcile_member_runtime_metadata(
            &self,
            updated_members: &[TeamMemberRecord],
        ) -> Result<()> {
            self.reconciled.lock().expect("lock reconciled").extend(
                updated_members
                    .iter()
                    .map(|member| member.session_id.clone()),
            );
            Ok(())
        }
    }

    impl TeamRuntimeSnapshotProvider for TestContext {
        fn background_runtime_snapshot(
            &self,
            session_id: &str,
        ) -> Result<Option<TeamMemberRuntimeSnapshot>> {
            Ok(self
                .background
                .lock()
                .expect("lock background")
                .get(session_id)
                .cloned())
        }

        fn persisted_runtime_snapshot(
            &self,
            _session_id: &str,
        ) -> Result<Option<TeamMemberRuntimeSnapshot>> {
            Ok(None)
        }
    }

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "hellox-team-runtime-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
            ));
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
    async fn resolve_team_selection_persists_refreshed_runtime_metadata() {
        let workspace = TestWorkspace::new();
        let path = team_file_path(&workspace.root);
        save_teams(
            &path,
            &[TeamRecord {
                name: "reviewers".to_string(),
                layout: TeamLayoutRecord::default(),
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
            root: workspace.root.clone(),
            background: Arc::new(Mutex::new(BTreeMap::from([(
                "session-a".to_string(),
                TeamMemberRuntimeSnapshot {
                    backend: Some("detached_process".to_string()),
                    layout_slot: Some("primary".to_string()),
                    pane_target: None,
                },
            )]))),
            reconciled: Arc::new(Mutex::new(Vec::new())),
        };

        let selection = resolve_team_selection(&context, "reviewers", None)
            .await
            .expect("resolve team");
        assert_eq!(
            selection.members[0].backend.as_deref(),
            Some("detached_process")
        );

        let stored = load_teams(&path).expect("load teams");
        assert_eq!(
            stored[0].members[0].backend.as_deref(),
            Some("detached_process")
        );
        assert_eq!(
            context
                .reconciled
                .lock()
                .expect("lock reconciled")
                .as_slice(),
            ["session-a"]
        );
    }

    #[tokio::test]
    async fn persist_team_runtime_value_for_session_updates_matching_member() {
        let workspace = TestWorkspace::new();
        let path = team_file_path(&workspace.root);
        save_teams(
            &path,
            &[TeamRecord {
                name: "reviewers".to_string(),
                layout: TeamLayoutRecord::default(),
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
            root: workspace.root.clone(),
            background: Arc::new(Mutex::new(BTreeMap::new())),
            reconciled: Arc::new(Mutex::new(Vec::new())),
        };

        persist_team_runtime_value_for_session(
            &context,
            "session-a",
            &json!({
                "backend": "tmux_pane",
                "layout_slot": "right",
                "pane_target": "%2"
            }),
        )
        .await
        .expect("persist runtime value");

        let stored = load_teams(&path).expect("load teams");
        assert_eq!(stored[0].members[0].backend.as_deref(), Some("tmux_pane"));
        assert_eq!(stored[0].members[0].layout_slot.as_deref(), Some("right"));
        assert_eq!(stored[0].members[0].pane_target.as_deref(), Some("%2"));
    }
}
