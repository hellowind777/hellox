use hellox_config::PermissionMode;

use crate::startup::AppLanguage;

pub(super) fn prompt_shell_lines(
    language: AppLanguage,
    model: &str,
    workspace_trusted: bool,
    activity_line: String,
    permission_mode: &PermissionMode,
    gateway_listen: &str,
    session_persist: bool,
) -> Vec<String> {
    vec![
        shell_status_line(language, model, workspace_trusted),
        activity_line,
        shell_quick_commands_line(language),
        shell_runtime_status_line(language, permission_mode, gateway_listen, session_persist),
        shell_shortcuts_line(language),
    ]
}

fn shell_status_line(language: AppLanguage, model: &str, workspace_trusted: bool) -> String {
    let model = truncate_model(model);
    match language {
        AppLanguage::English => format!(
            "╭─ local chat · model {model} · {}",
            if workspace_trusted {
                "trusted workspace"
            } else {
                "trust review"
            }
        ),
        AppLanguage::SimplifiedChinese => format!(
            "╭─ 本地对话 · 模型 {model} · {}",
            if workspace_trusted {
                "工作区已信任"
            } else {
                "工作区待确认"
            }
        ),
    }
}

fn shell_quick_commands_line(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => {
            "│ /help commands · /status session · /doctor diagnostics · /workflow flows".to_string()
        }
        AppLanguage::SimplifiedChinese => {
            "│ /help 命令 · /status 状态 · /doctor 诊断 · /workflow 工作流".to_string()
        }
    }
}

fn shell_shortcuts_line(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => {
            "│ Type / + Tab for commands · Enter to send · ↑ history".to_string()
        }
        AppLanguage::SimplifiedChinese => {
            "│ 输入 / + Tab 浏览命令 · Enter 发送 · ↑ 历史编辑".to_string()
        }
    }
}

fn shell_runtime_status_line(
    language: AppLanguage,
    permission_mode: &PermissionMode,
    gateway_listen: &str,
    session_persist: bool,
) -> String {
    match language {
        AppLanguage::English => format!(
            "│ {} · gateway {gateway_listen} · {}",
            permission_mode_label(language, permission_mode),
            if session_persist {
                "session persisted"
            } else {
                "session-only"
            }
        ),
        AppLanguage::SimplifiedChinese => format!(
            "│ {} · gateway {gateway_listen} · {}",
            permission_mode_label(language, permission_mode),
            if session_persist {
                "本地持久化"
            } else {
                "仅当前会话"
            }
        ),
    }
}

fn permission_mode_label(language: AppLanguage, permission_mode: &PermissionMode) -> &'static str {
    match (language, permission_mode) {
        (AppLanguage::English, PermissionMode::Default) => "default approvals",
        (AppLanguage::English, PermissionMode::AcceptEdits) => "accept edits",
        (AppLanguage::English, PermissionMode::BypassPermissions) => "bypass permissions",
        (AppLanguage::SimplifiedChinese, PermissionMode::Default) => "默认审批",
        (AppLanguage::SimplifiedChinese, PermissionMode::AcceptEdits) => "接受编辑",
        (AppLanguage::SimplifiedChinese, PermissionMode::BypassPermissions) => "绕过审批",
    }
}

fn truncate_model(model: &str) -> String {
    const MAX_MODEL_CHARS: usize = 18;
    let count = model.chars().count();
    if count <= MAX_MODEL_CHARS {
        return model.to_string();
    }

    let mut compact = model.chars().take(MAX_MODEL_CHARS - 1).collect::<String>();
    compact.push('…');
    compact
}

#[cfg(test)]
mod tests {
    use hellox_config::PermissionMode;

    use crate::startup::AppLanguage;

    use super::prompt_shell_lines;

    #[test]
    fn prompt_shell_lines_render_localized_status_and_shortcuts() {
        let lines = prompt_shell_lines(
            AppLanguage::SimplifiedChinese,
            "claude-sonnet-4-5",
            true,
            "│ 计划进行中 · 2 个计划步骤 · 3 个本地任务 · 1 个进行中".to_string(),
            &PermissionMode::AcceptEdits,
            "127.0.0.1:7821",
            true,
        );

        assert_eq!(lines.len(), 5);
        assert!(lines[0].contains("本地对话"));
        assert!(lines[0].contains("工作区已信任"));
        assert!(lines[1].contains("计划进行中"));
        assert!(lines[1].contains("3 个本地任务"));
        assert!(lines[2].contains("/workflow 工作流"));
        assert!(lines[3].contains("接受编辑"));
        assert!(lines[3].contains("本地持久化"));
        assert!(lines[4].contains("/ + Tab"));
    }

    #[test]
    fn prompt_shell_lines_truncate_long_models() {
        let lines = prompt_shell_lines(
            AppLanguage::English,
            "this-is-a-very-long-model-name-for-testing",
            false,
            "│ plan ready · no local tasks".to_string(),
            &PermissionMode::BypassPermissions,
            "127.0.0.1:9000",
            false,
        );

        assert!(lines[0].contains("trust review"));
        assert!(lines[1].contains("plan ready"));
        assert!(lines[3].contains("bypass permissions"));
        assert!(lines[3].contains("session-only"));
        assert!(lines[0].contains("…"));
    }
}
