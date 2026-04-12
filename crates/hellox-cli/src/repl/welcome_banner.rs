use hellox_agent::AgentSession;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::startup::AppLanguage;

use super::prompt_input::example_placeholder_for_workdir;

const BOX_WIDTH: usize = 62;
const BOX_CONTENT_WIDTH: usize = BOX_WIDTH - 4;
const DETAIL_LABEL_WIDTH: usize = 8;

const WELCOME_ART_LINES: &[&str] = &[
    "     *                                       █████▓▓░     ",
    "                                 *         ███▓░     ░░   ",
    "            ░░░░░░                        ███▓░           ",
    "    ░░░   ░░░░░░░░░░                      ███▓░           ",
    "   ░░░░░░░░░░░░░░░░░    *                ██▓░░      ▓   ",
    "                                             ░▓▓███▓▓░    ",
    " *                                 ░░░░                   ",
    "                                 ░░░░░░░░                 ",
    "                               ░░░░░░░░░░░░░░           ",
    "      █████████                                       * ",
    "      ██▄█████▄██                        *                ",
    "      █████████      *                                   ",
    "·······█ █   █ █······································",
];

pub(super) fn welcome_banner_lines(
    session: &AgentSession,
    language: AppLanguage,
    workspace_trusted: bool,
) -> Vec<String> {
    let mut lines = vec![
        welcome_header(language),
        "······················································".to_string(),
        String::new(),
    ];
    lines.extend(WELCOME_ART_LINES.iter().map(|line| (*line).to_string()));
    lines.push(String::new());
    lines.extend(render_box(
        workspace_title(language),
        &workspace_lines(session, language, workspace_trusted),
    ));
    lines.push(String::new());
    lines.extend(render_box(
        start_here_title(language),
        &start_here_lines(session, language),
    ));
    lines.push(String::new());
    lines.extend(render_box(
        local_flow_title(language),
        &local_flow_lines(language),
    ));
    lines
}

fn welcome_header(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => format!("Welcome to hellox v{} ", env!("CARGO_PKG_VERSION")),
        AppLanguage::SimplifiedChinese => {
            format!("欢迎使用 hellox v{} ", env!("CARGO_PKG_VERSION"))
        }
    }
}

fn workspace_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Workspace",
        AppLanguage::SimplifiedChinese => "当前工作区",
    }
}

fn start_here_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Start Here",
        AppLanguage::SimplifiedChinese => "开始使用",
    }
}

fn local_flow_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Local Flow",
        AppLanguage::SimplifiedChinese => "本地链路",
    }
}

fn workspace_lines(
    session: &AgentSession,
    language: AppLanguage,
    workspace_trusted: bool,
) -> Vec<String> {
    let workdir = session
        .working_directory()
        .display()
        .to_string()
        .replace('\\', "/");
    let session_id = session.session_id().unwrap_or("new local session");
    let trust_status = match (language, workspace_trusted) {
        (AppLanguage::English, true) => "trusted workspace",
        (AppLanguage::English, false) => "trust review required",
        (AppLanguage::SimplifiedChinese, true) => "工作区已信任",
        (AppLanguage::SimplifiedChinese, false) => "工作区待确认",
    };
    let language_label = match language {
        AppLanguage::English => "English",
        AppLanguage::SimplifiedChinese => "简体中文",
    };

    vec![
        detail_line(language, "cwd", "目录", &workdir),
        detail_line(language, "model", "模型", session.model()),
        detail_line(language, "session", "会话", session_id),
        detail_line(language, "trust", "信任", trust_status),
        detail_line(language, "language", "语言", language_label),
    ]
}

fn start_here_lines(session: &AgentSession, language: AppLanguage) -> Vec<String> {
    let example = example_placeholder_for_workdir(language, session.working_directory());

    match language {
        AppLanguage::English => vec![
            detail_line(language, "example", "示例", &example),
            detail_line(language, "send", "发送", "Type your task and press Enter"),
            detail_line(
                language,
                "slash",
                "命令",
                "Type `/` then press Tab for commands",
            ),
            detail_line(language, "history", "历史", "Press ↑ after the first task"),
        ],
        AppLanguage::SimplifiedChinese => vec![
            detail_line(language, "example", "示例", &example),
            detail_line(language, "发送", "发送", "直接输入任务并按 Enter"),
            detail_line(language, "命令", "命令", "输入 `/` 后按 Tab 浏览斜杠命令"),
            detail_line(language, "历史", "历史", "完成首轮后可按 ↑ 编辑上一条输入"),
        ],
    }
}

fn local_flow_lines(language: AppLanguage) -> Vec<String> {
    match language {
        AppLanguage::English => vec![
            detail_line(
                language,
                "gateway",
                "网关",
                "Third-party APIs route through the local gateway",
            ),
            detail_line(
                language,
                "format",
                "格式",
                "Requests are translated into Anthropic Messages",
            ),
            detail_line(
                language,
                "safety",
                "安全",
                "Workspace trust gates local file access and execution",
            ),
            detail_line(
                language,
                "help",
                "帮助",
                "/help  /shortcuts  /doctor  /workflow",
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            detail_line(language, "网关", "网关", "第三方 API 会先接入本地 gateway"),
            detail_line(
                language,
                "格式",
                "格式",
                "请求会在本地统一转换为 Anthropic Messages",
            ),
            detail_line(
                language,
                "安全",
                "安全",
                "工作区信任会先保护本地文件读取、编辑与执行",
            ),
            detail_line(
                language,
                "帮助",
                "帮助",
                "/help  /shortcuts  /doctor  /workflow",
            ),
        ],
    }
}

fn detail_line(
    language: AppLanguage,
    english_label: &str,
    simplified_chinese_label: &str,
    value: &str,
) -> String {
    let label = match language {
        AppLanguage::English => english_label,
        AppLanguage::SimplifiedChinese => simplified_chinese_label,
    };
    let padded_label = format!("{label:<width$}", width = DETAIL_LABEL_WIDTH);
    format!("{padded_label} {}", value.trim())
}

fn render_box(title: &str, lines: &[String]) -> Vec<String> {
    let title_text = format!(" {title} ");
    let top_fill = "─".repeat(BOX_WIDTH.saturating_sub(title_text.chars().count() + 3));
    let mut rendered = vec![format!("╭─{title_text}{top_fill}╮")];

    if lines.is_empty() {
        rendered.push(render_box_line("(none)"));
    } else {
        rendered.extend(lines.iter().map(|line| render_box_line(line)));
    }

    rendered.push(format!("╰{}╯", "─".repeat(BOX_WIDTH.saturating_sub(2))));
    rendered
}

fn render_box_line(line: &str) -> String {
    let content = truncate_text(line.trim(), BOX_CONTENT_WIDTH);
    let padding = BOX_CONTENT_WIDTH.saturating_sub(UnicodeWidthStr::width(content.as_str()));
    format!("│ {content}{} │", " ".repeat(padding))
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let width = UnicodeWidthStr::width(text);
    if width <= max_chars {
        return text.to_string();
    }

    if max_chars == 0 {
        return String::new();
    }

    let mut compact = String::new();
    let mut consumed = 0;
    let limit = max_chars.saturating_sub(1);
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
