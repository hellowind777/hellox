use crate::startup::AppLanguage;

pub(super) fn usage_text(language: AppLanguage, syntax: &str) -> String {
    match language {
        AppLanguage::English => format!("Usage: {syntax}"),
        AppLanguage::SimplifiedChinese => format!("用法：{syntax}"),
    }
}

pub(super) fn no_workspace_memory_text(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => {
            "No captured session or project memory found for the current workspace.".to_string()
        }
        AppLanguage::SimplifiedChinese => "当前工作区还没有已捕获的会话或项目记忆。".to_string(),
    }
}

pub(super) fn unable_to_render_memory_panel_text(
    language: AppLanguage,
    error: &impl std::fmt::Display,
) -> String {
    match language {
        AppLanguage::English => format!("Unable to render memory panel: {error}"),
        AppLanguage::SimplifiedChinese => format!("无法渲染记忆面板：{error}"),
    }
}

pub(super) fn unable_to_load_memory_text(
    language: AppLanguage,
    memory_id: &str,
    error: &impl std::fmt::Display,
) -> String {
    match language {
        AppLanguage::English => format!("Unable to load memory `{memory_id}`: {error}"),
        AppLanguage::SimplifiedChinese => format!("无法加载记忆 `{memory_id}`：{error}"),
    }
}

pub(super) fn captured_memory_text(
    language: AppLanguage,
    mode_label: &str,
    targets: &str,
) -> String {
    match language {
        AppLanguage::English => {
            format!("Captured layered memory using {mode_label} mode. {targets}")
        }
        AppLanguage::SimplifiedChinese => {
            format!("已使用 `{mode_label}` 模式捕获分层记忆。{targets}")
        }
    }
}

pub(super) fn unable_to_render_session_panel_text(
    language: AppLanguage,
    error: &impl std::fmt::Display,
) -> String {
    match language {
        AppLanguage::English => format!("Unable to render session panel: {error}"),
        AppLanguage::SimplifiedChinese => format!("无法渲染会话面板：{error}"),
    }
}

pub(super) fn shared_transcript_written_text(language: AppLanguage, path: &str) -> String {
    match language {
        AppLanguage::English => format!("Shared transcript written to `{path}`."),
        AppLanguage::SimplifiedChinese => format!("已将转录导出到 `{path}`。"),
    }
}

pub(super) fn unable_to_share_session_text(
    language: AppLanguage,
    session_id: &str,
    error: &impl std::fmt::Display,
) -> String {
    match language {
        AppLanguage::English => format!("Unable to share `{session_id}`: {error}"),
        AppLanguage::SimplifiedChinese => format!("无法导出会话 `{session_id}`：{error}"),
    }
}

pub(super) fn no_history_to_compact_text(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => "No conversation history to compact.".to_string(),
        AppLanguage::SimplifiedChinese => "当前没有可压缩的会话历史。".to_string(),
    }
}

pub(super) fn compacted_session_text(
    language: AppLanguage,
    mode_label: &str,
    original_count: usize,
    retained_count: usize,
    targets: &str,
) -> String {
    match language {
        AppLanguage::English => format!(
            "Compacted current session in {mode_label} mode: {original_count} -> {retained_count} message(s). {targets}"
        ),
        AppLanguage::SimplifiedChinese => format!(
            "已使用 `{mode_label}` 模式压缩当前会话：{original_count} -> {retained_count} 条消息。{targets}"
        ),
    }
}

pub(super) fn no_turn_to_rewind_text(language: AppLanguage) -> String {
    match language {
        AppLanguage::English => "No conversation turn to rewind.".to_string(),
        AppLanguage::SimplifiedChinese => "当前没有可回退的对话轮次。".to_string(),
    }
}

pub(super) fn rewound_turn_text(language: AppLanguage, removed: usize) -> String {
    match language {
        AppLanguage::English => {
            format!("Rewound the most recent turn ({removed} message(s) removed).")
        }
        AppLanguage::SimplifiedChinese => {
            format!("已回退最近一轮对话（移除 {removed} 条消息）。")
        }
    }
}

pub(super) fn current_model_text(language: AppLanguage, model: &str) -> String {
    match language {
        AppLanguage::English => format!("Current model: `{model}`"),
        AppLanguage::SimplifiedChinese => format!("当前模型：`{model}`"),
    }
}

pub(super) fn unable_to_render_model_panel_text(
    language: AppLanguage,
    error: &impl std::fmt::Display,
) -> String {
    match language {
        AppLanguage::English => format!("Unable to render model panel: {error}"),
        AppLanguage::SimplifiedChinese => format!("无法渲染模型面板：{error}"),
    }
}

pub(super) fn model_set_text(language: AppLanguage, model: &str) -> String {
    match language {
        AppLanguage::English => format!("Model set to `{model}`."),
        AppLanguage::SimplifiedChinese => format!("模型已切换为 `{model}`。"),
    }
}

pub(super) fn model_help_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => concat!(
            "Usage:\n",
            "  /model                 Show the current session model\n",
            "  /model panel [name]    Show a model dashboard or inspect one profile\n",
            "  /model list            List configured model profiles\n",
            "  /model show [name]     Show the current or named model profile\n",
            "  /model use <name>      Switch the current session model\n",
            "  /model default <name>  Persist the default model profile"
        ),
        AppLanguage::SimplifiedChinese => concat!(
            "用法：\n",
            "  /model                 显示当前会话模型\n",
            "  /model panel [name]    显示模型面板或查看某个档案\n",
            "  /model list            列出已配置模型档案\n",
            "  /model show [name]     显示当前或指定模型档案\n",
            "  /model use <name>      切换当前会话模型\n",
            "  /model default <name>  持久化默认模型档案"
        ),
    }
}
