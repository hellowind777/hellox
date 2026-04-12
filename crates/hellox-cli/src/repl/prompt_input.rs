use std::path::Path;

use hellox_agent::AgentSession;
use hellox_repl::{ReplCompletion, ReplPromptState};

use crate::startup::AppLanguage;

use super::prompt_shell_copy::prompt_shell_lines;

struct PromptCommandCopy {
    value: &'static str,
    english: &'static str,
    simplified_chinese: &'static str,
}

const PROMPT_COMMANDS: &[PromptCommandCopy] = &[
    PromptCommandCopy {
        value: "/help",
        english: "show available commands",
        simplified_chinese: "查看可用命令",
    },
    PromptCommandCopy {
        value: "/status",
        english: "show the active session",
        simplified_chinese: "查看当前会话状态",
    },
    PromptCommandCopy {
        value: "/doctor",
        english: "check gateway and provider readiness",
        simplified_chinese: "检查 gateway 与 provider 状态",
    },
    PromptCommandCopy {
        value: "/usage",
        english: "show usage and cost details",
        simplified_chinese: "查看用量与费用概览",
    },
    PromptCommandCopy {
        value: "/stats",
        english: "show session statistics",
        simplified_chinese: "查看会话统计",
    },
    PromptCommandCopy {
        value: "/cost",
        english: "show model cost summary",
        simplified_chinese: "查看模型成本摘要",
    },
    PromptCommandCopy {
        value: "/brief",
        english: "inspect or update the local brief",
        simplified_chinese: "查看或更新本地 brief",
    },
    PromptCommandCopy {
        value: "/tools",
        english: "run local tool queries",
        simplified_chinese: "执行本地工具查询",
    },
    PromptCommandCopy {
        value: "/search",
        english: "search the current workspace",
        simplified_chinese: "搜索当前工作区",
    },
    PromptCommandCopy {
        value: "/config",
        english: "inspect or edit configuration",
        simplified_chinese: "查看或编辑配置",
    },
    PromptCommandCopy {
        value: "/model",
        english: "inspect or switch models",
        simplified_chinese: "查看或切换模型",
    },
    PromptCommandCopy {
        value: "/permissions",
        english: "show or change approval mode",
        simplified_chinese: "查看或切换权限模式",
    },
    PromptCommandCopy {
        value: "/plan",
        english: "inspect or manage the active plan",
        simplified_chinese: "查看或管理当前计划",
    },
    PromptCommandCopy {
        value: "/tasks",
        english: "inspect or manage local tasks",
        simplified_chinese: "查看或管理本地任务",
    },
    PromptCommandCopy {
        value: "/workflow",
        english: "browse or run local workflows",
        simplified_chinese: "浏览或运行本地工作流",
    },
    PromptCommandCopy {
        value: "/memory",
        english: "inspect or manage memory items",
        simplified_chinese: "查看或管理记忆条目",
    },
    PromptCommandCopy {
        value: "/session",
        english: "inspect or share sessions",
        simplified_chinese: "查看或分享会话",
    },
    PromptCommandCopy {
        value: "/resume",
        english: "switch to another stored session",
        simplified_chinese: "切换到其他已保存会话",
    },
    PromptCommandCopy {
        value: "/share",
        english: "export the current transcript",
        simplified_chinese: "导出当前会话记录",
    },
    PromptCommandCopy {
        value: "/compact",
        english: "compact the current conversation",
        simplified_chinese: "压缩当前对话上下文",
    },
    PromptCommandCopy {
        value: "/rewind",
        english: "remove the latest turn",
        simplified_chinese: "回退最近一轮消息",
    },
    PromptCommandCopy {
        value: "/clear",
        english: "clear the current conversation",
        simplified_chinese: "清空当前会话消息",
    },
    PromptCommandCopy {
        value: "/output-style",
        english: "switch output styles",
        simplified_chinese: "切换输出风格",
    },
    PromptCommandCopy {
        value: "/persona",
        english: "switch personas",
        simplified_chinese: "切换人设",
    },
    PromptCommandCopy {
        value: "/fragment",
        english: "manage prompt fragments",
        simplified_chinese: "管理提示片段",
    },
    PromptCommandCopy {
        value: "/skills",
        english: "inspect installed skills",
        simplified_chinese: "查看已安装技能",
    },
    PromptCommandCopy {
        value: "/hooks",
        english: "inspect installed hooks",
        simplified_chinese: "查看已安装 hooks",
    },
    PromptCommandCopy {
        value: "/bridge",
        english: "inspect the local bridge state",
        simplified_chinese: "查看本地 bridge 状态",
    },
    PromptCommandCopy {
        value: "/ide",
        english: "inspect the IDE panel",
        simplified_chinese: "查看 IDE 面板",
    },
    PromptCommandCopy {
        value: "/mcp",
        english: "inspect or manage MCP servers",
        simplified_chinese: "查看或管理 MCP 服务",
    },
    PromptCommandCopy {
        value: "/plugin",
        english: "inspect or manage plugins",
        simplified_chinese: "查看或管理插件",
    },
    PromptCommandCopy {
        value: "/remote-env",
        english: "inspect remote environment definitions",
        simplified_chinese: "查看远程环境定义",
    },
    PromptCommandCopy {
        value: "/teleport",
        english: "inspect direct-connect environments",
        simplified_chinese: "查看直连环境",
    },
    PromptCommandCopy {
        value: "/assistant",
        english: "inspect assistant sessions",
        simplified_chinese: "查看助手会话",
    },
    PromptCommandCopy {
        value: "/install",
        english: "install the local binary",
        simplified_chinese: "安装本地二进制",
    },
    PromptCommandCopy {
        value: "/upgrade",
        english: "upgrade from a local binary",
        simplified_chinese: "从本地二进制升级",
    },
    PromptCommandCopy {
        value: "/exit",
        english: "leave the interactive session",
        simplified_chinese: "退出交互会话",
    },
];

pub(super) fn prompt_state(
    session: &AgentSession,
    language: AppLanguage,
    has_prior_submit: bool,
    workspace_trusted: bool,
) -> ReplPromptState {
    let placeholder = if has_prior_submit || session.message_count() > 0 {
        Some(continuation_placeholder(language))
    } else {
        Some(example_placeholder_for_workdir(
            language,
            session.working_directory(),
        ))
    };

    ReplPromptState::with_shell(
        placeholder,
        prompt_shell_lines(language, session.model(), workspace_trusted),
        PROMPT_COMMANDS
            .iter()
            .map(|command| {
                ReplCompletion::described(
                    command.value,
                    match language {
                        AppLanguage::English => command.english,
                        AppLanguage::SimplifiedChinese => command.simplified_chinese,
                    },
                )
            })
            .collect(),
    )
}

fn continuation_placeholder(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => {
            "Type another task, use `/` for commands, or press ↑ to edit the previous input"
                .to_string()
        }
        AppLanguage::SimplifiedChinese => {
            "继续输入任务，输入 `/` 查看命令，或按 ↑ 编辑上一条输入".to_string()
        }
    }
}

pub(super) fn example_placeholder_for_workdir(
    language: AppLanguage,
    working_directory: &Path,
) -> String {
    let is_rust_workspace = working_directory.join("Cargo.toml").exists();
    let is_js_workspace = working_directory.join("package.json").exists();

    match (language, is_rust_workspace, is_js_workspace) {
        (AppLanguage::English, true, _) => "Explain this Rust workspace".to_string(),
        (AppLanguage::English, false, true) => "Explain this JavaScript project".to_string(),
        (AppLanguage::English, false, false) => "Explain this repository".to_string(),
        (AppLanguage::SimplifiedChinese, true, _) => "解释这个 Rust 工作区的结构".to_string(),
        (AppLanguage::SimplifiedChinese, false, true) => {
            "解释这个 JavaScript 项目的结构".to_string()
        }
        (AppLanguage::SimplifiedChinese, false, false) => "解释这个仓库的结构".to_string(),
    }
}
