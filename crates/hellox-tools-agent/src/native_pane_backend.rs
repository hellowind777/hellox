use std::env;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

pub use crate::native_pane_backend_preflight::{
    detect_native_pane_backend, NativePaneBackend, ITERM_BACKEND, ITERM_COMMAND_ENV, TMUX_BACKEND,
    TMUX_COMMAND_ENV,
};
use crate::native_pane_layout::{
    build_iterm_script, build_tmux_new_session_args, build_tmux_select_layout_args,
    build_tmux_split_args, escape_applescript_string, pane_group_name, pane_group_title,
    pane_title, shell_join, split_direction, tmux_layout_preset,
};
use crate::pane_host_harness::{run_pane_host_command, PaneHostCommandOutput};
use crate::shared::normalize_path;

pub fn launch_native_pane(
    backend: NativePaneBackend,
    session_id: &str,
    job_path: &Path,
    agent_name: Option<&str>,
    pane_group: Option<&str>,
    layout_strategy: Option<&str>,
    layout_slot: Option<&str>,
    anchor_target: Option<&str>,
) -> Result<String> {
    let worker_command = worker_shell_command(job_path)?;
    match backend {
        NativePaneBackend::Tmux => launch_tmux_pane(
            &worker_command,
            session_id,
            agent_name,
            pane_group,
            layout_strategy,
            layout_slot,
            anchor_target,
        ),
        NativePaneBackend::ITerm => launch_iterm_pane(
            &worker_command,
            session_id,
            agent_name,
            pane_group,
            layout_strategy,
            layout_slot,
            anchor_target,
        ),
    }
}

pub fn terminate_native_pane(backend: &str, pane_target: &str) -> Result<()> {
    match backend {
        TMUX_BACKEND => terminate_tmux_pane(pane_target),
        ITERM_BACKEND => terminate_iterm_pane(pane_target),
        other => Err(anyhow!(
            "backend `{other}` does not support pane termination"
        )),
    }
}

fn launch_tmux_pane(
    worker_command: &str,
    session_id: &str,
    agent_name: Option<&str>,
    pane_group: Option<&str>,
    layout_strategy: Option<&str>,
    layout_slot: Option<&str>,
    anchor_target: Option<&str>,
) -> Result<String> {
    let prefix = command_prefix_from_env(TMUX_COMMAND_ENV, &["tmux"])?;
    let title = pane_title(session_id, agent_name);
    let group = pane_group_name(session_id, pane_group);
    let split_direction = split_direction(layout_slot);
    let split_target = anchor_target
        .map(ToString::to_string)
        .or_else(|| pane_group.is_some().then(|| format!("{group}:0")));
    let launch_args = match (split_target.as_deref(), split_direction) {
        (Some(target), Some(split_direction)) => {
            build_tmux_split_args(target, worker_command, Some(split_direction))
        }
        _ => build_tmux_new_session_args(&group, &title, worker_command),
    };
    let pane_target = match run_capture_command(&prefix, &launch_args, "tmux pane launch") {
        Ok(target) => Ok(target),
        Err(primary_error) => match fallback_tmux_launch(
            &prefix,
            &group,
            &title,
            worker_command,
            split_direction,
            anchor_target,
        ) {
            Ok(target) => Ok(target),
            Err(fallback_error) => Err(anyhow!(
                "tmux pane launch failed: {primary_error}; fallback launch also failed: {fallback_error}"
            )),
        },
    }?;
    apply_tmux_layout(&prefix, &group, layout_strategy);
    Ok(pane_target)
}

fn terminate_tmux_pane(pane_target: &str) -> Result<()> {
    let prefix = command_prefix_from_env(TMUX_COMMAND_ENV, &["tmux"])?;
    run_status_command(
        &prefix,
        &[
            "kill-pane".to_string(),
            "-t".to_string(),
            pane_target.to_string(),
        ],
        "tmux pane stop",
    )
}

fn launch_iterm_pane(
    worker_command: &str,
    session_id: &str,
    agent_name: Option<&str>,
    pane_group: Option<&str>,
    _layout_strategy: Option<&str>,
    layout_slot: Option<&str>,
    anchor_target: Option<&str>,
) -> Result<String> {
    let prefix = command_prefix_from_env(ITERM_COMMAND_ENV, &["osascript"])?;
    let title = pane_title(session_id, agent_name);
    let group = pane_group_title(session_id, pane_group);
    let script = build_iterm_script(worker_command, &title, &group, layout_slot, anchor_target);
    run_capture_command(&prefix, &[String::from("-e"), script], "iTerm pane launch")
}

fn apply_tmux_layout(prefix: &[String], group: &str, layout_strategy: Option<&str>) {
    let Some(preset) = tmux_layout_preset(layout_strategy) else {
        return;
    };
    let _ = run_status_command(
        prefix,
        &build_tmux_select_layout_args(&format!("{group}:0"), preset),
        "tmux pane layout",
    );
}

pub fn sync_tmux_layout_for_group(
    group: &str,
    layout_strategy: Option<&str>,
) -> Result<Option<String>> {
    let Some(preset) = tmux_layout_preset(layout_strategy) else {
        return Ok(None);
    };
    let prefix = command_prefix_from_env(TMUX_COMMAND_ENV, &["tmux"])?;
    run_status_command(
        &prefix,
        &build_tmux_select_layout_args(&format!("{group}:0"), preset),
        "tmux pane layout",
    )?;
    Ok(Some(preset.to_string()))
}

fn fallback_tmux_launch(
    prefix: &[String],
    group: &str,
    title: &str,
    worker_command: &str,
    split_direction: Option<&'static str>,
    anchor_target: Option<&str>,
) -> Result<String> {
    let live_panes = list_tmux_panes(prefix, group)?;
    if live_panes.is_empty() {
        return run_capture_command(
            prefix,
            &build_tmux_new_session_args(group, title, worker_command),
            "tmux pane fallback launch",
        );
    }

    let target = anchor_target
        .filter(|candidate| live_panes.iter().any(|pane| pane == candidate))
        .map(ToString::to_string)
        .unwrap_or_else(|| live_panes[0].clone());

    run_capture_command(
        prefix,
        &build_tmux_split_args(&target, worker_command, split_direction),
        "tmux pane fallback launch",
    )
}

pub fn list_tmux_panes(prefix: &[String], group: &str) -> Result<Vec<String>> {
    let output = run_prefixed_command_output(
        prefix,
        &[
            "list-panes".to_string(),
            "-t".to_string(),
            group.to_string(),
            "-F".to_string(),
            "#{pane_id}".to_string(),
        ],
        "tmux pane inspection",
    )
    .context("failed to invoke tmux pane inspection")?;

    if !output.success {
        return Ok(Vec::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
}

fn terminate_iterm_pane(pane_target: &str) -> Result<()> {
    let prefix = command_prefix_from_env(ITERM_COMMAND_ENV, &["osascript"])?;
    let script = format!(
        "tell application \"iTerm\"\n\
repeat with currentWindow in windows\n\
repeat with currentTab in tabs of currentWindow\n\
repeat with candidateSession in sessions of currentTab\n\
if (id of candidateSession as text) is \"{}\" then\n\
tell candidateSession to close\n\
return \"closed\"\n\
end if\n\
end repeat\n\
end repeat\n\
end repeat\n\
return \"missing\"\n\
end tell",
        escape_applescript_string(pane_target)
    );
    run_status_command(&prefix, &[String::from("-e"), script], "iTerm pane stop")
}

fn worker_shell_command(job_path: &Path) -> Result<String> {
    let prefix = match env::var("HELLOX_AGENT_BACKEND_COMMAND") {
        Ok(raw) if !raw.trim().is_empty() => {
            command_prefix_from_env("HELLOX_AGENT_BACKEND_COMMAND", &[])?
        }
        _ => vec![
            env::current_exe()
                .context("failed to locate current executable for pane backend")?
                .display()
                .to_string(),
            "worker-run-agent".to_string(),
        ],
    };
    let mut command = prefix;
    command.push("--job".to_string());
    command.push(normalize_path(job_path));
    Ok(shell_join(&command))
}

fn run_capture_command(prefix: &[String], args: &[String], context: &str) -> Result<String> {
    let output = run_prefixed_command_output(prefix, args, context)?;
    if !output.success {
        return Err(anyhow!(
            "{context} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err(anyhow!("{context} did not return a pane target"));
    }
    Ok(stdout)
}

fn run_status_command(prefix: &[String], args: &[String], context: &str) -> Result<()> {
    let output = run_prefixed_command_output(prefix, args, context)?;
    if output.success {
        Ok(())
    } else {
        Err(anyhow!("{context} failed"))
    }
}

fn run_prefixed_command_output(
    prefix: &[String],
    args: &[String],
    context: &str,
) -> Result<PaneHostCommandOutput> {
    let (program, program_args) = prefix
        .split_first()
        .ok_or_else(|| anyhow!("{context} launcher command is empty"))?;
    let mut argv = program_args.to_vec();
    argv.extend(args.iter().cloned());
    run_pane_host_command(program, &argv, context)
}

pub fn command_prefix_from_env(env_name: &str, default: &[&str]) -> Result<Vec<String>> {
    match env::var(env_name) {
        Ok(raw) if !raw.trim().is_empty() => {
            let items = serde_json::from_str::<Vec<String>>(&raw)
                .with_context(|| format!("failed to parse {env_name} as JSON array"))?;
            if items.is_empty() || items.iter().any(|item| item.trim().is_empty()) {
                return Err(anyhow!(
                    "{env_name} must be a non-empty JSON array of non-empty strings"
                ));
            }
            Ok(items)
        }
        _ => Ok(default.iter().map(|item| item.to_string()).collect()),
    }
}
