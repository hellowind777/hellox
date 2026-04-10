use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::StoredSessionSnapshot;
use hellox_auth::{login_account, save_auth_store, trust_device, AuthStore};
use hellox_config::{save_config, HelloxConfig, PermissionMode};
use hellox_sync::{
    ManagedSettingsDocument, PolicyLimitsDocument, SettingsSyncSnapshot, TeamMemoryEntry,
    TeamMemorySnapshot,
};
use reqwest::StatusCode;

use crate::{
    build_state, create_direct_connect_config, format_direct_connect_config, format_server_status,
    inspect_server_status, persist_managed_settings, persist_policy_limits, DirectConnectRequest,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-server-{suffix}"));
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

#[test]
fn inspect_server_status_reports_base_url() {
    let root = temp_dir();
    let config = prepare_config(&root);

    let status = inspect_server_status(Some(config)).expect("inspect server status");
    assert_eq!(status.listen, "127.0.0.1:7831");
    assert_eq!(status.base_url, "http://127.0.0.1:7831");
    assert!(format_server_status(&status).contains("base_url: http://127.0.0.1:7831"));
}

#[test]
fn create_direct_connect_config_uses_persisted_session_when_available() {
    let root = temp_dir();
    let config = prepare_config(&root);
    write_snapshot(&root.join(".hellox").join("sessions"), "session-123");

    let direct = create_direct_connect_config(
        Some(config),
        DirectConnectRequest {
            session_id: Some("session-123".to_string()),
            model: None,
            working_directory: None,
            base_url: Some("http://127.0.0.1:9000".to_string()),
        },
    )
    .expect("create direct connect config");

    assert_eq!(direct.session_id, "session-123");
    assert_eq!(direct.model, "opus");
    assert_eq!(direct.working_directory, "D:/workspace");
    assert_eq!(
        direct.connect_url,
        "cc://127.0.0.1:9000?session_id=session-123"
    );
    assert!(format_direct_connect_config(&direct).contains("source: persisted_session"));
}

#[test]
fn create_direct_connect_config_generates_ad_hoc_session() {
    let root = temp_dir();
    let config = prepare_config(&root);

    let direct = create_direct_connect_config(
        Some(config),
        DirectConnectRequest {
            session_id: None,
            model: Some("sonnet".to_string()),
            working_directory: Some("D:/repo".to_string()),
            base_url: None,
        },
    )
    .expect("create ad hoc direct connect config");

    assert_eq!(direct.model, "sonnet");
    assert_eq!(direct.working_directory, "D:/repo");
    assert_eq!(direct.server_url, "http://127.0.0.1:7831");
    assert!(direct.session_id.len() > 10);
    assert_eq!(direct.source, "ad_hoc");
}

#[tokio::test]
async fn http_remote_endpoints_support_owned_sessions_and_sync_documents() {
    let root = temp_dir();
    let config = prepare_config(&root);
    write_snapshot(&root.join(".hellox").join("sessions"), "session-123");

    let auth_store_path = root.join(".hellox").join("oauth-tokens.json");
    let provider_keys_path = root.join(".hellox").join("provider-keys.json");
    let mut auth_store = AuthStore::default();
    login_account(
        &mut auth_store,
        "account-1".to_string(),
        "hellox-cloud".to_string(),
        "access-token-123".to_string(),
        Some("refresh-token-123".to_string()),
        vec!["remote:sessions".to_string()],
    );
    let device = trust_device(
        &mut auth_store,
        "account-1",
        "Workstation".to_string(),
        vec!["remote:sessions".to_string()],
    )
    .expect("trust device");
    save_auth_store(Some(auth_store_path), Some(provider_keys_path), &auth_store)
        .expect("save auth store");

    let state = build_state(Some(config)).expect("build server state");
    persist_managed_settings(
        &state,
        &ManagedSettingsDocument {
            updated_at: 1,
            config_toml: "[permissions]\nmode = \"accept_edits\"\n".to_string(),
            signature: Some("sig-123".to_string()),
        },
    )
    .expect("persist managed settings");
    persist_policy_limits(
        &state,
        &PolicyLimitsDocument {
            updated_at: 2,
            disabled_commands: vec!["plugin".to_string()],
            disabled_tools: vec!["bash".to_string()],
            notes: Some("enterprise policy".to_string()),
        },
    )
    .expect("persist policy limits");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("server addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::http::router(state))
            .await
            .expect("serve test router");
    });

    let client = reqwest::Client::new();
    let base = format!("http://{address}");

    let created = client
        .post(format!("{base}/sessions"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .json(&DirectConnectRequest {
            session_id: Some("session-123".to_string()),
            model: None,
            working_directory: None,
            base_url: None,
        })
        .send()
        .await
        .expect("create session");
    assert_eq!(created.status(), StatusCode::OK);
    let direct: crate::DirectConnectConfig = created.json().await.expect("parse direct connect");
    assert_eq!(direct.owner_account_id.as_deref(), Some("account-1"));
    assert_eq!(
        direct.owner_device_id.as_deref(),
        Some(device.device_id.as_str())
    );
    assert!(direct.auth_token.is_some());

    let sessions = client
        .get(format!("{base}/sessions"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .send()
        .await
        .expect("list sessions");
    assert_eq!(sessions.status(), StatusCode::OK);
    let sessions: Vec<crate::ServerSessionSummary> =
        sessions.json().await.expect("parse session list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "session-123");

    let detail = client
        .get(format!("{base}/sessions/session-123"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .send()
        .await
        .expect("show session");
    assert_eq!(detail.status(), StatusCode::OK);
    let detail: crate::ServerSessionDetail = detail.json().await.expect("parse session detail");
    assert_eq!(detail.summary.owner_account_id, "account-1");
    assert_eq!(detail.message_count, 0);

    let stored_settings = client
        .put(format!("{base}/sync/settings"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .json(&SettingsSyncSnapshot {
            exported_at: 10,
            config_toml: "[gateway]\nlisten = \"127.0.0.1:9000\"\n".to_string(),
        })
        .send()
        .await
        .expect("put settings");
    assert_eq!(stored_settings.status(), StatusCode::OK);

    let fetched_settings = client
        .get(format!("{base}/sync/settings"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .send()
        .await
        .expect("get settings");
    assert_eq!(fetched_settings.status(), StatusCode::OK);
    let fetched_settings: SettingsSyncSnapshot =
        fetched_settings.json().await.expect("parse settings");
    assert!(fetched_settings.config_toml.contains("127.0.0.1:9000"));

    let synced_memory = client
        .put(format!("{base}/sync/team-memory/repo-1"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .json(&TeamMemorySnapshot {
            repo_id: "repo-1".to_string(),
            exported_at: 12,
            entries: [(
                "architecture".to_string(),
                TeamMemoryEntry {
                    content: "keep it simple".to_string(),
                    updated_at: 5,
                },
            )]
            .into_iter()
            .collect(),
        })
        .send()
        .await
        .expect("sync team memory");
    assert_eq!(synced_memory.status(), StatusCode::OK);
    let synced_memory: TeamMemorySnapshot = synced_memory.json().await.expect("parse team memory");
    assert!(synced_memory.entries.contains_key("architecture"));

    let managed = client
        .get(format!("{base}/managed-settings"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .send()
        .await
        .expect("get managed settings");
    assert_eq!(managed.status(), StatusCode::OK);
    let managed_etag = managed
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
        .expect("managed etag");
    let managed: ManagedSettingsDocument = managed.json().await.expect("parse managed settings");
    assert_eq!(managed.signature.as_deref(), Some("sig-123"));

    let managed_not_modified = client
        .get(format!("{base}/managed-settings"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .header("if-none-match", managed_etag)
        .send()
        .await
        .expect("reget managed settings");
    assert_eq!(managed_not_modified.status(), StatusCode::NOT_MODIFIED);

    let policy = client
        .get(format!("{base}/policy-limits"))
        .bearer_auth("access-token-123")
        .header("x-hellox-device-token", &device.device_token)
        .send()
        .await
        .expect("get policy limits");
    assert_eq!(policy.status(), StatusCode::OK);
    let policy: PolicyLimitsDocument = policy.json().await.expect("parse policy limits");
    assert_eq!(policy.disabled_commands, vec![String::from("plugin")]);

    server.abort();
}
