use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::StoredSessionSnapshot;
use hellox_auth::RemoteIdentity;
use hellox_config::{save_config, HelloxConfig, PermissionMode};
use hellox_sync::{SettingsSyncSnapshot, TeamMemoryEntry, TeamMemorySnapshot};

use crate::state::{build_state, create_owned_session, save_settings_snapshot, sync_team_memory};
use crate::{
    inspect_managed_settings, inspect_policy_limits, inspect_registered_session,
    inspect_registered_sessions, inspect_synced_settings, inspect_synced_team_memory,
    set_managed_settings, set_policy_limits, DirectConnectRequest,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-server-admin-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn write_snapshot(root: &Path, session_id: &str) {
    let snapshot = StoredSessionSnapshot {
        session_id: session_id.to_string(),
        model: "opus".to_string(),
        permission_mode: Some(PermissionMode::AcceptEdits),
        output_style_name: None,
        output_style: None,
        persona: None,
        prompt_fragments: Vec::new(),
        config_path: None,
        planning: hellox_agent::PlanningState::default(),
        working_directory: "D:\\workspace".to_string(),
        shell_name: "powershell".to_string(),
        system_prompt: "system".to_string(),
        created_at: 1,
        updated_at: 2,
        agent_runtime: None,
        usage_by_model: Default::default(),
        messages: Vec::new(),
    };
    let raw = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");
    fs::create_dir_all(root).expect("create session root");
    fs::write(root.join(format!("{session_id}.json")), raw).expect("write snapshot");
}

fn config_path(root: &Path) -> PathBuf {
    root.join(".hellox").join("config.toml")
}

fn prepare_config(root: &Path) -> PathBuf {
    let path = config_path(root);
    let mut config = HelloxConfig::default();
    config.server.listen = "127.0.0.1:7831".to_string();
    save_config(Some(path.clone()), &config).expect("save config");
    path
}

fn remote_identity(
    account_id: &str,
    device_id: Option<&str>,
    device_name: Option<&str>,
) -> RemoteIdentity {
    RemoteIdentity {
        account_id: account_id.to_string(),
        provider: "hellox-cloud".to_string(),
        device_id: device_id.map(ToString::to_string),
        device_name: device_name.map(ToString::to_string),
    }
}

#[test]
fn admin_roundtrips_managed_settings_and_policy_limits() {
    let root = temp_dir();
    let config = prepare_config(&root);

    let managed = set_managed_settings(
        Some(config.clone()),
        "[permissions]\nmode = \"accept_edits\"\n".to_string(),
        Some(" sig-123 ".to_string()),
    )
    .expect("set managed settings");
    assert_eq!(managed.signature.as_deref(), Some("sig-123"));
    assert!(managed.updated_at > 0);

    let managed_inspected = inspect_managed_settings(Some(config.clone()))
        .expect("inspect managed settings")
        .expect("managed settings document");
    assert_eq!(managed_inspected, managed);

    let policy = set_policy_limits(
        Some(config.clone()),
        vec![
            " plugin ".to_string(),
            "".to_string(),
            "session".to_string(),
        ],
        vec![" bash ".to_string(), " ".to_string()],
        Some(" enterprise policy ".to_string()),
    )
    .expect("set policy limits");
    assert_eq!(
        policy.disabled_commands,
        vec![String::from("plugin"), String::from("session")]
    );
    assert_eq!(policy.disabled_tools, vec![String::from("bash")]);
    assert_eq!(policy.notes.as_deref(), Some("enterprise policy"));

    let policy_inspected = inspect_policy_limits(Some(config))
        .expect("inspect policy limits")
        .expect("policy limits document");
    assert_eq!(policy_inspected, policy);
}

#[test]
fn admin_inspects_registered_sessions() {
    let root = temp_dir();
    let config = prepare_config(&root);
    write_snapshot(&root.join(".hellox").join("sessions"), "session-123");

    let state = build_state(Some(config.clone())).expect("build state");
    let persisted = create_owned_session(
        &state,
        &remote_identity("account-1", Some("device-1"), Some("Workstation")),
        DirectConnectRequest {
            session_id: Some("session-123".to_string()),
            model: None,
            working_directory: None,
            base_url: None,
        },
    )
    .expect("create persisted session");
    assert_eq!(persisted.owner_account_id.as_deref(), Some("account-1"));
    assert_eq!(persisted.owner_device_id.as_deref(), Some("device-1"));
    assert!(persisted.auth_token.is_some());

    let ad_hoc = create_owned_session(
        &state,
        &remote_identity("account-2", None, None),
        DirectConnectRequest {
            session_id: None,
            model: Some("sonnet".to_string()),
            working_directory: Some("D:/repo".to_string()),
            base_url: None,
        },
    )
    .expect("create ad hoc session");
    assert_eq!(ad_hoc.model, "sonnet");
    assert_eq!(ad_hoc.working_directory, "D:/repo");

    let sessions = inspect_registered_sessions(Some(config.clone())).expect("inspect sessions");
    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().any(|session| {
        session.session_id == "session-123"
            && session.owner_account_id == "account-1"
            && session.persisted
    }));
    assert!(sessions.iter().any(|session| {
        session.owner_account_id == "account-2" && session.model == "sonnet" && !session.persisted
    }));

    let detail = inspect_registered_session(Some(config), "session-123").expect("inspect detail");
    assert_eq!(detail.summary.session_id, "session-123");
    assert_eq!(detail.summary.owner_account_id, "account-1");
    assert_eq!(detail.owner_device_name.as_deref(), Some("Workstation"));
    assert_eq!(detail.permission_mode.as_deref(), Some("accept_edits"));
    assert_eq!(detail.shell_name.as_deref(), Some("powershell"));
    assert_eq!(detail.system_prompt.as_deref(), Some("system"));
    assert_eq!(detail.message_count, 0);
}

#[test]
fn admin_inspects_synced_settings_and_team_memory() {
    let root = temp_dir();
    let config = prepare_config(&root);
    let state = build_state(Some(config.clone())).expect("build state");
    let identity = remote_identity("account-1", Some("device-1"), Some("Laptop"));

    let settings = SettingsSyncSnapshot {
        exported_at: 10,
        config_toml: "[server]\nlisten = \"127.0.0.1:9000\"\n".to_string(),
    };
    save_settings_snapshot(&state, &identity, &settings).expect("save settings snapshot");

    let stored_settings = inspect_synced_settings(Some(config.clone()), "account-1")
        .expect("inspect settings snapshot")
        .expect("settings snapshot");
    assert_eq!(stored_settings, settings);

    let team_memory = sync_team_memory(
        &state,
        &identity,
        "repo-1",
        TeamMemorySnapshot {
            repo_id: "repo-1".to_string(),
            exported_at: 11,
            entries: [(
                "architecture".to_string(),
                TeamMemoryEntry {
                    content: "keep it simple".to_string(),
                    updated_at: 5,
                },
            )]
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
        },
    )
    .expect("sync team memory");
    assert_eq!(team_memory.repo_id, "repo-1");
    assert!(team_memory.exported_at > 0);

    let stored_team_memory = inspect_synced_team_memory(Some(config), "account-1", "repo-1")
        .expect("inspect team memory")
        .expect("team memory snapshot");
    assert_eq!(stored_team_memory.repo_id, "repo-1");
    assert_eq!(
        stored_team_memory.entries.get("architecture"),
        Some(&TeamMemoryEntry {
            content: "keep it simple".to_string(),
            updated_at: 5,
        })
    );
}
