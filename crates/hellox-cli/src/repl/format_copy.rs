use crate::startup::AppLanguage;

pub(super) fn help_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Available slash commands:",
        AppLanguage::SimplifiedChinese => "可用斜杠命令：",
    }
}

pub(super) fn help_core_local_workflow_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Core local workflow:",
        AppLanguage::SimplifiedChinese => "核心本地工作流：",
    }
}

pub(super) fn help_local_integration_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Local integration:",
        AppLanguage::SimplifiedChinese => "本地集成：",
    }
}

pub(super) fn help_remote_commands_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Optional remote-capable commands:",
        AppLanguage::SimplifiedChinese => "可选远程能力命令：",
    }
}

pub(super) fn project_workflow_commands_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Project workflow commands:",
        AppLanguage::SimplifiedChinese => "项目工作流命令：",
    }
}

pub(super) fn project_workflow_command_line(language: AppLanguage, workflow_name: &str) -> String {
    match language {
        AppLanguage::English => {
            format!("  /{workflow_name} [shared_context] Run project workflow `{workflow_name}`")
        }
        AppLanguage::SimplifiedChinese => {
            format!("  /{workflow_name} [shared_context] 运行项目工作流 `{workflow_name}`")
        }
    }
}

pub(super) fn workflow_discovery_error_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "workflow_discovery_error",
        AppLanguage::SimplifiedChinese => "工作流发现错误",
    }
}

pub(super) fn resume_usage_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Use `/resume <session-id>` to switch sessions.",
        AppLanguage::SimplifiedChinese => "使用 `/resume <session-id>` 可切换到指定会话。",
    }
}

pub(super) fn no_persisted_sessions_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => {
            "No persisted sessions found. Start a session with persistence enabled first."
        }
        AppLanguage::SimplifiedChinese => "未找到已持久化会话。请先启动一个启用了持久化的会话。",
    }
}

pub(super) fn unable_to_inspect_sessions_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to inspect persisted sessions",
        AppLanguage::SimplifiedChinese => "无法检查已持久化会话",
    }
}

pub(super) fn unable_to_inspect_workspace_health_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to inspect workspace health",
        AppLanguage::SimplifiedChinese => "无法检查工作区健康状态",
    }
}

pub(super) fn unable_to_inspect_workspace_usage_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to inspect workspace usage",
        AppLanguage::SimplifiedChinese => "无法检查工作区使用情况",
    }
}

pub(super) fn unable_to_inspect_workspace_stats_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to inspect workspace stats",
        AppLanguage::SimplifiedChinese => "无法检查工作区统计信息",
    }
}

pub(super) fn unable_to_load_config_for_cost_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to load config for cost inspection",
        AppLanguage::SimplifiedChinese => "无法加载用于成本检查的配置",
    }
}

pub(super) fn unable_to_inspect_cost_state_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to inspect cost state",
        AppLanguage::SimplifiedChinese => "无法检查成本状态",
    }
}

pub(super) fn unable_to_load_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to load",
        AppLanguage::SimplifiedChinese => "无法加载",
    }
}

pub(super) fn unable_to_search_persisted_sessions_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to search persisted sessions",
        AppLanguage::SimplifiedChinese => "无法搜索已持久化会话",
    }
}

pub(super) fn unable_to_search_memory_files_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Unable to search memory files",
        AppLanguage::SimplifiedChinese => "无法搜索记忆文件",
    }
}

pub(super) fn config_path_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "config_path",
        AppLanguage::SimplifiedChinese => "配置路径",
    }
}

pub(super) fn status_error_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "status_error",
        AppLanguage::SimplifiedChinese => "状态错误",
    }
}

pub(super) fn active_output_style_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "active_output_style",
        AppLanguage::SimplifiedChinese => "当前输出风格",
    }
}

pub(super) fn active_persona_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "active_persona",
        AppLanguage::SimplifiedChinese => "当前人设",
    }
}

pub(super) fn active_prompt_fragments_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "active_prompt_fragments",
        AppLanguage::SimplifiedChinese => "当前提示片段",
    }
}

pub(super) fn none_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "(none)",
        AppLanguage::SimplifiedChinese => "（无）",
    }
}

pub(super) fn field_line(language: AppLanguage, label: &str, value: &str) -> String {
    match language {
        AppLanguage::English => format!("{label}: {value}"),
        AppLanguage::SimplifiedChinese => format!("{label}：{value}"),
    }
}
