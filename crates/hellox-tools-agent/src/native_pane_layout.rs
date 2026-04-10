pub fn build_tmux_new_session_args(group: &str, title: &str, worker_command: &str) -> Vec<String> {
    vec![
        "new-session".to_string(),
        "-d".to_string(),
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
        "-s".to_string(),
        group.to_string(),
        "-n".to_string(),
        title.to_string(),
        worker_command.to_string(),
    ]
}

pub fn build_tmux_split_args(
    target: &str,
    worker_command: &str,
    split_direction: Option<&'static str>,
) -> Vec<String> {
    let mut args = vec![
        "split-window".to_string(),
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
        "-t".to_string(),
        target.to_string(),
    ];
    if let Some(direction) = split_direction {
        args.push(direction.to_string());
    }
    args.extend([
        "-c".to_string(),
        ".".to_string(),
        worker_command.to_string(),
    ]);
    args
}

pub fn build_tmux_select_layout_args(target: &str, preset: &str) -> Vec<String> {
    vec![
        "select-layout".to_string(),
        "-t".to_string(),
        target.to_string(),
        preset.to_string(),
    ]
}

pub fn build_iterm_script(
    worker_command: &str,
    title: &str,
    group: &str,
    layout_slot: Option<&str>,
    anchor_target: Option<&str>,
) -> String {
    let escaped_command = escape_applescript_string(worker_command);
    let escaped_title = escape_applescript_string(title);
    let escaped_group = escape_applescript_string(group);
    let split = match split_direction(layout_slot) {
        Some("-h") => "horizontal",
        Some("-v") => "vertical",
        _ => "vertical",
    };

    let anchor_lookup = anchor_target
        .map(|anchor_target| {
            format!(
                "if (id of candidateSession as text) is \"{}\" then\n\
set preferredSession to candidateSession\n\
end if",
                escape_applescript_string(anchor_target)
            )
        })
        .unwrap_or_default();
    format!(
        "tell application \"iTerm\"\n\
set preferredSession to missing value\n\
set groupSession to missing value\n\
repeat with currentWindow in windows\n\
repeat with currentTab in tabs of currentWindow\n\
repeat with candidateSession in sessions of currentTab\n\
if (custom title of candidateSession as text) is \"{escaped_group}\" and groupSession is missing value then\n\
set groupSession to candidateSession\n\
end if\n\
{anchor_lookup}\n\
end repeat\n\
end repeat\n\
end repeat\n\
if preferredSession is not missing value then\n\
set targetSession to preferredSession\n\
else\n\
set targetSession to groupSession\n\
end if\n\
if targetSession is missing value then\n\
set newWindow to (create window with default profile command \"{escaped_command}\")\n\
tell current session of newWindow\n\
set custom title to \"{escaped_group}\"\n\
set name to \"{escaped_title}\"\n\
return (id as text)\n\
end tell\n\
else\n\
tell targetSession\n\
set newSession to (split {split} with default profile command \"{escaped_command}\")\n\
end tell\n\
tell newSession\n\
set custom title to \"{escaped_group}\"\n\
set name to \"{escaped_title}\"\n\
return (id as text)\n\
end tell\n\
end if\n\
end tell"
    )
}

pub fn pane_title(session_id: &str, agent_name: Option<&str>) -> String {
    match agent_name.map(str::trim).filter(|value| !value.is_empty()) {
        Some(agent_name) => format!("hellox-{agent_name}-{session_id}"),
        None => format!("hellox-{session_id}"),
    }
}

pub fn pane_group_name(session_id: &str, pane_group: Option<&str>) -> String {
    sanitize_identifier(
        pane_group
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(session_id),
    )
}

pub fn pane_group_title(session_id: &str, pane_group: Option<&str>) -> String {
    pane_group
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("hellox-{session_id}"))
}

pub fn split_direction(layout_slot: Option<&str>) -> Option<&'static str> {
    let slot = layout_slot?;
    if slot.starts_with("right") {
        Some("-h")
    } else if slot.starts_with("bottom") {
        Some("-v")
    } else {
        None
    }
}

pub fn tmux_layout_preset(layout_strategy: Option<&str>) -> Option<&'static str> {
    match layout_strategy
        .map(|value| value.trim().to_ascii_lowercase().replace('-', "_"))
        .as_deref()
    {
        Some("fanout") => Some("main-vertical"),
        Some("horizontal") => Some("even-horizontal"),
        Some("vertical") => Some("even-vertical"),
        Some("grid") => Some("tiled"),
        _ => None,
    }
}

pub fn shell_join(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_identifier(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('-').to_string()
}

fn shell_quote(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "'\"'\"'"))
}

pub fn escape_applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_tmux_session_for_primary_slot() {
        let args = build_tmux_new_session_args("team-alpha", "hellox-alice", "'hx' '--job'");
        assert!(args.iter().any(|item| item == "new-session"));
        assert!(args.iter().any(|item| item == "team-alpha"));
    }

    #[test]
    fn build_tmux_split_for_secondary_slot() {
        let args = build_tmux_split_args("%2", "'hx' '--job'", Some("-h"));
        assert!(args.iter().any(|item| item == "split-window"));
        assert!(args.iter().any(|item| item == "-h"));
        assert!(args.iter().any(|item| item == "%2"));
    }

    #[test]
    fn build_iterm_script_uses_anchor_target_and_split() {
        let script = build_iterm_script(
            "hellox worker",
            "hellox-bob",
            "team-alpha",
            Some("right"),
            Some("session-2"),
        );
        assert!(script.contains("session-2"));
        assert!(script.contains("custom title of candidateSession as text"));
        assert!(script.contains("split horizontal"));
        assert!(script.contains("hellox worker"));
    }

    #[test]
    fn build_iterm_script_reuses_existing_group_for_primary_slot() {
        let script = build_iterm_script(
            "hellox worker",
            "hellox-alice",
            "team-alpha",
            Some("primary"),
            None,
        );
        assert!(script.contains("groupSession"));
        assert!(script.contains("split vertical"));
        assert!(script.contains("create window with default profile command"));
    }

    #[test]
    fn shell_join_quotes_arguments() {
        let command = shell_join(&[
            "/tmp/hello x".to_string(),
            "worker-run-agent".to_string(),
            "--job".to_string(),
            "/tmp/a'b.json".to_string(),
        ]);
        assert!(command.contains("'/tmp/hello x'"));
        assert!(command.contains("'worker-run-agent'"));
        assert!(command.contains("\"'\""));
    }

    #[test]
    fn resolves_tmux_layout_presets() {
        assert_eq!(tmux_layout_preset(Some("fanout")), Some("main-vertical"));
        assert_eq!(
            tmux_layout_preset(Some("horizontal")),
            Some("even-horizontal")
        );
        assert_eq!(tmux_layout_preset(Some("vertical")), Some("even-vertical"));
        assert_eq!(tmux_layout_preset(Some("grid")), Some("tiled"));
        assert_eq!(tmux_layout_preset(None), None);
    }
}
