use crate::startup::AppLanguage;

pub(super) fn prompt_shell_lines(
    language: AppLanguage,
    model: &str,
    workspace_trusted: bool,
) -> Vec<String> {
    vec![
        shell_status_line(language, model, workspace_trusted),
        shell_quick_commands_line(language),
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
    use crate::startup::AppLanguage;

    use super::prompt_shell_lines;

    #[test]
    fn prompt_shell_lines_render_localized_status_and_shortcuts() {
        let lines = prompt_shell_lines(AppLanguage::SimplifiedChinese, "claude-sonnet-4-5", true);

        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("本地对话"));
        assert!(lines[0].contains("工作区已信任"));
        assert!(lines[1].contains("/workflow 工作流"));
        assert!(lines[2].contains("/ + Tab"));
    }

    #[test]
    fn prompt_shell_lines_truncate_long_models() {
        let lines = prompt_shell_lines(
            AppLanguage::English,
            "this-is-a-very-long-model-name-for-testing",
            false,
        );

        assert!(lines[0].contains("trust review"));
        assert!(lines[0].contains("…"));
    }
}
