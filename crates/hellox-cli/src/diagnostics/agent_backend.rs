use std::env;
use std::path::Path;

use hellox_agent::{pane_backend_preflight, PaneCommandPrefixStatus};

use crate::startup::AppLanguage;

use super::normalize_path;

pub(super) fn agent_backend_doctor_lines(language: AppLanguage) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(format!(
        "{} agent_backend.backends: in_process, detached_process, pane, tmux_pane, iterm_pane",
        ok_tag(language)
    ));

    match env::var("HELLOX_AGENT_BACKEND_COMMAND") {
        Ok(raw) if !raw.trim().is_empty() => match parse_json_array_env(&raw) {
            Ok(items) => {
                lines.push(format!(
                    "{} agent_backend.command: env HELLOX_AGENT_BACKEND_COMMAND = {}",
                    ok_tag(language),
                    render_command_prefix(&items)
                ));
                lines.push(command_prefix_resolution_line(
                    language,
                    "agent_backend.command.program",
                    items.first().map(String::as_str),
                ));
            }
            Err(error) => lines.push(format!(
                "{} env HELLOX_AGENT_BACKEND_COMMAND {}: {error}",
                warn_tag(language),
                invalid_text(language)
            )),
        },
        _ => match env::current_exe() {
            Ok(path) => {
                let executable = normalize_path(&path);
                lines.push(format!(
                    "{} agent_backend.command: {} = {}",
                    ok_tag(language),
                    default_text(language),
                    render_command_prefix(&[executable.clone(), "worker-run-agent".to_string()])
                ));
                lines.push(command_prefix_resolution_line(
                    language,
                    "agent_backend.command.program",
                    Some(executable.as_str()),
                ));
            }
            Err(error) => lines.push(format!(
                "{} agent_backend.command: {} ({error})",
                warn_tag(language),
                default_current_exe_unavailable_text(language)
            )),
        },
    }

    let pane_preflight = pane_backend_preflight();
    inspect_command_prefix_status(&mut lines, &pane_preflight.tmux_command, language);
    inspect_command_prefix_status(&mut lines, &pane_preflight.iterm_command, language);

    if let Some(raw) = pane_preflight.requested_backend_raw.as_deref() {
        if pane_preflight.requested_backend.is_some() {
            lines.push(format!(
                "{} agent_pane_backend.env: HELLOX_AGENT_PANE_BACKEND={raw}",
                ok_tag(language)
            ));
        } else {
            lines.push(format!(
                "{} agent_pane_backend.env: HELLOX_AGENT_PANE_BACKEND={raw} ({})",
                warn_tag(language),
                unknown_requested_backend_text(language)
            ));
        }
    } else {
        lines.push(format!(
            "{} agent_pane_backend.env: HELLOX_AGENT_PANE_BACKEND {}",
            ok_tag(language),
            is_unset_text(language)
        ));
    }

    lines.push(format!(
        "{} agent_pane_backend.layout_presets: fanout=main-vertical, horizontal=even-horizontal, vertical=even-vertical, grid=tiled",
        ok_tag(language)
    ));

    lines.push(format!(
        "{} agent_pane_backend.tmux_session: {}",
        if pane_preflight.tmux_attached {
            ok_tag(language)
        } else {
            warn_tag(language)
        },
        env::var("TMUX")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| not_attached_text(language).to_string())
    ));
    lines.push(format!(
        "{} agent_pane_backend.tmux: {}",
        if pane_preflight.tmux_available {
            ok_tag(language)
        } else {
            warn_tag(language)
        },
        if pane_preflight.tmux_available {
            available_text(language)
        } else {
            unavailable_text(language)
        }
    ));
    lines.push(format!(
        "{} agent_pane_backend.iterm: {}",
        if pane_preflight.iterm_available {
            ok_tag(language)
        } else {
            warn_tag(language)
        },
        if pane_preflight.iterm_available {
            available_text(language)
        } else {
            iterm_reason_text(language, pane_preflight.iterm_reason.as_str())
        }
    ));
    lines.push(format!(
        "{} agent_pane_backend.detected: {}",
        if pane_preflight.detected_backend.is_some() {
            ok_tag(language)
        } else {
            warn_tag(language)
        },
        pane_preflight
            .detected_backend
            .as_deref()
            .unwrap_or(no_detected_backend_text(language))
    ));

    lines
}

fn inspect_command_prefix_status(
    lines: &mut Vec<String>,
    status: &PaneCommandPrefixStatus,
    language: AppLanguage,
) {
    if let Some(error) = status.error.as_deref() {
        lines.push(format!(
            "{} env {} {}: {error}",
            warn_tag(language),
            status.env_name,
            invalid_text(language)
        ));
        return;
    }

    let label = format!("env {}.program", status.env_name);
    if status.source == "env" {
        lines.push(format!(
            "{} env {} = {}",
            ok_tag(language),
            status.env_name,
            render_command_prefix(&status.prefix)
        ));
    } else {
        lines.push(format!(
            "{} env {} {} ({})",
            ok_tag(language),
            status.env_name,
            is_unset_text(language),
            render_command_prefix(&status.prefix)
        ));
    }
    lines.push(command_prefix_resolution_line(
        language,
        &label,
        status.prefix.first().map(String::as_str),
    ));
}

fn parse_json_array_env(raw: &str) -> Result<Vec<String>, String> {
    let items = serde_json::from_str::<Vec<String>>(raw)
        .map_err(|error| format!("expected JSON array of strings ({error})"))?;
    if items.is_empty() || items.iter().any(|item| item.trim().is_empty()) {
        return Err("expected non-empty JSON array of non-empty strings".to_string());
    }
    Ok(items)
}

fn render_command_prefix(items: &[String]) -> String {
    let rendered = items
        .iter()
        .map(|item| {
            if item.contains(' ') {
                format!("\"{item}\"")
            } else {
                item.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("[{rendered}]")
}

fn command_prefix_resolution_line(
    language: AppLanguage,
    label: &str,
    program: Option<&str>,
) -> String {
    match program
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(resolve_command_program_path)
    {
        Some(path) => format!("{} {}: {}", ok_tag(language), label, path),
        None => format!(
            "{} {}: {}",
            warn_tag(language),
            label,
            unresolved_program_text(language)
        ),
    }
}

fn resolve_command_program_path(program: &str) -> Option<String> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 || candidate.is_absolute() {
        return candidate.exists().then(|| normalize_path(candidate));
    }

    let path_var = env::var_os("PATH")?;
    let has_extension = candidate.extension().is_some();
    let windows_exts = env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .filter_map(|item| {
                    let trimmed = item.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| {
            vec![
                ".COM".to_string(),
                ".EXE".to_string(),
                ".BAT".to_string(),
                ".CMD".to_string(),
            ]
        });

    for directory in env::split_paths(&path_var) {
        let direct = directory.join(program);
        if direct.exists() {
            return Some(normalize_path(&direct));
        }
        if cfg!(windows) && !has_extension {
            for extension in &windows_exts {
                let candidate = directory.join(format!("{program}{extension}"));
                if candidate.exists() {
                    return Some(normalize_path(&candidate));
                }
            }
        }
    }

    None
}

fn ok_tag(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "[ok]",
        AppLanguage::SimplifiedChinese => "[通过]",
    }
}

fn warn_tag(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "[warn]",
        AppLanguage::SimplifiedChinese => "[警告]",
    }
}

fn invalid_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "invalid",
        AppLanguage::SimplifiedChinese => "无效",
    }
}

fn default_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "default",
        AppLanguage::SimplifiedChinese => "默认值",
    }
}

fn default_current_exe_unavailable_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "default current_exe unavailable",
        AppLanguage::SimplifiedChinese => "默认 current_exe 不可用",
    }
}

fn unknown_requested_backend_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "unknown, expected tmux|iterm",
        AppLanguage::SimplifiedChinese => "未知值，预期为 tmux|iterm",
    }
}

fn is_unset_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "is unset",
        AppLanguage::SimplifiedChinese => "未设置",
    }
}

fn not_attached_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "not attached",
        AppLanguage::SimplifiedChinese => "未附着",
    }
}

fn available_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "available",
        AppLanguage::SimplifiedChinese => "可用",
    }
}

fn unavailable_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "unavailable",
        AppLanguage::SimplifiedChinese => "不可用",
    }
}

fn no_detected_backend_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "none (pane backend will fall back to detached_process)",
        AppLanguage::SimplifiedChinese => "无（pane backend 将回退到 detached_process）",
    }
}

fn iterm_reason_text<'a>(language: AppLanguage, reason: &'a str) -> &'a str {
    match (language, reason) {
        (AppLanguage::SimplifiedChinese, "unsupported (non-macos)") => "不支持（非 macOS）",
        _ => reason,
    }
}

fn unresolved_program_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "unresolved (command may fail at runtime)",
        AppLanguage::SimplifiedChinese => "未解析（命令在运行时可能失败）",
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use crate::startup::AppLanguage;

    use super::{agent_backend_doctor_lines, resolve_command_program_path};

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.as_ref() {
                env::set_var(self.key, previous);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn resolve_command_program_path_handles_absolute_and_missing_commands() {
        let current = env::current_exe().expect("current exe");
        assert_eq!(
            resolve_command_program_path(&current.display().to_string()),
            Some(current.display().to_string().replace('\\', "/"))
        );
        assert_eq!(
            resolve_command_program_path("definitely-missing-hellox-command"),
            None
        );
    }

    #[test]
    fn agent_backend_doctor_lines_report_invalid_pane_command_env() {
        let _guard = EnvGuard::set("HELLOX_AGENT_TMUX_COMMAND", "{not-json}");

        let text = agent_backend_doctor_lines(AppLanguage::English).join("\n");
        assert!(text.contains("[warn] env HELLOX_AGENT_TMUX_COMMAND invalid:"));
    }

    #[test]
    fn agent_backend_doctor_lines_localize_tags_for_chinese() {
        let text = agent_backend_doctor_lines(AppLanguage::SimplifiedChinese).join("\n");
        assert!(text.contains("[通过]") || text.contains("[警告]"), "{text}");
    }
}
