use std::env;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::{detached_job_path, AgentSession, DetachedAgentJob};

use super::background::{running_record, store_background_record, AgentJobRecord};
use super::native_pane_backend::{
    detect_native_pane_backend, launch_native_pane, terminate_native_pane, NativePaneBackend,
    ITERM_BACKEND, TMUX_BACKEND,
};
use super::shared::normalize_path;

const DETACHED_PROCESS_BACKEND: &str = "detached_process";
const IN_PROCESS_BACKEND: &str = "in_process";
const PANE_BACKEND_ALIAS: &str = "pane";
const AUTO_BACKEND_ALIAS: &str = "auto";
const TMUX_BACKEND_ALIAS: &str = "tmux";
const ITERM_BACKEND_ALIAS: &str = "iterm";
const BACKEND_COMMAND_ENV: &str = "HELLOX_AGENT_BACKEND_COMMAND";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentBackend {
    InProcess,
    DetachedProcess,
    TmuxPane,
    ITermPane,
}

pub(super) fn parse_backend(input: &Value, key: &str) -> Result<Option<String>> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .map(Some)
        .map_or(Ok(None), |value| {
            validate_backend_name(value.as_deref()).map(|_| value)
        })
}

pub(super) fn resolve_backend(
    requested: Option<&str>,
    run_in_background: bool,
) -> Result<AgentBackend> {
    let backend = match requested
        .map(|value| value.trim().to_ascii_lowercase().replace('-', "_"))
        .as_deref()
    {
        None | Some(AUTO_BACKEND_ALIAS) | Some(IN_PROCESS_BACKEND) => AgentBackend::InProcess,
        Some(DETACHED_PROCESS_BACKEND) => AgentBackend::DetachedProcess,
        Some(PANE_BACKEND_ALIAS) => match detect_native_pane_backend() {
            Some(NativePaneBackend::Tmux) => AgentBackend::TmuxPane,
            Some(NativePaneBackend::ITerm) => AgentBackend::ITermPane,
            None => AgentBackend::DetachedProcess,
        },
        Some(TMUX_BACKEND_ALIAS) | Some(TMUX_BACKEND) => AgentBackend::TmuxPane,
        Some(ITERM_BACKEND_ALIAS) | Some(ITERM_BACKEND) => AgentBackend::ITermPane,
        Some(other) => {
            return Err(anyhow!(
                "unsupported agent backend `{other}`; use one of: in_process, detached_process, pane, tmux, iterm"
            ));
        }
    };

    if !run_in_background
        && matches!(
            backend,
            AgentBackend::DetachedProcess | AgentBackend::TmuxPane | AgentBackend::ITermPane
        )
    {
        return Err(anyhow!(
            "agent backend requires `run_in_background: true` for out-of-process execution"
        ));
    }

    Ok(backend)
}

pub(super) struct ProcessLaunchOptions<'a> {
    pub(super) prompt: String,
    pub(super) max_turns: usize,
    pub(super) resumed: bool,
    pub(super) config_path: &'a Path,
    pub(super) agent_name: Option<&'a str>,
    pub(super) pane_group: Option<&'a str>,
    pub(super) layout_strategy: Option<&'a str>,
    pub(super) layout_slot: Option<&'a str>,
    pub(super) pane_anchor_target: Option<&'a str>,
}

pub(super) fn launch_process_backend_agent(
    backend: AgentBackend,
    session: &AgentSession,
    session_id: &str,
    options: ProcessLaunchOptions<'_>,
) -> Result<AgentJobRecord> {
    let job_path = detached_job_path(session_id);
    DetachedAgentJob {
        session_id: session_id.to_string(),
        session_path: normalize_path(&hellox_config::session_file_path(session_id)),
        prompt: options.prompt,
        max_turns: options.max_turns,
        config_path: Some(normalize_path(options.config_path)),
    }
    .save(&job_path)?;

    let running = match backend {
        AgentBackend::DetachedProcess => launch_detached_process(
            session,
            session_id,
            options.resumed,
            options.layout_slot,
            &job_path,
        )?,
        AgentBackend::TmuxPane => launch_native_pane_backend(
            session,
            session_id,
            options.resumed,
            options.agent_name,
            options.pane_group,
            options.layout_strategy,
            options.layout_slot,
            options.pane_anchor_target,
            &job_path,
            NativePaneBackend::Tmux,
        )?,
        AgentBackend::ITermPane => launch_native_pane_backend(
            session,
            session_id,
            options.resumed,
            options.agent_name,
            options.pane_group,
            options.layout_strategy,
            options.layout_slot,
            options.pane_anchor_target,
            &job_path,
            NativePaneBackend::ITerm,
        )?,
        AgentBackend::InProcess => {
            return Err(anyhow!(
                "process backend launcher cannot handle in_process sessions"
            ));
        }
    };
    store_background_record(running.clone())?;
    Ok(running)
}

pub(super) fn terminate_backend_process(
    backend: &str,
    pid: Option<u32>,
    pane_target: Option<&str>,
) -> Result<()> {
    match backend {
        DETACHED_PROCESS_BACKEND => terminate_detached_process(
            pid.ok_or_else(|| anyhow!("detached process backend is missing a pid"))?,
        ),
        TMUX_BACKEND | ITERM_BACKEND => terminate_native_pane(
            backend,
            pane_target.ok_or_else(|| anyhow!("pane backend is missing a pane target"))?,
        ),
        other => Err(anyhow!(
            "backend `{other}` does not support out-of-process termination"
        )),
    }
}

fn launch_detached_process(
    session: &AgentSession,
    session_id: &str,
    resumed: bool,
    layout_slot: Option<&str>,
    job_path: &Path,
) -> Result<AgentJobRecord> {
    let prefix = launcher_command_prefix()?;
    let (program, prefix_args) = prefix
        .split_first()
        .ok_or_else(|| anyhow!("detached backend launcher command is empty"))?;
    let mut command = Command::new(program);
    command
        .args(prefix_args)
        .arg("--job")
        .arg(job_path)
        .current_dir(session.working_directory())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_spawn_flags(&mut command);

    let child = command.spawn().with_context(|| {
        format!(
            "failed to launch detached agent backend `{}`",
            session.working_directory().display()
        )
    })?;
    Ok(running_record(
        session,
        session_id,
        resumed,
        true,
        DETACHED_PROCESS_BACKEND,
        Some(child.id()),
        None,
        layout_slot.map(ToString::to_string),
    ))
}

fn launch_native_pane_backend(
    session: &AgentSession,
    session_id: &str,
    resumed: bool,
    agent_name: Option<&str>,
    pane_group: Option<&str>,
    layout_strategy: Option<&str>,
    layout_slot: Option<&str>,
    pane_anchor_target: Option<&str>,
    job_path: &Path,
    backend: NativePaneBackend,
) -> Result<AgentJobRecord> {
    let pane_target = launch_native_pane(
        backend,
        session_id,
        job_path,
        agent_name,
        pane_group,
        layout_strategy,
        layout_slot,
        pane_anchor_target,
    )?;
    Ok(running_record(
        session,
        session_id,
        resumed,
        true,
        backend.as_str(),
        None,
        Some(pane_target),
        layout_slot.map(ToString::to_string),
    ))
}

pub(super) fn terminate_detached_process(pid: u32) -> Result<()> {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to invoke taskkill for detached agent")?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "failed to stop detached agent process `{pid}` with taskkill"
            ))
        }
    }

    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to invoke kill for detached agent")?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "failed to stop detached agent process `{pid}` with kill"
            ))
        }
    }
}

fn validate_backend_name(value: Option<&str>) -> Result<()> {
    let _ = resolve_backend(value, true)?;
    Ok(())
}

fn launcher_command_prefix() -> Result<Vec<String>> {
    match env::var(BACKEND_COMMAND_ENV) {
        Ok(raw) if !raw.trim().is_empty() => parse_command_prefix(&raw),
        _ => Ok(vec![
            env::current_exe()
                .context("failed to locate current executable for detached agent backend")?
                .display()
                .to_string(),
            "worker-run-agent".to_string(),
        ]),
    }
}

fn parse_command_prefix(raw: &str) -> Result<Vec<String>> {
    let items = serde_json::from_str::<Vec<String>>(raw)
        .context("failed to parse HELLOX_AGENT_BACKEND_COMMAND as JSON array")?;
    if items.is_empty() || items.iter().any(|item| item.trim().is_empty()) {
        return Err(anyhow!(
            "HELLOX_AGENT_BACKEND_COMMAND must be a non-empty JSON array of non-empty strings"
        ));
    }
    Ok(items)
}

#[cfg(windows)]
fn apply_spawn_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_spawn_flags(_command: &mut Command) {}
