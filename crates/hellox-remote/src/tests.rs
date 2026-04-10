use axum::extract::Path as AxumPath;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use hellox_agent::StoredSessionSnapshot;
use hellox_auth::{login_account, trust_device, AuthStore};
use hellox_config::{HelloxConfig, PermissionMode};
use hellox_server::{
    DirectConnectConfig, DirectConnectRequest, ServerSessionDetail, ServerSessionSummary,
};

use crate::{
    add_remote_environment, build_remote_environment, build_teleport_plan,
    create_remote_direct_connect, format_remote_environment_detail, format_remote_environment_list,
    format_teleport_plan, list_remote_environments, list_remote_sessions, load_remote_session,
    remove_remote_environment, set_remote_environment_enabled, TeleportOverrides,
};

#[test]
fn add_list_toggle_and_remove_remote_environments() {
    let mut config = HelloxConfig::default();
    add_remote_environment(
        &mut config,
        "dev".to_string(),
        build_remote_environment(
            "https://remote.example.test".to_string(),
            Some("REMOTE_TOKEN".to_string()),
            None,
            None,
            Some("Shared dev cluster".to_string()),
        ),
    )
    .expect("add environment");

    let environments = list_remote_environments(&config);
    assert_eq!(environments.len(), 1);
    assert!(format_remote_environment_list(&environments).contains("remote.example.test"));
    assert!(format_remote_environment_detail(&environments[0]).contains("token_env: REMOTE_TOKEN"));

    set_remote_environment_enabled(&mut config, "dev", false).expect("disable environment");
    assert!(
        !config
            .remote
            .environments
            .get("dev")
            .expect("dev environment")
            .enabled
    );

    let removed = remove_remote_environment(&mut config, "dev").expect("remove environment");
    assert_eq!(removed.server_url, "https://remote.example.test");
    assert!(config.remote.environments.is_empty());
}

#[test]
fn build_teleport_plan_uses_persisted_session_context() {
    let mut config = HelloxConfig::default();
    add_remote_environment(
        &mut config,
        "dev".to_string(),
        build_remote_environment(
            "https://remote.example.test".to_string(),
            Some("REMOTE_TOKEN".to_string()),
            None,
            None,
            None,
        ),
    )
    .expect("add environment");

    let snapshot = StoredSessionSnapshot {
        session_id: "session-123".to_string(),
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

    let plan = build_teleport_plan(
        &config,
        "dev",
        Some(&snapshot),
        TeleportOverrides::default(),
    )
    .expect("build teleport plan");

    assert_eq!(plan.session_id, "session-123");
    assert_eq!(plan.model, "opus");
    assert_eq!(plan.working_directory, "D:/workspace");
    assert_eq!(
        plan.connect_url,
        "cc://remote.example.test?session_id=session-123"
    );
    assert!(format_teleport_plan(&plan).contains("source: persisted_session"));
}

#[tokio::test]
async fn remote_client_uses_account_and_device_credentials() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind remote test server");
    let address = listener.local_addr().expect("server addr");
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/sessions",
                    get(list_sessions_handler).post(create_session_handler),
                )
                .route("/sessions/{session_id}", get(show_session_handler)),
        )
        .await
        .expect("serve remote test app");
    });

    let mut config = HelloxConfig::default();
    let mut store = AuthStore::default();
    login_account(
        &mut store,
        "account-1".to_string(),
        "hellox-cloud".to_string(),
        "access-token-123".to_string(),
        Some("refresh-token-123".to_string()),
        vec!["remote:sessions".to_string()],
    );
    let device = trust_device(
        &mut store,
        "account-1",
        "Workstation".to_string(),
        vec!["remote:sessions".to_string()],
    )
    .expect("trust device");
    add_remote_environment(
        &mut config,
        "dev".to_string(),
        build_remote_environment(
            format!("http://{address}"),
            None,
            Some("account-1".to_string()),
            Some(device.device_id.clone()),
            Some("Shared dev".to_string()),
        ),
    )
    .expect("add environment");

    let direct = tokio::task::spawn_blocking({
        let config = config.clone();
        let store = store.clone();
        move || {
            create_remote_direct_connect(
                &config,
                &store,
                "dev",
                DirectConnectRequest {
                    session_id: Some("session-123".to_string()),
                    model: Some("opus".to_string()),
                    working_directory: Some("D:/workspace".to_string()),
                    base_url: None,
                },
            )
            .expect("create remote direct connect")
        }
    })
    .await
    .expect("join direct connect");
    assert_eq!(direct.owner_account_id.as_deref(), Some("account-1"));
    assert!(direct.auth_token.is_some());

    let listed = tokio::task::spawn_blocking({
        let config = config.clone();
        let store = store.clone();
        move || list_remote_sessions(&config, &store, "dev").expect("list remote sessions")
    })
    .await
    .expect("join list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, "session-123");

    let detail = tokio::task::spawn_blocking({
        let config = config.clone();
        let store = store.clone();
        move || {
            load_remote_session(&config, &store, "dev", "session-123").expect("load remote session")
        }
    })
    .await
    .expect("join detail");
    assert_eq!(detail.summary.session_id, "session-123");
    assert_eq!(detail.summary.owner_account_id, "account-1");
}

async fn create_session_handler(
    headers: HeaderMap,
    Json(request): Json<DirectConnectRequest>,
) -> Json<DirectConnectConfig> {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer access-token-123")
    );
    assert!(headers.get("x-hellox-device-token").is_some());
    Json(DirectConnectConfig {
        server_url: "http://remote.example.test".to_string(),
        connect_url: "cc://remote.example.test?session_id=session-123&auth_token=session-token"
            .to_string(),
        session_id: request.session_id.expect("session id"),
        model: request.model.expect("model"),
        working_directory: request.working_directory.expect("cwd"),
        source: "persisted_session".to_string(),
        auth_token: Some("session-token".to_string()),
        owner_account_id: Some("account-1".to_string()),
        owner_device_id: Some("device-1".to_string()),
    })
}

async fn list_sessions_handler(headers: HeaderMap) -> Json<Vec<ServerSessionSummary>> {
    assert!(headers.get("authorization").is_some());
    Json(vec![ServerSessionSummary {
        session_id: "session-123".to_string(),
        model: "opus".to_string(),
        working_directory: "D:/workspace".to_string(),
        source: "persisted_session".to_string(),
        owner_account_id: "account-1".to_string(),
        owner_device_id: Some("device-1".to_string()),
        created_at: 1,
        updated_at: 2,
        persisted: true,
    }])
}

async fn show_session_handler(
    AxumPath(session_id): AxumPath<String>,
    headers: HeaderMap,
) -> Json<ServerSessionDetail> {
    assert!(headers.get("authorization").is_some());
    Json(ServerSessionDetail {
        summary: ServerSessionSummary {
            session_id,
            model: "opus".to_string(),
            working_directory: "D:/workspace".to_string(),
            source: "persisted_session".to_string(),
            owner_account_id: "account-1".to_string(),
            owner_device_id: Some("device-1".to_string()),
            created_at: 1,
            updated_at: 2,
            persisted: true,
        },
        owner_device_name: Some("Workstation".to_string()),
        permission_mode: Some("accept_edits".to_string()),
        shell_name: Some("powershell".to_string()),
        system_prompt: Some("system".to_string()),
        message_count: 1,
    })
}
