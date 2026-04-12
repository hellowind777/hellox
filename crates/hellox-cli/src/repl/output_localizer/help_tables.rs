use super::Replacement;

pub(super) const HELP_REPLACEMENTS: &[Replacement] = &[
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
    ("Update a supported config key", "更新一个支持写入的键"),
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
