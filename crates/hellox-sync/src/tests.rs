use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, put};
use axum::{Json, Router};
use hellox_config::{save_config, HelloxConfig};
use serde_json::json;

use crate::{
    export_settings_snapshot, fetch_cached_document, format_team_memory_snapshot,
    import_settings_snapshot, load_team_memory_snapshot_from, merge_team_memory_snapshot_in,
    persist_cached_document, put_team_memory_entry_in, remove_team_memory_entry_in,
    ManagedSettingsDocument, PolicyLimitsDocument, RemoteFetch, RemoteSyncClient,
    SettingsSyncSnapshot, TeamMemoryEntry, TeamMemorySnapshot,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-sync-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

#[test]
fn exports_and_imports_settings_snapshot() {
    let root = temp_dir();
    let config_path = root.join("config.toml");
    let mut config = HelloxConfig::default();
    config.server.listen = "127.0.0.1:9000".to_string();
    save_config(Some(config_path.clone()), &config).expect("save config");

    let snapshot = export_settings_snapshot(Some(config_path.clone())).expect("export snapshot");
    assert!(snapshot.config_toml.contains("[server]"));

    let imported_path =
        import_settings_snapshot(Some(root.join("imported.toml")), &snapshot).expect("import");
    let imported = fs::read_to_string(imported_path).expect("read imported config");
    assert!(imported.contains("127.0.0.1:9000"));
}

#[test]
fn team_memory_snapshot_put_merge_and_remove() {
    let root = temp_dir();
    let repo_id = format!(
        "repo-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    );

    let created = put_team_memory_entry_in(
        &root,
        &repo_id,
        "architecture".to_string(),
        "keep it simple".to_string(),
    )
    .expect("put entry");
    assert!(format_team_memory_snapshot(&created).contains("architecture"));

    let merged = merge_team_memory_snapshot_in(
        &root,
        &repo_id,
        TeamMemorySnapshot {
            repo_id: repo_id.clone(),
            exported_at: 1,
            entries: [(
                "testing".to_string(),
                crate::TeamMemoryEntry {
                    content: "cargo test".to_string(),
                    updated_at: 2,
                },
            )]
            .into_iter()
            .collect(),
        },
    )
    .expect("merge snapshot");
    assert!(merged.entries.contains_key("architecture"));
    assert!(merged.entries.contains_key("testing"));

    let removed =
        remove_team_memory_entry_in(&root, &repo_id, "architecture").expect("remove entry");
    assert!(!removed.entries.contains_key("architecture"));
    assert!(load_team_memory_snapshot_from(&root, &repo_id)
        .expect("reload snapshot")
        .entries
        .contains_key("testing"));
}

#[tokio::test]
async fn remote_sync_client_handles_snapshots_and_etag_documents() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind sync test server");
    let address = listener.local_addr().expect("sync server addr");
    let state = Arc::new(Mutex::new(ServerState {
        settings: None,
        team_memory: TeamMemorySnapshot {
            repo_id: "repo-1".to_string(),
            exported_at: 0,
            entries: Default::default(),
        },
    }));
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/sync/settings",
                    get(get_settings_handler).put(put_settings_handler),
                )
                .route("/sync/team-memory/{repo_id}", put(put_team_memory_handler))
                .route("/managed-settings", get(get_managed_settings_handler))
                .route("/policy-limits", get(get_policy_limits_handler))
                .with_state(state),
        )
        .await
        .expect("serve sync test app");
    });

    let client = RemoteSyncClient::new(
        format!("http://{address}"),
        "access-token-123",
        Some("device-token-123".to_string()),
    );

    let uploaded = tokio::task::spawn_blocking({
        let client = client.clone();
        move || {
            client
                .push_settings_snapshot(&SettingsSyncSnapshot {
                    exported_at: 1,
                    config_toml: "[gateway]\nlisten = \"127.0.0.1:9000\"\n".to_string(),
                })
                .expect("push settings")
        }
    })
    .await
    .expect("join settings push");
    assert!(uploaded.config_toml.contains("127.0.0.1:9000"));

    let pulled = tokio::task::spawn_blocking({
        let client = client.clone();
        move || client.pull_settings_snapshot().expect("pull settings")
    })
    .await
    .expect("join settings pull");
    assert!(pulled.is_some());

    let synced = tokio::task::spawn_blocking({
        let client = client.clone();
        move || {
            client
                .sync_team_memory_snapshot(
                    "repo-1",
                    &TeamMemorySnapshot {
                        repo_id: "repo-1".to_string(),
                        exported_at: 2,
                        entries: [(
                            "architecture".to_string(),
                            TeamMemoryEntry {
                                content: "keep it simple".to_string(),
                                updated_at: 10,
                            },
                        )]
                        .into_iter()
                        .collect(),
                    },
                )
                .expect("sync team memory")
        }
    })
    .await
    .expect("join team memory sync");
    assert!(synced.entries.contains_key("architecture"));

    let managed = tokio::task::spawn_blocking({
        let client = client.clone();
        move || {
            client
                .fetch_managed_settings(None)
                .expect("fetch managed settings")
        }
    })
    .await
    .expect("join managed fetch");
    let managed = match managed {
        RemoteFetch::Updated(document) => document,
        other => panic!("unexpected managed fetch result: {other:?}"),
    };
    let cache_path = temp_dir().join("managed-cache.json");
    persist_cached_document(&cache_path, "dev", &managed).expect("persist cache");
    let cached = fetch_cached_document::<ManagedSettingsDocument>(&cache_path)
        .expect("read cache")
        .expect("cached document");
    assert_eq!(cached.etag.as_deref(), Some("etag-managed"));

    let not_modified = tokio::task::spawn_blocking({
        let client = client.clone();
        move || {
            client
                .fetch_managed_settings(Some("etag-managed"))
                .expect("refetch managed settings")
        }
    })
    .await
    .expect("join managed not modified");
    assert!(matches!(not_modified, RemoteFetch::NotModified { .. }));

    let policy = tokio::task::spawn_blocking(move || {
        client
            .fetch_policy_limits(None)
            .expect("fetch policy limits")
    })
    .await
    .expect("join policy fetch");
    match policy {
        RemoteFetch::Updated(document) => {
            assert_eq!(document.etag.as_deref(), Some("etag-policy"));
            assert_eq!(
                document.value.disabled_commands,
                vec![String::from("plugin")]
            );
        }
        other => panic!("unexpected policy fetch result: {other:?}"),
    }
}

#[derive(Default)]
struct ServerState {
    settings: Option<SettingsSyncSnapshot>,
    team_memory: TeamMemorySnapshot,
}

async fn put_settings_handler(
    State(state): State<Arc<Mutex<ServerState>>>,
    headers: HeaderMap,
    Json(snapshot): Json<SettingsSyncSnapshot>,
) -> Json<SettingsSyncSnapshot> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer access-token-123")
    );
    state.lock().expect("state lock").settings = Some(snapshot.clone());
    Json(snapshot)
}

async fn get_settings_handler(
    State(state): State<Arc<Mutex<ServerState>>>,
    headers: HeaderMap,
) -> Result<Json<SettingsSyncSnapshot>, StatusCode> {
    assert!(headers.get("x-hellox-device-token").is_some());
    state
        .lock()
        .expect("state lock")
        .settings
        .clone()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn put_team_memory_handler(
    State(state): State<Arc<Mutex<ServerState>>>,
    AxumPath(repo_id): AxumPath<String>,
    Json(snapshot): Json<TeamMemorySnapshot>,
) -> Json<TeamMemorySnapshot> {
    let mut guard = state.lock().expect("state lock");
    guard.team_memory.repo_id = repo_id;
    for (key, value) in snapshot.entries {
        guard.team_memory.entries.insert(key, value);
    }
    Json(guard.team_memory.clone())
}

async fn get_managed_settings_handler(headers: HeaderMap) -> (StatusCode, HeaderMap, String) {
    if headers
        .get("if-none-match")
        .and_then(|value| value.to_str().ok())
        == Some("etag-managed")
    {
        return (StatusCode::NOT_MODIFIED, HeaderMap::new(), String::new());
    }
    let mut response_headers = HeaderMap::new();
    response_headers.insert("etag", "etag-managed".parse().expect("etag header"));
    (
        StatusCode::OK,
        response_headers,
        json!(ManagedSettingsDocument {
            updated_at: 3,
            config_toml: "[permissions]\nmode = \"accept_edits\"\n".to_string(),
            signature: Some("sig-123".to_string()),
        })
        .to_string(),
    )
}

async fn get_policy_limits_handler() -> (StatusCode, HeaderMap, String) {
    let mut response_headers = HeaderMap::new();
    response_headers.insert("etag", "etag-policy".parse().expect("etag header"));
    (
        StatusCode::OK,
        response_headers,
        json!(PolicyLimitsDocument {
            updated_at: 4,
            disabled_commands: vec!["plugin".to_string()],
            disabled_tools: vec!["bash".to_string()],
            notes: Some("policy".to_string()),
        })
        .to_string(),
    )
}
