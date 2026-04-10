use std::env;
use std::fs;
use std::future::IntoFuture;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{AgentOptions, AgentSession};
use crate::client::GatewayClient;
use crate::planning::PlanItem;
use crate::tools::ToolRegistry;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use hellox_config::PermissionMode;
use hellox_gateway_api::{ContentBlock, DocumentSource, MessageContent, MessageRole};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn temp_workspace() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-agent-session-{suffix}"));
    fs::create_dir_all(&root).expect("create temp workspace");
    root
}

#[test]
fn effective_system_prompt_includes_planning_guidance() {
    let workspace = temp_workspace();
    let session = AgentSession::create(
        GatewayClient::new("http://127.0.0.1:1"),
        ToolRegistry::default(),
        workspace.join(".hellox").join("config.toml"),
        workspace.clone(),
        "powershell",
        AgentOptions::default(),
        PermissionMode::Default,
        None,
        None,
        false,
        None,
    );

    session.context.enter_plan_mode().expect("enter plan mode");
    let active_prompt = session.effective_system_prompt();
    assert!(active_prompt.contains("plan_mode: active"));

    session
        .context
        .exit_plan_mode(
            vec![PlanItem {
                step: "Implement task tools".to_string(),
                status: "completed".to_string(),
            }],
            vec![String::from("continue implementation")],
        )
        .expect("exit plan mode");
    let stored_prompt = session.effective_system_prompt();
    assert!(stored_prompt.contains("accepted_plan"));
    assert!(stored_prompt.contains("Implement task tools"));
    assert!(stored_prompt.contains("continue implementation"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn effective_system_prompt_includes_workspace_brief_when_present() {
    let workspace = temp_workspace();
    fs::create_dir_all(workspace.join(".hellox")).expect("create .hellox dir");
    fs::write(
        workspace.join(".hellox").join("brief.json"),
        r#"{
  "message": "Keep outputs concise and focus on local-first implementations.",
  "attachments": [{ "path": "notes/review.md", "label": "review" }],
  "status": "in_progress",
  "updated_at": 42
}
"#,
    )
    .expect("write brief file");

    let session = AgentSession::create(
        GatewayClient::new("http://127.0.0.1:1"),
        ToolRegistry::default(),
        workspace.join(".hellox").join("config.toml"),
        workspace.clone(),
        "powershell",
        AgentOptions::default(),
        PermissionMode::Default,
        None,
        None,
        false,
        None,
    );

    let prompt = session.effective_system_prompt();
    assert!(prompt.contains("# Workspace brief"), "{prompt}");
    assert!(prompt.contains("Keep outputs concise"), "{prompt}");
    assert!(prompt.contains("notes/review.md (review)"), "{prompt}");

    let _ = fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn injects_workspace_brief_attachments_by_uploading_to_gateway() {
    let workspace = temp_workspace();
    fs::create_dir_all(workspace.join(".hellox")).expect("create .hellox dir");
    fs::create_dir_all(workspace.join("notes")).expect("create notes dir");
    fs::write(
        workspace.join("notes").join("review.md"),
        "hello from attachment",
    )
    .expect("write attachment file");
    fs::write(
        workspace.join(".hellox").join("brief.json"),
        r#"{
  "message": "Project notes",
  "attachments": [{ "path": "notes/review.md", "label": "review" }],
  "status": "in_progress",
  "updated_at": 42
}
"#,
    )
    .expect("write brief file");

    let hits = Arc::new(AtomicUsize::new(0));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let base_url = format!("http://{}", addr);

    async fn handle_upload(State(hits): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
        let idx = hits.fetch_add(1, Ordering::SeqCst) + 1;
        Json(json!({ "id": format!("file_test_{idx}") }))
    }

    let app = Router::new()
        .route("/v1/files", post(handle_upload))
        .with_state(hits.clone());

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
    });
    tokio::spawn(server.into_future());

    let mut session = AgentSession::create(
        GatewayClient::new(base_url),
        ToolRegistry::default(),
        workspace.join(".hellox").join("config.toml"),
        workspace.clone(),
        "powershell",
        AgentOptions::default(),
        PermissionMode::Default,
        None,
        None,
        false,
        None,
    );

    session
        .maybe_inject_brief_attachments()
        .await
        .expect("inject attachments");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let messages = session.messages();
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0].role, MessageRole::User));
    let MessageContent::Blocks(blocks) = &messages[0].content else {
        panic!("expected blocks message");
    };

    let file_ids = blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Document {
                source: DocumentSource::File { file_id },
                ..
            } => Some(file_id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(file_ids, vec![String::from("file_test_1")]);

    let _ = shutdown_tx.send(());
    let _ = fs::remove_dir_all(workspace);
}
