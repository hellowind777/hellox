use anyhow::{anyhow, Context, Result};

use crate::native_pane_backend::{command_prefix_from_env, list_tmux_panes};
use crate::native_pane_backend_preflight::{
    ITERM_BACKEND, ITERM_COMMAND_ENV, TMUX_BACKEND, TMUX_COMMAND_ENV,
};
use crate::native_pane_layout::escape_applescript_string;
use crate::pane_host_harness::run_pane_host_command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneGroupHostState {
    pub backend: &'static str,
    pub live_targets: Vec<String>,
    pub inspect_error: Option<String>,
}

pub fn inspect_tmux_group(pane_group: Option<&str>) -> Option<PaneGroupHostState> {
    let pane_group = pane_group
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    Some(match command_prefix_from_env(TMUX_COMMAND_ENV, &["tmux"]) {
        Ok(prefix) => match list_tmux_panes(&prefix, pane_group) {
            Ok(live_targets) => PaneGroupHostState {
                backend: TMUX_BACKEND,
                live_targets,
                inspect_error: None,
            },
            Err(error) => PaneGroupHostState {
                backend: TMUX_BACKEND,
                live_targets: Vec::new(),
                inspect_error: Some(error.to_string()),
            },
        },
        Err(error) => PaneGroupHostState {
            backend: TMUX_BACKEND,
            live_targets: Vec::new(),
            inspect_error: Some(error.to_string()),
        },
    })
}

pub fn inspect_iterm_group(pane_group: Option<&str>) -> Option<PaneGroupHostState> {
    let pane_group = pane_group
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    Some(
        match command_prefix_from_env(ITERM_COMMAND_ENV, &["osascript"]) {
            Ok(prefix) => {
                let script = format!(
                    "tell application \"iTerm\"\n\
set sessionIds to {{}}\n\
repeat with currentWindow in windows\n\
repeat with currentTab in tabs of currentWindow\n\
repeat with candidateSession in sessions of currentTab\n\
if (custom title of candidateSession as text) is \"{}\" then\n\
set end of sessionIds to (id of candidateSession as text)\n\
end if\n\
end repeat\n\
end repeat\n\
end repeat\n\
set AppleScript's text item delimiters to linefeed\n\
return sessionIds as text\n\
end tell",
                    escape_applescript_string(pane_group)
                );

                match run_text_command(
                    &prefix,
                    &[String::from("-e"), script],
                    "iTerm pane inspection",
                ) {
                    Ok(stdout) => PaneGroupHostState {
                        backend: ITERM_BACKEND,
                        live_targets: stdout
                            .lines()
                            .map(str::trim)
                            .filter(|line| !line.is_empty())
                            .map(ToString::to_string)
                            .collect(),
                        inspect_error: None,
                    },
                    Err(error) => PaneGroupHostState {
                        backend: ITERM_BACKEND,
                        live_targets: Vec::new(),
                        inspect_error: Some(error.to_string()),
                    },
                }
            }
            Err(error) => PaneGroupHostState {
                backend: ITERM_BACKEND,
                live_targets: Vec::new(),
                inspect_error: Some(error.to_string()),
            },
        },
    )
}

fn run_text_command(prefix: &[String], args: &[String], context: &str) -> Result<String> {
    let (program, program_args) = prefix
        .split_first()
        .ok_or_else(|| anyhow!("{context} launcher command is empty"))?;
    let mut argv = program_args.to_vec();
    argv.extend(args.iter().cloned());
    let output = run_pane_host_command(program, &argv, context)
        .with_context(|| format!("failed to invoke {context}"))?;
    if !output.success {
        return Err(anyhow!(
            "{context} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
