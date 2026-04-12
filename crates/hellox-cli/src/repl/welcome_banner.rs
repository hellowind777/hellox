use hellox_agent::AgentSession;
use hellox_tui::{render_cards, Card};

use crate::startup::AppLanguage;

use super::prompt_input::example_placeholder_for_workdir;

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
    lines.extend(render_cards(&welcome_cards(
        session,
        language,
        workspace_trusted,
    )));
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

fn welcome_cards(
    session: &AgentSession,
    language: AppLanguage,
    workspace_trusted: bool,
) -> Vec<Card> {
    vec![
        Card::new(
            workspace_title(language),
            workspace_lines(session, language, workspace_trusted),
        ),
        Card::new(
            start_here_title(language),
            start_here_lines(session, language),
        ),
        Card::new(
            quick_commands_title(language),
            quick_command_lines(language),
        ),
    ]
}

fn workspace_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "workspace",
        AppLanguage::SimplifiedChinese => "工作区",
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

    match language {
        AppLanguage::English => vec![
            format!("cwd: {workdir}"),
            format!("model: {}", session.model()),
            format!("session: {session_id}"),
            format!(
                "trust: {}",
                if workspace_trusted {
                    "workspace trusted"
                } else {
                    "review required"
                }
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            format!("目录：{workdir}"),
            format!("模型：{}", session.model()),
            format!("会话：{session_id}"),
            format!(
                "信任：{}",
                if workspace_trusted {
                    "当前工作区已信任"
                } else {
                    "当前工作区待确认"
                }
            ),
        ],
    }
}

fn start_here_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "start here",
        AppLanguage::SimplifiedChinese => "开始使用",
    }
}

fn start_here_lines(session: &AgentSession, language: AppLanguage) -> Vec<String> {
    let example = example_placeholder_for_workdir(language, session.working_directory());

    match language {
        AppLanguage::English => vec![
            format!("example: {example}"),
            "prompt: type your task and press Enter".to_string(),
            "slash: type `/` then press Tab to browse commands".to_string(),
            "history: press ↑ after your first task to edit previous input".to_string(),
        ],
        AppLanguage::SimplifiedChinese => vec![
            format!("示例：{example}"),
            "输入：直接输入任务并按 Enter".to_string(),
            "斜杠：输入 `/` 后按 Tab 浏览命令".to_string(),
            "历史：完成首轮任务后可按 ↑ 编辑上一条输入".to_string(),
        ],
    }
}

fn quick_commands_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "quick commands",
        AppLanguage::SimplifiedChinese => "快捷命令",
    }
}

fn quick_command_lines(language: AppLanguage) -> Vec<String> {
    match language {
        AppLanguage::English => vec![
            "/help — show all available commands".to_string(),
            "/status — inspect the active local session".to_string(),
            "/doctor — verify gateway and provider readiness".to_string(),
            "/workflow — browse or run local workflows".to_string(),
        ],
        AppLanguage::SimplifiedChinese => vec![
            "/help —— 查看全部可用命令".to_string(),
            "/status —— 查看当前本地会话状态".to_string(),
            "/doctor —— 检查 gateway 与 provider 状态".to_string(),
            "/workflow —— 浏览或运行本地工作流".to_string(),
        ],
    }
}
