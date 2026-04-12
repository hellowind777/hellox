use hellox_tui::Card;

use super::AppLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrustChoice {
    Trust,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrustMode {
    RememberWorkspace,
    SessionOnly,
}

impl TrustChoice {
    pub(super) fn from_input(language: AppLanguage, input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() || language.accepts_input(trimmed) {
            return Some(Self::Trust);
        }
        if language.rejects_input(trimmed)
            || trimmed == "\u{1b}"
            || trimmed.eq_ignore_ascii_case("esc")
            || trimmed.eq_ignore_ascii_case("exit")
        {
            return Some(Self::Exit);
        }
        None
    }
}

pub(super) fn dialog_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Accessing workspace:",
        AppLanguage::SimplifiedChinese => "正在访问工作区：",
    }
}

pub(super) fn trust_cards(
    language: AppLanguage,
    working_directory: &str,
    mode: TrustMode,
    store_path: &str,
) -> Vec<Card> {
    match language {
        AppLanguage::English => vec![
            Card::new(
                "workspace",
                vec![
                    working_directory.to_string(),
                    "Quick safety check: Is this a project you created or one you trust?"
                        .to_string(),
                    "If not, review what is in this folder before continuing.".to_string(),
                ],
            ),
            Card::new(
                "permissions",
                vec![
                    "hellox will be able to read, edit, and execute files here.".to_string(),
                    "Slash commands, local workflows, hooks, and gateway-backed requests can use this workspace.".to_string(),
                    match mode {
                        TrustMode::RememberWorkspace => {
                            format!("This decision is remembered locally in {store_path}.")
                        }
                        TrustMode::SessionOnly => {
                            "Because this is your home directory, trust lasts for this session only.".to_string()
                        }
                    },
                ],
            ),
            Card::new(
                "security",
                vec![
                    "Security guide: https://code.claude.com/docs/en/security".to_string(),
                    "Default action: press Enter to trust this folder.".to_string(),
                    "Exit: type 2, no, or Esc then Enter.".to_string(),
                ],
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            Card::new(
                "工作区",
                vec![
                    working_directory.to_string(),
                    "快速安全检查：这是你创建的项目，或你明确可信的代码目录吗？"
                        .to_string(),
                    "如果不是，请先检查目录内容，再决定是否继续。".to_string(),
                ],
            ),
            Card::new(
                "能力边界",
                vec![
                    "继续后，hellox 将可以在这里读取、编辑并执行文件。".to_string(),
                    "斜杠命令、本地工作流、hooks，以及经 gateway 转换的请求都可以使用这个工作区。".to_string(),
                    match mode {
                        TrustMode::RememberWorkspace => {
                            format!("该决定只会保存在本机：{store_path}")
                        }
                        TrustMode::SessionOnly => {
                            "由于当前目录是家目录，本次信任仅在当前会话内生效。".to_string()
                        }
                    },
                ],
            ),
            Card::new(
                "安全提示",
                vec![
                    "安全指南：https://code.claude.com/docs/en/security".to_string(),
                    "默认动作：直接回车即可信任当前目录。".to_string(),
                    "退出：输入 2、no，或按 Esc 后回车。".to_string(),
                ],
            ),
        ],
    }
}

pub(super) fn prompt_label(language: AppLanguage, mode: TrustMode) -> &'static str {
    match (language, mode) {
        (AppLanguage::English, TrustMode::RememberWorkspace) => {
            "Press Enter to trust and remember, or type 2 to exit: "
        }
        (AppLanguage::English, TrustMode::SessionOnly) => {
            "Press Enter to trust for this session, or type 2 to exit: "
        }
        (AppLanguage::SimplifiedChinese, TrustMode::RememberWorkspace) => {
            "直接回车即可信任并记住，或输入 2 退出："
        }
        (AppLanguage::SimplifiedChinese, TrustMode::SessionOnly) => {
            "直接回车即可仅信任本会话，或输入 2 退出："
        }
    }
}

pub(super) fn invalid_choice_text(language: AppLanguage, mode: TrustMode) -> &'static str {
    match (language, mode) {
        (AppLanguage::English, TrustMode::RememberWorkspace) => {
            "Press Enter to trust and remember this folder, or type 2 to exit."
        }
        (AppLanguage::English, TrustMode::SessionOnly) => {
            "Press Enter to trust this home directory for the current session, or type 2 to exit."
        }
        (AppLanguage::SimplifiedChinese, TrustMode::RememberWorkspace) => {
            "直接回车即可信任并记住当前目录，或输入 2 退出。"
        }
        (AppLanguage::SimplifiedChinese, TrustMode::SessionOnly) => {
            "直接回车即可仅信任当前会话，或输入 2 退出。"
        }
    }
}

pub(super) fn accepted_cards(
    language: AppLanguage,
    working_directory: &str,
    mode: TrustMode,
) -> Vec<Card> {
    match (language, mode) {
        (AppLanguage::English, TrustMode::RememberWorkspace) => vec![Card::new(
            "trust accepted",
            vec![
                format!("Workspace trusted: {working_directory}"),
                "This machine will remember the decision for future launches.".to_string(),
                "Startup continues to the welcome screen and input prompt.".to_string(),
            ],
        )],
        (AppLanguage::English, TrustMode::SessionOnly) => vec![Card::new(
            "trust accepted",
            vec![
                format!("Home directory trusted for this session: {working_directory}"),
                "The decision is not persisted to disk.".to_string(),
                "Startup continues to the welcome screen and input prompt.".to_string(),
            ],
        )],
        (AppLanguage::SimplifiedChinese, TrustMode::RememberWorkspace) => vec![Card::new(
            "已接受信任",
            vec![
                format!("已信任工作区：{working_directory}"),
                "本机后续启动会记住这次决定。".to_string(),
                "启动流程将继续进入欢迎首屏和输入区。".to_string(),
            ],
        )],
        (AppLanguage::SimplifiedChinese, TrustMode::SessionOnly) => vec![Card::new(
            "已接受信任",
            vec![
                format!("当前会话已信任家目录：{working_directory}"),
                "该决定不会写入磁盘。".to_string(),
                "启动流程将继续进入欢迎首屏和输入区。".to_string(),
            ],
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::{TrustChoice, TrustMode};
    use crate::startup::AppLanguage;

    #[test]
    fn empty_input_defaults_to_trust() {
        assert_eq!(
            TrustChoice::from_input(AppLanguage::English, ""),
            Some(TrustChoice::Trust)
        );
    }

    #[test]
    fn escape_aliases_exit_the_dialog() {
        assert_eq!(
            TrustChoice::from_input(AppLanguage::English, "esc"),
            Some(TrustChoice::Exit)
        );
        assert_eq!(
            TrustChoice::from_input(AppLanguage::SimplifiedChinese, "\u{1b}"),
            Some(TrustChoice::Exit)
        );
    }

    #[test]
    fn prompt_copy_changes_with_trust_mode() {
        assert!(
            super::prompt_label(AppLanguage::English, TrustMode::RememberWorkspace)
                .contains("remember")
        );
        assert!(
            super::prompt_label(AppLanguage::English, TrustMode::SessionOnly).contains("session")
        );
    }
}
