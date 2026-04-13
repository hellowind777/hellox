use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::startup::AppLanguage;

const DIALOG_WIDTH: usize = 78;
const CONTENT_WIDTH: usize = DIALOG_WIDTH - 2;
const CONTENT_INDENT: &str = "  ";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrustSelection {
    Trust,
    Exit,
}

impl TrustSelection {
    pub(super) fn previous(self) -> Self {
        match self {
            Self::Trust => Self::Exit,
            Self::Exit => Self::Trust,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::Trust => Self::Exit,
            Self::Exit => Self::Trust,
        }
    }

    pub(super) fn choice(self) -> TrustChoice {
        match self {
            Self::Trust => TrustChoice::Trust,
            Self::Exit => TrustChoice::Exit,
        }
    }
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

pub(super) fn trust_dialog_lines(
    language: AppLanguage,
    working_directory: &str,
    selection: TrustSelection,
    exit_pending: bool,
) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        render_top_border(),
        format!("{CONTENT_INDENT}{}", dialog_title(language)),
        String::new(),
    ];
    lines.extend(
        dialog_body_lines(language, working_directory)
            .into_iter()
            .map(|line| format!("{CONTENT_INDENT}{line}")),
    );
    lines.push(String::new());
    lines.extend(
        option_lines(language, selection)
            .into_iter()
            .map(|line| format!("{CONTENT_INDENT}{line}")),
    );
    lines.push(String::new());
    lines.push(format!(
        "{CONTENT_INDENT}{}",
        footer_text(language, exit_pending)
    ));
    lines
}

pub(super) fn invalid_choice_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Enter 1 to trust this folder, or type 2 / esc to cancel.",
        AppLanguage::SimplifiedChinese => "输入 1 信任此目录，或输入 2 / esc 取消。",
    }
}

pub(super) fn prompt_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Select 1 or 2, then press Enter: ",
        AppLanguage::SimplifiedChinese => "请输入 1 或 2，然后按 Enter：",
    }
}

pub(super) fn fallback_notice_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => {
            "Interactive selection is unavailable here, falling back to typed confirmation."
        }
        AppLanguage::SimplifiedChinese => "当前终端无法使用交互式选择，已回退为输入确认模式。",
    }
}

fn dialog_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Accessing workspace:",
        AppLanguage::SimplifiedChinese => "正在访问工作区：",
    }
}

fn dialog_body_lines(language: AppLanguage, working_directory: &str) -> Vec<String> {
    let mut lines = wrap_text(working_directory);
    lines.push(String::new());
    match language {
        AppLanguage::English => {
            lines.extend(wrap_text(
                "Quick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source project, or work from your team). If not, take a moment to review what's in this folder first.",
            ));
            lines.push(String::new());
            lines.extend(wrap_text(
                "hellox will be able to read, edit, and execute files here.",
            ));
            lines.push(String::new());
            lines.push("Security guide: https://code.claude.com/docs/en/security".to_string());
        }
        AppLanguage::SimplifiedChinese => {
            lines.extend(wrap_text(
                "快速安全检查：这是你创建的项目，或你信任的项目吗？例如你自己的代码、知名开源项目，或团队内部的工作目录。如果不是，请先检查此目录中的内容，再决定是否继续。",
            ));
            lines.push(String::new());
            lines.extend(wrap_text(
                "继续后，hellox 将可以在这里读取、编辑并执行文件。",
            ));
            lines.push(String::new());
            lines.push("安全指南：https://code.claude.com/docs/en/security".to_string());
        }
    }
    lines
}

fn option_lines(language: AppLanguage, selection: TrustSelection) -> Vec<String> {
    let options = match language {
        AppLanguage::English => [("1", "Yes, I trust this folder"), ("2", "No, exit")],
        AppLanguage::SimplifiedChinese => [("1", "是的，我信任这个目录"), ("2", "不，退出")],
    };

    options
        .into_iter()
        .enumerate()
        .map(|(index, (number, label))| {
            let is_selected = matches!(
                (index, selection),
                (0, TrustSelection::Trust) | (1, TrustSelection::Exit)
            );
            let marker = if is_selected { "❯" } else { " " };
            format!("{marker} {number}. {label}")
        })
        .collect()
}

fn footer_text(language: AppLanguage, exit_pending: bool) -> &'static str {
    match (language, exit_pending) {
        (AppLanguage::English, false) => "Enter to confirm · Esc to cancel",
        (AppLanguage::English, true) => "Press Ctrl+C again to exit",
        (AppLanguage::SimplifiedChinese, false) => "Enter 确认 · Esc 取消",
        (AppLanguage::SimplifiedChinese, true) => "再按一次 Ctrl+C 即可退出",
    }
}

fn render_top_border() -> String {
    format!("╭{}╮", "─".repeat(DIALOG_WIDTH.saturating_sub(2)))
}

fn wrap_text(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let mut last_whitespace_byte = None;

    for character in text.chars() {
        if character == '\n' {
            lines.push(current.trim_end().to_string());
            current.clear();
            last_whitespace_byte = None;
            continue;
        }

        current.push(character);
        if character.is_whitespace() {
            last_whitespace_byte = Some(current.len());
        }

        if display_width(&current) <= CONTENT_WIDTH {
            continue;
        }

        if let Some(index) = last_whitespace_byte {
            let line = current[..index].trim_end().to_string();
            let remainder = current[index..].trim_start().to_string();
            if !line.is_empty() {
                lines.push(line);
            }
            current = remainder;
        } else {
            let mut compact = String::new();
            for ch in current.chars() {
                let candidate = format!("{compact}{ch}");
                if display_width(&candidate) > CONTENT_WIDTH {
                    break;
                }
                compact.push(ch);
            }

            if compact.is_empty() {
                lines.push(truncate_text(&current, CONTENT_WIDTH));
                current.clear();
            } else {
                let remainder = current[compact.len()..].to_string();
                lines.push(compact);
                current = remainder;
            }
        }

        last_whitespace_byte = current
            .char_indices()
            .rev()
            .find_map(|(index, ch)| ch.is_whitespace().then_some(index + ch.len_utf8()));
    }

    if !current.is_empty() {
        lines.push(current.trim_end().to_string());
    }

    if lines.is_empty() {
        return vec![String::new()];
    }

    lines
}

fn truncate_text(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    let mut compact = String::new();
    let mut consumed = 0;
    let limit = max_width.saturating_sub(1);
    for character in text.chars() {
        let char_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if consumed + char_width > limit {
            break;
        }
        compact.push(character);
        consumed += char_width;
    }
    compact.push('…');
    compact
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

#[cfg(test)]
mod tests {
    use super::{
        display_width, prompt_label, trust_dialog_lines, TrustChoice, TrustSelection, DIALOG_WIDTH,
    };
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
    fn trust_dialog_renders_boxed_layout_and_selected_option() {
        let lines = trust_dialog_lines(
            AppLanguage::English,
            "C:/repo/project",
            TrustSelection::Trust,
            false,
        );

        assert!(lines
            .iter()
            .any(|line| line.contains("Accessing workspace")));
        assert!(lines
            .iter()
            .any(|line| line.starts_with('╭') && line.ends_with('╮')));
        assert!(lines.iter().any(|line| line == "  Accessing workspace:"));
        assert!(lines
            .iter()
            .any(|line| line.contains("❯ 1. Yes, I trust this folder")));
        assert!(lines.iter().any(|line| line.contains("2. No, exit")));
        assert!(lines.iter().any(|line| line.contains("Security guide")));
        assert!(lines
            .iter()
            .any(|line| line.contains("Enter to confirm · Esc to cancel")));
    }

    #[test]
    fn dialog_footer_changes_when_exit_is_pending() {
        let lines = trust_dialog_lines(
            AppLanguage::SimplifiedChinese,
            "D:/workspace",
            TrustSelection::Exit,
            true,
        );
        assert!(lines.iter().any(|line| line.contains("再按一次 Ctrl+C")));
        assert!(lines.iter().any(|line| line.contains("❯ 2. 不，退出")));
    }

    #[test]
    fn chinese_copy_wraps_without_spacing_breakage() {
        let lines = trust_dialog_lines(
            AppLanguage::SimplifiedChinese,
            "D:/一个非常长的目录/用于验证中文段落在没有空格时也能正常自动换行/而不是直接溢出终端边界",
            TrustSelection::Trust,
            false,
        );
        assert!(lines
            .iter()
            .all(|line| display_width(line) <= DIALOG_WIDTH + 2));
    }

    #[test]
    fn fallback_prompt_is_localized() {
        assert_eq!(
            prompt_label(AppLanguage::English),
            "Select 1 or 2, then press Enter: "
        );
        assert_eq!(
            prompt_label(AppLanguage::SimplifiedChinese),
            "请输入 1 或 2，然后按 Enter："
        );
    }

    #[test]
    fn invalid_choice_copy_matches_typed_fallback_flow() {
        assert_eq!(
            super::invalid_choice_text(AppLanguage::English),
            "Enter 1 to trust this folder, or type 2 / esc to cancel."
        );
        assert_eq!(
            super::invalid_choice_text(AppLanguage::SimplifiedChinese),
            "输入 1 信任此目录，或输入 2 / esc 取消。"
        );
    }
}
