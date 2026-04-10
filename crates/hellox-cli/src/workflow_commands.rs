use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use hellox_agent::{
    default_tool_registry, AgentOptions, AgentSession, ConsoleApprovalHandler, GatewayClient,
};
use hellox_config::{default_config_path, load_or_default};

use crate::cli_workflow_types::WorkflowCommands;
use crate::workflow_authoring::{
    add_workflow_step, remove_workflow_step, resolve_existing_workflow_path,
    set_workflow_continue_on_error, set_workflow_shared_context, update_workflow_step,
    WorkflowStepDraft, WorkflowStepPatch,
};
use crate::workflow_overview::render_workflow_overview;
use crate::workflow_panel::{render_workflow_panel, render_workflow_panel_detail};
use crate::workflow_runs::{
    execute_and_record_workflow, list_workflow_runs, load_latest_workflow_run, load_workflow_run,
    render_workflow_run_inspect_panel, render_workflow_run_list,
};
use crate::workflows::{
    initialize_workflow, list_workflows, load_named_workflow_detail,
    load_workflow_detail_from_path, render_workflow_detail, render_workflow_list,
    render_workflow_validation, validate_explicit_workflow_path, validate_named_workflow,
    validate_workflows, WorkflowRunTarget,
};

pub(crate) async fn handle_workflow_command(command: WorkflowCommands) -> Result<()> {
    println!("{}", workflow_command_text(command).await?);
    Ok(())
}

pub(crate) async fn workflow_command_text(command: WorkflowCommands) -> Result<String> {
    match command {
        WorkflowCommands::List { cwd } => {
            let root = workspace_root(cwd)?;
            let workflows = list_workflows(&root)?;
            Ok(render_workflow_list(&root, &workflows))
        }
        WorkflowCommands::Overview { workflow_name, cwd } => {
            let root = workspace_root(cwd)?;
            render_workflow_overview(&root, workflow_name.as_deref())
        }
        WorkflowCommands::Panel {
            workflow_name,
            script_path,
            step,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            match resolve_optional_lookup_target(workflow_name, script_path, "workflow panel")? {
                Some(WorkflowLookupTarget::Named(name)) => {
                    render_workflow_panel(&root, Some(&name), step)
                }
                Some(WorkflowLookupTarget::Path(path)) => {
                    let detail = load_workflow_detail_from_path(
                        &root,
                        &resolve_script_path(&root, path),
                        None,
                    )?;
                    render_workflow_panel_detail(&root, &detail, step)
                }
                None => render_workflow_panel(&root, None, step),
            }
        }
        WorkflowCommands::Runs {
            workflow_name,
            limit,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let runs = list_workflow_runs(&root, workflow_name.as_deref(), limit)?;
            Ok(render_workflow_run_list(
                &root,
                &runs,
                workflow_name.as_deref(),
            ))
        }
        WorkflowCommands::Validate {
            workflow_name,
            script_path,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let results = match resolve_optional_lookup_target(
                workflow_name,
                script_path,
                "workflow validate",
            )? {
                Some(WorkflowLookupTarget::Named(name)) => {
                    vec![validate_named_workflow(&root, &name)?]
                }
                Some(WorkflowLookupTarget::Path(path)) => {
                    vec![validate_explicit_workflow_path(
                        &root,
                        &resolve_script_path(&root, path),
                    )?]
                }
                None => validate_workflows(&root)?,
            };
            Ok(render_workflow_validation(&results, &root))
        }
        WorkflowCommands::ShowRun { run_id, cwd } => {
            let root = workspace_root(cwd)?;
            Ok(render_workflow_run_inspect_panel(
                &root,
                &load_workflow_run(&root, &run_id)?,
            ))
        }
        WorkflowCommands::LastRun { workflow_name, cwd } => {
            let root = workspace_root(cwd)?;
            Ok(render_workflow_run_inspect_panel(
                &root,
                &load_latest_workflow_run(&root, workflow_name.as_deref())?,
            ))
        }
        WorkflowCommands::Show {
            workflow_name,
            script_path,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            match resolve_lookup_target(workflow_name, script_path, "workflow show")? {
                WorkflowLookupTarget::Named(name) => Ok(render_workflow_detail(
                    &load_named_workflow_detail(&root, &name)?,
                )),
                WorkflowLookupTarget::Path(path) => {
                    Ok(render_workflow_detail(&load_workflow_detail_from_path(
                        &root,
                        &resolve_script_path(&root, path),
                        None,
                    )?))
                }
            }
        }
        WorkflowCommands::Init {
            workflow_name,
            cwd,
            shared_context,
            continue_on_error,
            force,
        } => {
            let root = workspace_root(cwd)?;
            let path = initialize_workflow(
                &root,
                &workflow_name,
                shared_context,
                continue_on_error,
                force,
            )?;
            Ok(format!(
                "Initialized workflow `{}` at `{}`.",
                workflow_name,
                path_text(&path)
            ))
        }
        WorkflowCommands::AddStep {
            workflow_name,
            script_path,
            name,
            prompt,
            index,
            when,
            model,
            backend,
            step_cwd,
            run_in_background,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let path = resolve_lookup_path(
                root.as_path(),
                workflow_name,
                script_path,
                "workflow add-step",
            )?;
            let result = add_workflow_step(
                &root,
                &path,
                WorkflowStepDraft {
                    name,
                    prompt,
                    when,
                    model,
                    backend,
                    step_cwd,
                    run_in_background,
                },
                index,
            )?;
            Ok(format!(
                "Added workflow step {} at `{}`.\n{}",
                result.step_number,
                path_text(&result.detail.summary.path),
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommands::UpdateStep {
            workflow_name,
            step_number,
            script_path,
            name,
            clear_name,
            prompt,
            when,
            clear_when,
            model,
            clear_model,
            backend,
            clear_backend,
            step_cwd,
            clear_step_cwd,
            run_in_background,
            foreground,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let path = resolve_lookup_path(
                root.as_path(),
                workflow_name,
                script_path,
                "workflow update-step",
            )?;
            let patch = WorkflowStepPatch {
                name: merge_optional_field(name, clear_name),
                prompt,
                when: merge_optional_field(when, clear_when),
                model: merge_optional_field(model, clear_model),
                backend: merge_optional_field(backend, clear_backend),
                step_cwd: merge_optional_field(step_cwd, clear_step_cwd),
                run_in_background: merge_background_flags(run_in_background, foreground)?,
            };
            let detail = update_workflow_step(&root, &path, step_number, patch)?;
            Ok(format!(
                "Updated workflow step {} at `{}`.\n{}",
                step_number,
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::RemoveStep {
            workflow_name,
            step_number,
            script_path,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let path = resolve_lookup_path(
                root.as_path(),
                workflow_name,
                script_path,
                "workflow remove-step",
            )?;
            let result = remove_workflow_step(&root, &path, step_number)?;
            let removed_name = result.removed_step_name.as_deref().unwrap_or("(unnamed)");
            Ok(format!(
                "Removed workflow step {} (`{}`) from `{}`.\n{}",
                step_number,
                removed_name,
                path_text(&result.detail.summary.path),
                render_workflow_detail(&result.detail)
            ))
        }
        WorkflowCommands::SetSharedContext {
            workflow_name,
            value,
            script_path,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let path = resolve_lookup_path(
                root.as_path(),
                workflow_name,
                script_path,
                "workflow set-shared-context",
            )?;
            let detail = set_workflow_shared_context(&root, &path, value)?;
            Ok(format!(
                "Updated shared_context for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::ClearSharedContext {
            workflow_name,
            script_path,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let path = resolve_lookup_path(
                root.as_path(),
                workflow_name,
                script_path,
                "workflow clear-shared-context",
            )?;
            let detail = set_workflow_shared_context(&root, &path, None)?;
            Ok(format!(
                "Cleared shared_context for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::EnableContinueOnError {
            workflow_name,
            script_path,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let path = resolve_lookup_path(
                root.as_path(),
                workflow_name,
                script_path,
                "workflow enable-continue-on-error",
            )?;
            let detail = set_workflow_continue_on_error(&root, &path, true)?;
            Ok(format!(
                "Enabled continue_on_error for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::DisableContinueOnError {
            workflow_name,
            script_path,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let path = resolve_lookup_path(
                root.as_path(),
                workflow_name,
                script_path,
                "workflow disable-continue-on-error",
            )?;
            let detail = set_workflow_continue_on_error(&root, &path, false)?;
            Ok(format!(
                "Disabled continue_on_error for `{}`.\n{}",
                path_text(&detail.summary.path),
                render_workflow_detail(&detail)
            ))
        }
        WorkflowCommands::Run {
            workflow_name,
            script_path,
            shared_context,
            continue_on_error,
            config,
            cwd,
        } => {
            let root = workspace_root(cwd)?;
            let target = match resolve_lookup_target(workflow_name, script_path, "workflow run")? {
                WorkflowLookupTarget::Named(name) => WorkflowRunTarget::Named(name),
                WorkflowLookupTarget::Path(path) => {
                    WorkflowRunTarget::Path(resolve_script_path(&root, path))
                }
            };
            let session = build_workflow_session(config, root)?;
            execute_and_record_workflow(
                &session,
                target,
                shared_context,
                continue_on_error.then_some(true),
            )
            .await
        }
    }
}

enum WorkflowLookupTarget {
    Named(String),
    Path(PathBuf),
}

fn resolve_lookup_target(
    workflow_name: Option<String>,
    script_path: Option<PathBuf>,
    label: &str,
) -> Result<WorkflowLookupTarget> {
    resolve_optional_lookup_target(workflow_name, script_path, label)?
        .ok_or_else(|| anyhow!("{label} requires a workflow name or `--script-path`"))
}

fn resolve_optional_lookup_target(
    workflow_name: Option<String>,
    script_path: Option<PathBuf>,
    label: &str,
) -> Result<Option<WorkflowLookupTarget>> {
    match (normalize_optional_text(workflow_name), script_path) {
        (Some(name), None) => Ok(Some(WorkflowLookupTarget::Named(name))),
        (None, Some(path)) => Ok(Some(WorkflowLookupTarget::Path(path))),
        (Some(_), Some(_)) => Err(anyhow!(
            "{label} accepts either a workflow name or `--script-path`, but not both"
        )),
        (None, None) => Ok(None),
    }
}

fn resolve_lookup_path(
    root: &Path,
    workflow_name: Option<String>,
    script_path: Option<PathBuf>,
    label: &str,
) -> Result<PathBuf> {
    match resolve_lookup_target(workflow_name, script_path, label)? {
        WorkflowLookupTarget::Named(name) => resolve_existing_workflow_path(root, &name),
        WorkflowLookupTarget::Path(path) => Ok(resolve_script_path(root, path)),
    }
}

fn build_workflow_session(
    config: Option<PathBuf>,
    working_directory: PathBuf,
) -> Result<AgentSession> {
    let config_path = config.unwrap_or_else(default_config_path);
    let current = load_or_default(Some(config_path.clone()))?;
    let gateway = GatewayClient::from_config(&current, None);
    let shell_name = current_shell_name();
    let console_handler = Arc::new(ConsoleApprovalHandler);
    let approval_handler = Some(console_handler.clone() as _);
    let question_handler = Some(console_handler as _);
    let options = AgentOptions {
        output_style: hellox_style::resolve_configured_output_style(&current, &working_directory)?,
        persona: hellox_style::resolve_configured_persona(&current, &working_directory)?,
        prompt_fragments: hellox_style::resolve_configured_fragments(&current, &working_directory)?,
        model: current.session.model.clone(),
        max_turns: 1,
        ..AgentOptions::default()
    };

    Ok(AgentSession::create(
        gateway,
        default_tool_registry(),
        config_path,
        working_directory,
        &shell_name,
        options,
        current.permissions.mode.clone(),
        approval_handler,
        question_handler,
        false,
        None,
    ))
}

fn current_shell_name() -> String {
    env::var("SHELL")
        .ok()
        .or_else(|| env::var("COMSPEC").ok())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "powershell".to_string()
            } else {
                "sh".to_string()
            }
        })
}

fn workspace_root(value: Option<PathBuf>) -> Result<PathBuf> {
    let root = match value {
        Some(path) => path,
        None => env::current_dir()?,
    };
    if !root.is_dir() {
        return Err(anyhow!(
            "workflow working directory does not exist or is not a directory: {}",
            path_text(&root)
        ));
    }
    Ok(root)
}

fn resolve_script_path(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn merge_optional_field(value: Option<String>, clear: bool) -> Option<Option<String>> {
    if clear {
        Some(None)
    } else {
        value.map(Some)
    }
}

fn merge_background_flags(run_in_background: bool, foreground: bool) -> Result<Option<bool>> {
    if run_in_background && foreground {
        return Err(anyhow!(
            "choose either `--run-in-background` or `--foreground`, but not both"
        ));
    }
    if run_in_background {
        Ok(Some(true))
    } else if foreground {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::{workflow_command_text, WorkflowCommands};

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-cli-workflow-command-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn write_workflow(root: &Path, relative: &str, raw: &str) {
        let path = root.join(".hellox").join("workflows").join(relative);
        fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
        fs::write(path, raw).expect("write workflow");
    }

    fn write_config(root: &Path, base_url: &str) -> PathBuf {
        let config = format!(
            "[gateway]\nlisten = \"{}\"\n\n[session]\npersist = false\nmodel = \"mock-model\"\n",
            base_url
        );
        let path = root.join(".hellox").join("config.toml");
        fs::create_dir_all(path.parent().expect("config dir")).expect("create config dir");
        fs::write(&path, config).expect("write config");
        path
    }

    async fn spawn_mock_gateway(response_text: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
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
                    "id": "workflow-command-response",
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
    async fn workflow_run_command_executes_named_script() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review {{workflow.shared_context}}" }
  ]
}"#,
        );
        let base_url = spawn_mock_gateway("workflow command done").await;
        let config_path = write_config(&root, &base_url);

        let text = workflow_command_text(WorkflowCommands::Run {
            workflow_name: Some("release-review".to_string()),
            script_path: None,
            shared_context: Some("ship carefully".to_string()),
            continue_on_error: false,
            config: Some(config_path),
            cwd: Some(root.clone()),
        })
        .await
        .expect("run workflow command");

        let run_id = serde_json::from_str::<serde_json::Value>(&text)
            .expect("parse workflow run output")
            .get("run_id")
            .and_then(serde_json::Value::as_str)
            .expect("workflow run id")
            .to_string();

        assert!(text.contains("\"workflow_source\": \".hellox/workflows/release-review.json\""));
        assert!(text.contains("workflow command done"));
        assert!(root
            .join(".hellox")
            .join("workflow-runs")
            .join(format!("{run_id}.json"))
            .exists());

        let runs = workflow_command_text(WorkflowCommands::Runs {
            workflow_name: Some("release-review".to_string()),
            limit: 10,
            cwd: Some(root.clone()),
        })
        .await
        .expect("list workflow runs");
        assert!(runs.contains(&run_id));

        let show_run = workflow_command_text(WorkflowCommands::ShowRun {
            run_id: run_id.clone(),
            cwd: Some(root.clone()),
        })
        .await
        .expect("show workflow run");
        assert!(show_run.contains("Workflow run inspect panel:"));
        assert!(show_run.contains("release-review"));
        assert!(show_run.contains("== Visual execution map =="));
        assert!(show_run.contains("== CLI palette =="));

        let last_run = workflow_command_text(WorkflowCommands::LastRun {
            workflow_name: Some("release-review".to_string()),
            cwd: Some(root.clone()),
        })
        .await
        .expect("show latest workflow run");
        assert!(last_run.contains(&run_id));
        assert!(last_run.contains("== REPL palette =="));
    }

    #[tokio::test]
    async fn workflow_show_command_renders_script_detail() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "continue_on_error": true,
  "steps": [
    { "name": "review", "prompt": "review release notes", "backend": "detached_process" }
  ]
}"#,
        );

        let text = workflow_command_text(WorkflowCommands::Show {
            workflow_name: Some("release-review".to_string()),
            script_path: None,
            cwd: Some(root.clone()),
        })
        .await
        .expect("show workflow command");

        assert!(text.contains("workflow: release-review"));
        assert!(text.contains("continue_on_error: true"));
        assert!(text.contains("backend=detached_process"));
    }

    #[tokio::test]
    async fn workflow_panel_command_renders_authoring_surface() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" },
    { "name": "summarize", "prompt": "summarize findings", "backend": "detached_process", "run_in_background": true }
  ]
}"#,
        );

        let text = workflow_command_text(WorkflowCommands::Panel {
            workflow_name: Some("release-review".to_string()),
            script_path: None,
            step: Some(2),
            cwd: Some(root.clone()),
        })
        .await
        .expect("render workflow panel command");

        assert!(text.contains("Workflow authoring panel: release-review"));
        assert!(text.contains("> | 2 | summarize"));
        assert!(text.contains("== Step selector =="));
        assert!(text.contains("== Action palette =="));
        assert!(text.contains("hellox workflow update-step --workflow release-review 2"));
    }

    #[tokio::test]
    async fn workflow_validate_and_init_commands_work() {
        let root = temp_dir();
        write_workflow(&root, "broken.json", "{ not-json");

        let validation = workflow_command_text(WorkflowCommands::Validate {
            workflow_name: None,
            script_path: None,
            cwd: Some(root.clone()),
        })
        .await
        .expect("validate workflow command");
        assert!(validation.contains("broken"));
        assert!(validation.contains("invalid"));

        let initialized = workflow_command_text(WorkflowCommands::Init {
            workflow_name: "release-review".to_string(),
            cwd: Some(root.clone()),
            shared_context: Some("ship carefully".to_string()),
            continue_on_error: true,
            force: false,
        })
        .await
        .expect("init workflow command");
        assert!(initialized.contains("Initialized workflow `release-review`"));
        assert!(root
            .join(".hellox")
            .join("workflows")
            .join("release-review.json")
            .exists());
    }

    #[tokio::test]
    async fn workflow_authoring_commands_edit_local_script() {
        let root = temp_dir();
        write_workflow(
            &root,
            "release-review.json",
            r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" }
  ]
}"#,
        );

        let added = workflow_command_text(WorkflowCommands::AddStep {
            workflow_name: Some("release-review".to_string()),
            script_path: None,
            name: Some("summarize".to_string()),
            prompt: "summarize findings".to_string(),
            index: Some(2),
            when: Some(r#"{"previous_status":"completed"}"#.to_string()),
            model: Some("mock-model".to_string()),
            backend: None,
            step_cwd: Some("docs".to_string()),
            run_in_background: true,
            cwd: Some(root.clone()),
        })
        .await
        .expect("add workflow step");
        assert!(added.contains("Added workflow step 2"));
        assert!(added.contains("summarize"));
        assert!(added.contains("background=true"));

        let updated = workflow_command_text(WorkflowCommands::UpdateStep {
            workflow_name: Some("release-review".to_string()),
            step_number: 2,
            script_path: None,
            name: None,
            clear_name: true,
            prompt: Some("ship release".to_string()),
            when: None,
            clear_when: true,
            model: None,
            clear_model: true,
            backend: Some("detached_process".to_string()),
            clear_backend: false,
            step_cwd: None,
            clear_step_cwd: true,
            run_in_background: false,
            foreground: true,
            cwd: Some(root.clone()),
        })
        .await
        .expect("update workflow step");
        assert!(updated.contains("Updated workflow step 2"));
        assert!(updated.contains("backend=detached_process"));
        assert!(!updated.contains("background=true"));

        let shared_context = workflow_command_text(WorkflowCommands::SetSharedContext {
            workflow_name: Some("release-review".to_string()),
            value: Some("ship carefully".to_string()),
            script_path: None,
            cwd: Some(root.clone()),
        })
        .await
        .expect("set shared context");
        assert!(shared_context.contains("shared_context: ship carefully"));

        let continue_on_error = workflow_command_text(WorkflowCommands::EnableContinueOnError {
            workflow_name: Some("release-review".to_string()),
            script_path: None,
            cwd: Some(root.clone()),
        })
        .await
        .expect("enable continue_on_error");
        assert!(continue_on_error.contains("continue_on_error: true"));

        let removed = workflow_command_text(WorkflowCommands::RemoveStep {
            workflow_name: Some("release-review".to_string()),
            step_number: 1,
            script_path: None,
            cwd: Some(root.clone()),
        })
        .await
        .expect("remove workflow step");
        assert!(removed.contains("Removed workflow step 1"));
        assert!(removed.contains("steps: 1"));

        let cleared_context = workflow_command_text(WorkflowCommands::ClearSharedContext {
            workflow_name: Some("release-review".to_string()),
            script_path: None,
            cwd: Some(root.clone()),
        })
        .await
        .expect("clear shared context");
        assert!(cleared_context.contains("shared_context: (none)"));

        let disabled_continue = workflow_command_text(WorkflowCommands::DisableContinueOnError {
            workflow_name: Some("release-review".to_string()),
            script_path: None,
            cwd: Some(root.clone()),
        })
        .await
        .expect("disable continue_on_error");
        assert!(disabled_continue.contains("continue_on_error: false"));
    }
}
