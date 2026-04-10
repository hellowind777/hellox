use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use hellox_agent::{DetachedAgentJob, StoredAgentRuntime, StoredSession};

use crate::auto_compact::maybe_auto_compact_session;
use crate::auto_memory::{format_auto_memory_refresh_notice, maybe_auto_refresh_session_memory};
use crate::build_session;

pub(crate) async fn run_worker_job(job_path: PathBuf) -> Result<()> {
    let job = DetachedAgentJob::load(&job_path)?;
    let _ = fs::remove_file(&job_path);

    match build_session(
        job.config_path.clone().map(PathBuf::from),
        None,
        None,
        None,
        Some(job.session_id.clone()),
        job.max_turns,
    ) {
        Ok(mut bootstrap) => match bootstrap.session.run_user_prompt(job.prompt).await {
            Ok(result) => {
                let memory_maintenance_note = match maybe_auto_compact_session(
                    &mut bootstrap.session,
                    &bootstrap.repl_metadata.memory_root,
                ) {
                    Ok(Some(outcome)) => Some(format!(
                        "Note: {}",
                        crate::auto_compact::format_auto_compact_notice(&outcome)
                    )),
                    Ok(None) => match maybe_auto_refresh_session_memory(
                        &bootstrap.session,
                        &bootstrap.repl_metadata.memory_root,
                    ) {
                        Ok(Some(outcome)) => Some(format!(
                            "Note: {}",
                            format_auto_memory_refresh_notice(&outcome)
                        )),
                        Ok(None) => None,
                        Err(error) => Some(format!("Warning: auto-memory refresh failed: {error}")),
                    },
                    Err(error) => Some(format!("Warning: auto-compact failed: {error}")),
                };
                update_runtime(
                    &job.session_id,
                    bootstrap.session.permission_mode().clone(),
                    "completed",
                    Some(result.iterations),
                    Some(append_runtime_warning(
                        result.final_text,
                        memory_maintenance_note.as_deref(),
                    )),
                    None,
                )?;
                Ok(())
            }
            Err(error) => {
                update_runtime(
                    &job.session_id,
                    bootstrap.session.permission_mode().clone(),
                    "failed",
                    None,
                    None,
                    Some(error.to_string()),
                )?;
                Err(error)
            }
        },
        Err(error) => {
            update_runtime_without_session(&job.session_id, "failed", Some(error.to_string()))?;
            Err(error)
        }
    }
}

fn update_runtime(
    session_id: &str,
    permission_mode: hellox_config::PermissionMode,
    status: &str,
    iterations: Option<usize>,
    result: Option<String>,
    error: Option<String>,
) -> Result<()> {
    let mut stored = StoredSession::load(session_id)?;
    let existing = stored.snapshot.agent_runtime.clone();
    stored.save_runtime(StoredAgentRuntime {
        status: status.to_string(),
        background: existing
            .as_ref()
            .map(|runtime| runtime.background)
            .unwrap_or(true),
        resumed: existing
            .as_ref()
            .map(|runtime| runtime.resumed)
            .unwrap_or(true),
        backend: existing
            .as_ref()
            .and_then(|runtime| runtime.backend.clone())
            .or_else(|| Some(String::from("detached_process"))),
        permission_mode: Some(permission_mode),
        started_at: existing
            .as_ref()
            .and_then(|runtime| runtime.started_at)
            .or_else(|| Some(unix_timestamp())),
        finished_at: Some(unix_timestamp()),
        pid: existing.as_ref().and_then(|runtime| runtime.pid),
        pane_target: existing
            .as_ref()
            .and_then(|runtime| runtime.pane_target.clone()),
        layout_slot: existing
            .as_ref()
            .and_then(|runtime| runtime.layout_slot.clone()),
        iterations,
        result,
        error,
    })
}

fn update_runtime_without_session(
    session_id: &str,
    status: &str,
    error: Option<String>,
) -> Result<()> {
    let mut stored = StoredSession::load(session_id)?;
    let existing = stored.snapshot.agent_runtime.clone();
    stored.save_runtime(StoredAgentRuntime {
        status: status.to_string(),
        background: existing
            .as_ref()
            .map(|runtime| runtime.background)
            .unwrap_or(true),
        resumed: existing
            .as_ref()
            .map(|runtime| runtime.resumed)
            .unwrap_or(true),
        backend: existing
            .as_ref()
            .and_then(|runtime| runtime.backend.clone())
            .or_else(|| Some(String::from("detached_process"))),
        permission_mode: existing
            .as_ref()
            .and_then(|runtime| runtime.permission_mode.clone()),
        started_at: Some(unix_timestamp()),
        finished_at: Some(unix_timestamp()),
        pid: None,
        pane_target: existing
            .as_ref()
            .and_then(|runtime| runtime.pane_target.clone()),
        layout_slot: existing
            .as_ref()
            .and_then(|runtime| runtime.layout_slot.clone()),
        iterations: None,
        result: None,
        error,
    })
}

fn append_runtime_warning(result: String, warning: Option<&str>) -> String {
    match warning {
        Some(warning) => format!("{result}\n\n{warning}"),
        None => result,
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use hellox_agent::{
        default_tool_registry, AgentOptions, AgentSession, DetachedAgentJob, GatewayClient,
        StoredSession,
    };
    use hellox_config::{session_file_path, PermissionMode};

    use super::run_worker_job;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-cli-worker-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn write_config(root: &PathBuf, base_url: &str) -> PathBuf {
        let config = format!(
            "[gateway]\nlisten = \"{}\"\n\n[session]\npersist = true\nmodel = \"mock-model\"\n",
            base_url
        );
        let config_path = root.join(".hellox").join("config.toml");
        fs::create_dir_all(config_path.parent().expect("config dir")).expect("create config dir");
        fs::write(&config_path, config).expect("write config");
        config_path
    }

    async fn spawn_mock_gateway(response_text: &str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock gateway");
        let address = listener.local_addr().expect("local addr");
        let response_text = response_text.to_string();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.expect("accept connection");
                let mut buffer = vec![0_u8; 4096];
                let _ = stream.read(&mut buffer).await;
                let body = serde_json::json!({
                    "id": "worker-response",
                    "type": "message",
                    "role": "assistant",
                    "model": "mock-model",
                    "content": [{ "type": "text", "text": response_text }],
                    "stop_reason": "end_turn",
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 5
                    }
                })
                .to_string();
                let payload = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(payload.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });
        format!("http://{}", address)
    }

    #[tokio::test]
    async fn run_worker_job_updates_persisted_runtime() {
        let root = temp_dir();
        let base_url = spawn_mock_gateway("worker backend done").await;
        let config_path = write_config(&root, &base_url);
        let working_directory = root.join("workspace");
        fs::create_dir_all(&working_directory).expect("create workspace");

        let session_id = format!(
            "worker-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let session = AgentSession::create(
            GatewayClient::new(&base_url),
            default_tool_registry(),
            config_path.clone(),
            working_directory,
            "powershell",
            AgentOptions {
                model: "mock-model".to_string(),
                max_turns: 4,
                ..AgentOptions::default()
            },
            PermissionMode::AcceptEdits,
            None,
            None,
            true,
            Some(session_id.clone()),
        );
        drop(session);

        let job_path = root.join("job.json");
        DetachedAgentJob {
            session_id: session_id.clone(),
            session_path: session_file_path(&session_id).display().to_string(),
            prompt: "Run detached worker".to_string(),
            max_turns: 4,
            config_path: Some(config_path.display().to_string()),
        }
        .save(&job_path)
        .expect("write job");

        run_worker_job(job_path).await.expect("run worker job");

        let stored = StoredSession::load(&session_id).expect("load stored session");
        let runtime = stored.snapshot.agent_runtime.expect("agent runtime");
        assert_eq!(runtime.status, "completed");
        assert_eq!(runtime.backend.as_deref(), Some("detached_process"));
        assert_eq!(runtime.result.as_deref(), Some("worker backend done"));

        let _ = fs::remove_file(session_file_path(&session_id));
        let _ = fs::remove_dir_all(&root);
    }
}
