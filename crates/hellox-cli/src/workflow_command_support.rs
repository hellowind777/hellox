use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use hellox_agent::{
    default_tool_registry, AgentOptions, AgentSession, ConsoleApprovalHandler, GatewayClient,
};
use hellox_config::{default_config_path, load_or_default};

use crate::cli_workflow_types::WorkflowCommands;
use crate::workflows::WorkflowRunTarget;

pub(crate) enum WorkflowLookupTarget {
    Named(String),
    Path(PathBuf),
}

pub(crate) fn workflow_command_cwd(command: &WorkflowCommands) -> Option<&PathBuf> {
    match command {
        WorkflowCommands::List { cwd }
        | WorkflowCommands::Dashboard { cwd, .. }
        | WorkflowCommands::Overview { cwd, .. }
        | WorkflowCommands::Panel { cwd, .. }
        | WorkflowCommands::Runs { cwd, .. }
        | WorkflowCommands::Validate { cwd, .. }
        | WorkflowCommands::ShowRun { cwd, .. }
        | WorkflowCommands::LastRun { cwd, .. }
        | WorkflowCommands::Show { cwd, .. }
        | WorkflowCommands::Init { cwd, .. }
        | WorkflowCommands::AddStep { cwd, .. }
        | WorkflowCommands::UpdateStep { cwd, .. }
        | WorkflowCommands::DuplicateStep { cwd, .. }
        | WorkflowCommands::MoveStep { cwd, .. }
        | WorkflowCommands::RemoveStep { cwd, .. }
        | WorkflowCommands::SetSharedContext { cwd, .. }
        | WorkflowCommands::ClearSharedContext { cwd, .. }
        | WorkflowCommands::EnableContinueOnError { cwd, .. }
        | WorkflowCommands::DisableContinueOnError { cwd, .. }
        | WorkflowCommands::Run { cwd, .. } => cwd.as_ref(),
    }
}

pub(crate) fn resolve_lookup_target(
    workflow_name: Option<String>,
    script_path: Option<PathBuf>,
    label: &str,
) -> Result<WorkflowLookupTarget> {
    resolve_optional_lookup_target(workflow_name, script_path, label)?
        .ok_or_else(|| anyhow!("{label} requires a workflow name or `--script-path`"))
}

pub(crate) fn resolve_optional_lookup_target(
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

pub(crate) fn resolve_lookup_run_target(
    root: &Path,
    workflow_name: Option<String>,
    script_path: Option<PathBuf>,
    label: &str,
) -> Result<WorkflowRunTarget> {
    resolve_optional_lookup_run_target(root, workflow_name, script_path, label)?
        .ok_or_else(|| anyhow!("{label} requires a workflow name or `--script-path`"))
}

pub(crate) fn resolve_optional_lookup_run_target(
    root: &Path,
    workflow_name: Option<String>,
    script_path: Option<PathBuf>,
    label: &str,
) -> Result<Option<WorkflowRunTarget>> {
    Ok(
        match resolve_optional_lookup_target(workflow_name, script_path, label)? {
            Some(WorkflowLookupTarget::Named(name)) => Some(WorkflowRunTarget::Named(name)),
            Some(WorkflowLookupTarget::Path(path)) => {
                Some(WorkflowRunTarget::Path(resolve_script_path(root, path)))
            }
            None => None,
        },
    )
}

pub(crate) fn resolve_lookup_path(
    root: &Path,
    workflow_name: Option<String>,
    script_path: Option<PathBuf>,
    label: &str,
    resolve_named: impl FnOnce(&str) -> Result<PathBuf>,
) -> Result<PathBuf> {
    match resolve_lookup_target(workflow_name, script_path, label)? {
        WorkflowLookupTarget::Named(name) => resolve_named(&name),
        WorkflowLookupTarget::Path(path) => Ok(resolve_script_path(root, path)),
    }
}

pub(crate) fn build_workflow_session(
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

pub(crate) fn preferred_workflow_config_path(root: &Path) -> Option<PathBuf> {
    let path = root.join(".hellox").join("config.toml");
    path.is_file().then_some(path)
}

pub(crate) fn workspace_root(value: Option<PathBuf>) -> Result<PathBuf> {
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

pub(crate) fn resolve_script_path(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub(crate) fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn merge_optional_field(value: Option<String>, clear: bool) -> Option<Option<String>> {
    if clear {
        Some(None)
    } else {
        value.map(Some)
    }
}

pub(crate) fn merge_background_flags(
    run_in_background: bool,
    foreground: bool,
) -> Result<Option<bool>> {
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

pub(crate) fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
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
