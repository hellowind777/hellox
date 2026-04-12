use crate::startup::AppLanguage;

type Replacement = (&'static str, &'static str);

pub(super) fn print_localized_repl_output(language: AppLanguage, text: String) {
    println!("{}", localize_repl_output(language, text));
}

pub(super) fn localized_invalid_selection_text(
    language: AppLanguage,
    upper_bound: usize,
    rerun: &str,
) -> String {
    match language {
        AppLanguage::English => {
            format!("Invalid selection. Choose 1..{upper_bound} or re-run `{rerun}`.")
        }
        AppLanguage::SimplifiedChinese => {
            format!("选择无效。请选择 1..{upper_bound}，或重新运行 `{rerun}`。")
        }
    }
}

pub(super) fn localize_repl_output(language: AppLanguage, text: String) -> String {
    if matches!(language, AppLanguage::English) {
        return text;
    }

    normalize_localized_spacing(
        [
            HEADING_REPLACEMENTS,
            ERROR_REPLACEMENTS,
            EMPTY_STATE_REPLACEMENTS,
            WORKFLOW_REPLACEMENTS,
            MCP_REPLACEMENTS,
            STYLE_REPLACEMENTS,
            TASK_REPLACEMENTS,
            PLUGIN_REMOTE_REPLACEMENTS,
            LABEL_REPLACEMENTS,
            HELP_REPLACEMENTS,
        ]
        .into_iter()
        .fold(text, apply_replacements),
    )
}

fn apply_replacements(text: String, replacements: &[Replacement]) -> String {
    replacements
        .iter()
        .fold(text, |acc, (from, to)| acc.replace(from, to))
}

fn normalize_localized_spacing(text: String) -> String {
    apply_replacements(text, &[("用法： ", "用法："), ("。.", "。")])
}

const HEADING_REPLACEMENTS: &[Replacement] = &[
    (
        "Usage (user-managed remote capability):",
        "用法（用户管理的远程能力）：",
    ),
    ("Usage:", "用法："),
    ("Action palette", "操作面板"),
    ("REPL palette", "REPL 面板"),
    ("MCP server panel:", "MCP 服务器面板："),
    ("MCP panel", "MCP 面板"),
    ("Servers", "服务器"),
    ("Transport", "传输"),
    ("OAuth", "OAuth"),
];

const ERROR_REPLACEMENTS: &[Replacement] = &[
    (
        "Unable to inspect persisted sessions",
        "无法检查已持久化会话",
    ),
    (
        "Unable to inspect workspace health",
        "无法检查工作区健康状态",
    ),
    ("Unable to inspect workspace usage", "无法检查工作区用量"),
    ("Unable to inspect workspace stats", "无法检查工作区统计"),
    (
        "Unable to load config for cost inspection",
        "无法为成本检查加载配置",
    ),
    ("Unable to inspect cost state", "无法检查成本状态"),
    (
        "Unable to search persisted sessions",
        "无法搜索已持久化会话",
    ),
    ("Unable to search memory files", "无法搜索记忆文件"),
    ("Unable to render bridge panel", "无法渲染 bridge 面板"),
    ("Unable to render memory panel", "无法渲染记忆面板"),
    ("Unable to render model panel", "无法渲染模型面板"),
    (
        "Unable to render output-style panel",
        "无法渲染输出风格面板",
    ),
    (
        "Unable to render output style panel",
        "无法渲染输出风格面板",
    ),
    ("Unable to render persona panel", "无法渲染人设面板"),
    (
        "Unable to render prompt fragment panel",
        "无法渲染提示片段面板",
    ),
    ("Unable to render session panel", "无法渲染会话面板"),
    ("Unable to render task panel", "无法渲染任务面板"),
    ("Unable to inspect output styles", "无法检查输出风格"),
    ("Unable to inspect personas", "无法检查人设"),
    ("Unable to inspect prompt fragments", "无法检查提示片段"),
    ("Unable to inspect tasks", "无法检查任务"),
    ("Unable to load output style", "无法加载输出风格"),
    ("Unable to load persona", "无法加载人设"),
    ("Unable to load prompt fragment", "无法加载提示片段"),
    ("Unable to load prompt fragments", "无法加载提示片段"),
    ("Unable to load memory", "无法加载记忆"),
    ("Unable to load", "无法加载"),
    ("Unable to share", "无法导出"),
    ("was not found", "未找到"),
    ("does not have OAuth configured", "没有配置 OAuth"),
    ("does not have a refresh token", "没有刷新令牌"),
    ("is disabled.", "已禁用。"),
    ("cannot be empty", "不能为空"),
    ("must be valid JSON.", "必须是有效 JSON。"),
    ("must be a JSON object.", "必须是 JSON 对象。"),
    ("JSON object", "JSON 对象"),
    ("Invalid MCP", "无效的 MCP"),
    ("Invalid selection.", "选择无效。"),
];

const EMPTY_STATE_REPLACEMENTS: &[Replacement] = &[
    ("No tasks found.", "未找到任务。"),
    ("No memory files found.", "未找到记忆文件。"),
    ("No output styles found.", "未找到输出风格。"),
    ("No definitions found.", "未找到定义。"),
    ("No plugins installed.", "尚未安装插件。"),
    ("No plugin marketplaces configured.", "尚未配置插件市场。"),
    ("No remote environments configured.", "尚未配置远程环境。"),
    ("No remote sessions available.", "没有可用的远程会话。"),
    ("No model profiles configured.", "尚未配置模型档案。"),
    (
        "No persisted bridge sessions found.",
        "未找到已持久化的 bridge 会话。",
    ),
    ("No persisted sessions found.", "未找到已持久化会话。"),
    (
        "Start a session with persistence enabled first.",
        "请先启动启用持久化的会话。",
    ),
    (
        "No project workflow scripts found.",
        "未找到项目工作流脚本。",
    ),
    (
        "No workflow scripts found under",
        "未在以下路径找到工作流脚本",
    ),
    (
        "Create one with `hellox workflow init <name>` or `/workflow init <name>`.",
        "可使用 `hellox workflow init <name>` 或 `/workflow init <name>` 创建。",
    ),
];

const WORKFLOW_REPLACEMENTS: &[Replacement] = &[
    ("Closed workflow dashboard.", "已关闭工作流仪表板。"),
    ("Initialized workflow", "已初始化工作流"),
    ("Added workflow step", "已添加工作流步骤"),
    ("Updated workflow step", "已更新工作流步骤"),
    ("Duplicated workflow step", "已复制工作流步骤"),
    ("Moved workflow step", "已移动工作流步骤"),
    ("Removed workflow step", "已移除工作流步骤"),
    ("Updated shared_context.", "已更新 shared_context。"),
    ("Cleared shared_context.", "已清除 shared_context。"),
    ("Enabled continue_on_error.", "已启用 continue_on_error。"),
    ("Disabled continue_on_error.", "已禁用 continue_on_error。"),
    ("workflow run `", "工作流运行 `"),
    ("workflow `", "工作流 `"),
    ("has no steps to focus yet.", "还没有可聚焦的步骤。"),
    ("has no recorded steps.", "没有已记录的步骤。"),
    ("Focused workflow step", "已聚焦工作流步骤"),
    ("Focused recorded step", "已聚焦已记录步骤"),
    ("Already on the first workflow step", "已在第一个工作流步骤"),
    (
        "Already on the last workflow step",
        "已在最后一个工作流步骤",
    ),
    ("Already on the first recorded step", "已在第一个已记录步骤"),
    (
        "Already on the last recorded step",
        "已在最后一个已记录步骤",
    ),
    (" into step ", " 复制为步骤 "),
    (" to step ", " 到步骤 "),
    (" of ", " / "),
    (" at `", "，位置 `"),
    (
        "choose either `<name>` or `--script-path <path>` for",
        "请为以下命令在 `<name>` 与 `--script-path <path>` 中选择一个：",
    ),
];

const MCP_REPLACEMENTS: &[Replacement] = &[
    ("No MCP servers configured.", "尚未配置 MCP 服务器。"),
    ("Stored MCP bearer token for", "已保存 MCP bearer token："),
    ("Cleared MCP bearer token for", "已清除 MCP bearer token："),
    (
        "No stored MCP bearer token found for",
        "未找到已保存的 MCP bearer token：",
    ),
    ("Configured MCP OAuth for", "已配置 MCP OAuth："),
    ("Refreshed MCP OAuth account", "已刷新 MCP OAuth 账号"),
    (
        "Cleared linked MCP OAuth account for",
        "已清除已关联的 MCP OAuth 账号：",
    ),
    (
        "OAuth client config remains in",
        "OAuth 客户端配置仍保留在",
    ),
    (
        "No linked MCP OAuth account found for",
        "未找到已关联的 MCP OAuth 账号：",
    ),
    ("Installed MCP registry server", "已安装 MCP 注册表服务器"),
    ("Added MCP server", "已添加 MCP 服务器"),
    ("Enabled MCP server", "已启用 MCP 服务器"),
    ("Disabled MCP server", "已禁用 MCP 服务器"),
    ("Removed MCP server", "已移除 MCP 服务器"),
    ("MCP server `", "MCP 服务器 `"),
    ("Stored MCP OAuth account `", "已保存 MCP OAuth 账号 `"),
    ("MCP OAuth account `", "MCP OAuth 账号 `"),
    (
        "MCP OAuth is only supported for HTTP/SSE or WebSocket servers.",
        "MCP OAuth 仅支持 HTTP/SSE 或 WebSocket 服务器。",
    ),
    (
        "MCP bearer-token helper only supports HTTP/SSE or WebSocket servers configured with `transport = \"sse\"` or `transport = \"ws\"`.",
        "MCP bearer-token 助手仅支持配置为 `transport = \"sse\"` 或 `transport = \"ws\"` 的 HTTP/SSE 或 WebSocket 服务器。",
    ),
    ("Unknown MCP error", "未知 MCP 错误"),
    ("MCP error", "MCP 错误"),
    ("server:", "服务器："),
    ("authorization_url:", "授权 URL："),
    ("code_verifier:", "code_verifier："),
    ("state:", "state："),
    ("tool:", "工具："),
    ("prompt:", "提示："),
    ("result:", "结果："),
    ("tools:", "工具："),
    ("resources:", "资源："),
    ("prompts:", "提示："),
    ("uri:", "URI："),
    ("content_uri:", "内容 URI："),
    ("mime_type:", "MIME 类型："),
    ("text:", "文本："),
    ("blob_base64_length:", "blob base64 长度："),
    (
        "name\tenabled\ttransport\tscope\tdescription",
        "名称\t启用\t传输\t范围\t描述",
    ),
];

const STYLE_REPLACEMENTS: &[Replacement] = &[
    ("Active output style set to", "当前会话输出风格已设置为"),
    ("Cleared active output style", "已清除当前输出风格"),
    (
        "No active output style is set for the current session.",
        "当前会话没有设置输出风格。",
    ),
    ("Active persona set to", "当前会话人设已设置为"),
    ("Cleared active persona", "已清除当前人设"),
    (
        "No active persona is set for the current session.",
        "当前会话没有设置人设。",
    ),
    ("Active prompt fragments set to", "当前会话提示片段已设置为"),
    ("Cleared active prompt fragments", "已清除当前提示片段"),
    (
        "No active prompt fragments are set for the current session.",
        "当前会话没有设置提示片段。",
    ),
    ("for the current session", "（当前会话）"),
];

const TASK_REPLACEMENTS: &[Replacement] = &[
    ("Added task", "已添加任务"),
    ("Updated task", "已更新任务"),
    ("Stopped task", "已停止任务"),
    ("Marked task", "已将任务"),
    ("Removed task", "已移除任务"),
    (" task(s).", " 个任务。"),
    (" as ", " 标记为 "),
    ("task_id:", "任务 ID："),
    ("status:", "状态："),
    ("priority:", "优先级："),
    ("content:", "内容："),
    ("description:", "描述："),
    ("output:", "输出："),
];

const PLUGIN_REMOTE_REPLACEMENTS: &[Replacement] = &[
    ("Installed plugin", "已安装插件"),
    ("Enabled plugin marketplace", "已启用插件市场"),
    ("Disabled plugin marketplace", "已禁用插件市场"),
    ("Removed plugin marketplace", "已移除插件市场"),
    ("Added plugin marketplace", "已添加插件市场"),
    ("Enabled plugin", "已启用插件"),
    ("Disabled plugin", "已禁用插件"),
    ("Removed plugin", "已移除插件"),
    ("Added remote environment", "已添加远程环境"),
    ("Enabled remote environment", "已启用远程环境"),
    ("Disabled remote environment", "已禁用远程环境"),
    ("Removed remote environment", "已移除远程环境"),
    ("Remote environment `", "远程环境 `"),
    (
        "Open the local assistant viewer panel",
        "打开本地 assistant 查看面板",
    ),
    (
        "Show the local/remote assistant session viewer",
        "显示本地/远程 assistant 会话查看器",
    ),
    (
        "Inspect one assistant-viewable session",
        "查看一个可由 assistant 查看器读取的会话",
    ),
];

const LABEL_REPLACEMENTS: &[Replacement] = &[
    ("active_output_style:", "当前输出风格："),
    ("default_output_style:", "默认输出风格："),
    ("active_persona:", "当前人设："),
    ("default_persona:", "默认人设："),
    ("active_prompt_fragments:", "当前提示片段："),
    ("default_prompt_fragments:", "默认提示片段："),
    ("workspace_root:", "工作区根目录："),
    ("provider:", "供应商："),
    ("client_id:", "client_id："),
    ("authorize_url:", "授权 URL："),
    ("token_url:", "令牌 URL："),
    ("redirect_url:", "重定向 URL："),
    ("scopes:", "范围："),
    ("login_hint:", "登录提示："),
    ("account_id:", "账号 ID："),
    ("enabled:", "启用："),
    ("scope:", "范围："),
    ("transport:", "传输："),
    ("oauth:", "OAuth："),
    ("command:", "命令："),
    ("args:", "参数："),
    ("cwd:", "cwd："),
    ("env:", "环境变量："),
    ("headers:", "请求头："),
    ("url:", "URL："),
    ("config_path", "配置路径"),
    ("oauth_configured", "OAuth 已配置"),
    ("YES", "是"),
    ("NO", "否"),
    ("(none)", "（无）"),
    ("(unknown)", "（未知）"),
    ("(unnamed)", "（未命名）"),
    ("(inherit current process)", "（继承当前进程）"),
    ("configured", "已配置"),
];

const HELP_REPLACEMENTS: &[Replacement] = &[
    ("Shared transcript written to", "转录已写入"),
    ("Current model:", "当前模型："),
    ("Model set to", "模型已切换为"),
    ("Current permission mode:", "当前权限模式："),
    ("Available modes:", "可用模式："),
    (
        "Use `/permissions <mode>` to switch.",
        "使用 `/permissions <mode>` 可切换。",
    ),
    (
        "No conversation history to compact.",
        "当前没有可压缩的会话历史。",
    ),
    (
        "No conversation turn to rewind.",
        "当前没有可回退的对话轮次。",
    ),
    (
        "No captured session or project memory found for the current workspace.",
        "当前工作区还没有已捕获的会话或项目记忆。",
    ),
    ("List configured model profiles", "列出已配置模型档案"),
    ("Show the current session model", "显示当前会话模型"),
    (
        "Show a model dashboard or inspect one profile",
        "显示模型面板或查看某个档案",
    ),
    (
        "Show the current or named model profile",
        "显示当前或指定模型档案",
    ),
    ("Switch the current session model", "切换当前会话模型"),
    ("Persist the default model profile", "持久化默认模型档案"),
    (
        "Show active, default, and discovered output styles",
        "显示当前、默认与已发现的输出风格",
    ),
    (
        "Show an output-style dashboard or inspect one style",
        "显示输出风格面板或查看某个风格",
    ),
    ("List discovered output styles", "列出已发现输出风格"),
    ("Show a style prompt", "显示风格提示词"),
    ("Clear the active session style", "清除当前会话输出风格"),
    (
        "Show active, default, and discovered personas",
        "显示当前、默认与已发现的人设",
    ),
    (
        "Show a persona dashboard or inspect one persona",
        "显示人设面板或查看某个人设",
    ),
    ("List discovered personas", "列出已发现人设"),
    ("Show a persona prompt", "显示人设提示词"),
    ("Clear the active session persona", "清除当前会话人设"),
    (
        "Show active, default, and discovered prompt fragments",
        "显示当前、默认与已发现提示片段",
    ),
    (
        "Show a prompt-fragment dashboard or inspect one fragment",
        "显示提示片段面板或查看某个片段",
    ),
    ("List discovered prompt fragments", "列出已发现提示片段"),
    ("Show a prompt fragment", "显示提示片段"),
    (
        "Clear active session prompt fragments",
        "清除当前会话提示片段",
    ),
    ("Add a workspace task", "添加工作区任务"),
    ("Show a single workspace task", "显示单个工作区任务"),
    ("Update task fields", "更新任务字段"),
    ("Show the latest stored task output", "显示任务最近一次输出"),
    (
        "Cancel a task and optionally record a reason",
        "取消任务并可记录原因",
    ),
    ("Mark a task in progress", "将任务标记为进行中"),
    ("Mark a task completed", "将任务标记为已完成"),
    ("Mark a task cancelled", "将任务标记为已取消"),
    ("Delete a task", "删除任务"),
    (
        "Clear `completed` or `all` tasks",
        "清空 `completed` 或 `all` 任务",
    ),
    (
        "Show local bridge runtime status",
        "显示本地 bridge 运行状态",
    ),
    ("Show bridge session details", "显示 bridge 会话详情"),
    (
        "Show IDE-facing bridge status",
        "显示面向 IDE 的 bridge 状态",
    ),
    (
        "Show an IDE-facing bridge overview panel",
        "显示面向 IDE 的 bridge 概览面板",
    ),
    (
        "Show active config path and resolved config",
        "显示当前配置路径与解析后的配置",
    ),
    (
        "Show active config path or supported writable keys",
        "显示当前配置路径或支持写入的键",
    ),
    ("Update a supported config key", "更新一个支持的配置键"),
    (
        "Clear a supported optional config key",
        "清除一个可选配置键",
    ),
    ("Show the current workspace brief", "显示当前工作区 brief"),
    (
        "Store a local brief for the current workspace",
        "为当前工作区保存本地 brief",
    ),
    ("Remove the current workspace brief", "移除当前工作区 brief"),
    (
        "Search available local tools by name or description",
        "按名称或描述搜索本地可用工具",
    ),
    (
        "Apply a style to the current session",
        "将风格应用到当前会话",
    ),
    (
        "Apply a persona to the current session",
        "将人设应用到当前会话",
    ),
    (
        "Apply one or more prompt fragments to the current session",
        "将一个或多个提示片段应用到当前会话",
    ),
    ("Alias for /fragment", "/fragment 的别名"),
    (
        "Show a bridge panel or inspect one persisted session",
        "显示 bridge 面板或查看一个已持久化会话",
    ),
    ("List persisted bridge sessions", "列出已持久化 bridge 会话"),
    ("List configured MCP servers", "列出已配置 MCP 服务器"),
    (
        "Show an MCP dashboard or inspect one server",
        "显示 MCP 面板或查看一个服务器",
    ),
    ("Show a configured MCP server", "显示已配置 MCP 服务器"),
    (
        "List tools exposed by a configured MCP server",
        "列出已配置 MCP 服务器暴露的工具",
    ),
    (
        "List MCP resources exposed by a configured server",
        "列出已配置 MCP 服务器暴露的资源",
    ),
    (
        "List MCP prompts exposed by a configured server",
        "列出已配置 MCP 服务器暴露的提示",
    ),
    ("List installed plugins", "列出已安装插件"),
    (
        "Show a plugin dashboard or inspect one plugin",
        "显示插件面板或查看一个插件",
    ),
    ("Show an installed plugin", "显示已安装插件"),
    ("List configured marketplaces", "列出已配置市场"),
    ("List configured remote environments", "列出已配置远程环境"),
    (
        "Show a remote-environment panel or inspect one target",
        "显示远程环境面板或查看一个目标",
    ),
    ("Add a remote environment profile", "添加远程环境配置"),
    (
        "Open a direct-connect teleport plan panel",
        "打开 direct-connect teleport 计划面板",
    ),
    (
        "Create a remote direct-connect session",
        "创建远程 direct-connect 会话",
    ),
    ("choose either", "请选择其中一个"),
];

#[cfg(test)]
mod tests {
    use crate::startup::AppLanguage;

    use super::{localize_repl_output, localized_invalid_selection_text};

    #[test]
    fn localizes_common_usage_and_error_copy() {
        let text = localize_repl_output(
            AppLanguage::SimplifiedChinese,
            "Usage: /workflow add-step <name> --prompt <text>\nUnable to render task panel: boom"
                .to_string(),
        );

        assert!(text.contains("用法：/workflow add-step"));
        assert!(text.contains("无法渲染任务面板"));
    }

    #[test]
    fn localizes_deep_command_status_copy() {
        let text = localize_repl_output(
            AppLanguage::SimplifiedChinese,
            "Added MCP server `docs` to `config.toml`.\nserver: docs\nresult:\n{}".to_string(),
        );

        assert!(text.contains("已添加 MCP 服务器 `docs`"));
        assert!(text.contains("服务器： docs"));
        assert!(text.contains("结果："));
    }

    #[test]
    fn localizes_selector_errors() {
        assert_eq!(
            localized_invalid_selection_text(AppLanguage::SimplifiedChinese, 3, "/workflow panel"),
            "选择无效。请选择 1..3，或重新运行 `/workflow panel`。"
        );
    }

    #[test]
    fn leaves_english_copy_unchanged() {
        let text = "Usage: /config set <key> <value>".to_string();
        assert_eq!(
            localize_repl_output(AppLanguage::English, text.clone()),
            text
        );
    }
}
