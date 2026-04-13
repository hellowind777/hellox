use hellox_tui::Card;

use super::AppLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProviderOption {
    OpenAiCompatible,
    Anthropic,
    Exit,
}

impl ProviderOption {
    pub(super) fn from_input(value: &str) -> Option<Self> {
        match value.trim() {
            "1" => Some(Self::OpenAiCompatible),
            "2" => Some(Self::Anthropic),
            "3" => Some(Self::Exit),
            _ => None,
        }
    }

    pub(super) fn config_key(self) -> Option<&'static str> {
        match self {
            Self::OpenAiCompatible => Some("openai"),
            Self::Anthropic => Some("anthropic"),
            Self::Exit => None,
        }
    }

    pub(super) fn step_title(self, language: AppLanguage) -> &'static str {
        match (self, language) {
            (Self::OpenAiCompatible, AppLanguage::English) => "OpenAI-compatible endpoint",
            (Self::OpenAiCompatible, AppLanguage::SimplifiedChinese) => "OpenAI Compatible 接口",
            (Self::Anthropic, AppLanguage::English) => "Anthropic-compatible endpoint",
            (Self::Anthropic, AppLanguage::SimplifiedChinese) => "Anthropic 兼容接口",
            (Self::Exit, AppLanguage::English) => "Exit setup",
            (Self::Exit, AppLanguage::SimplifiedChinese) => "退出引导",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModelPreset {
    OpenAiOpus,
    OpenAiSonnet,
    AnthropicOpus,
    AnthropicSonnet,
    AnthropicHaiku,
}

impl ModelPreset {
    pub(super) fn from_input(provider: ProviderOption, value: &str) -> Option<Self> {
        match (provider, value.trim()) {
            (ProviderOption::OpenAiCompatible, "1") => Some(Self::OpenAiOpus),
            (ProviderOption::OpenAiCompatible, "2") => Some(Self::OpenAiSonnet),
            (ProviderOption::Anthropic, "1") => Some(Self::AnthropicOpus),
            (ProviderOption::Anthropic, "2") => Some(Self::AnthropicSonnet),
            (ProviderOption::Anthropic, "3") => Some(Self::AnthropicHaiku),
            _ => None,
        }
    }

    pub(super) fn profile_name(self) -> &'static str {
        match self {
            Self::OpenAiOpus => "openai_opus",
            Self::OpenAiSonnet => "openai_sonnet",
            Self::AnthropicOpus => "opus",
            Self::AnthropicSonnet => "sonnet",
            Self::AnthropicHaiku => "haiku",
        }
    }

    pub(super) fn display_name(self, language: AppLanguage) -> &'static str {
        match (self, language) {
            (Self::OpenAiOpus, AppLanguage::English) => "OpenAI Opus",
            (Self::OpenAiOpus, AppLanguage::SimplifiedChinese) => "OpenAI Opus（高能力）",
            (Self::OpenAiSonnet, AppLanguage::English) => "OpenAI Sonnet",
            (Self::OpenAiSonnet, AppLanguage::SimplifiedChinese) => "OpenAI Sonnet（平衡）",
            (Self::AnthropicOpus, AppLanguage::English) => "Opus",
            (Self::AnthropicOpus, AppLanguage::SimplifiedChinese) => "Opus（最高能力）",
            (Self::AnthropicSonnet, AppLanguage::English) => "Sonnet",
            (Self::AnthropicSonnet, AppLanguage::SimplifiedChinese) => "Sonnet（平衡）",
            (Self::AnthropicHaiku, AppLanguage::English) => "Haiku",
            (Self::AnthropicHaiku, AppLanguage::SimplifiedChinese) => "Haiku（更快更轻量）",
        }
    }
}

pub(super) fn onboarding_title(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "hellox setup",
        AppLanguage::SimplifiedChinese => "hellox 首次配置",
    }
}

pub(super) fn intro_cards(language: AppLanguage) -> Vec<Card> {
    match language {
        AppLanguage::English => vec![
            Card::new(
                "how it works",
                vec![
                    "Third-party providers connect through the local gateway.".to_string(),
                    "hellox converts every request into Anthropic Messages-compatible payloads."
                        .to_string(),
                    "Cloud-hosted features stay disabled for now; local interfaces remain ready."
                        .to_string(),
                ],
            ),
            Card::new(
                "this setup will configure",
                vec![
                    "provider and default model".to_string(),
                    "compatible base URL".to_string(),
                    "API key stored in the local auth store".to_string(),
                    "first-run recovery entry points like `/doctor`".to_string(),
                ],
            ),
            Card::new(
                "language",
                vec![
                    "Current interface language: English".to_string(),
                    "All onboarding prompts follow your configured or detected system language."
                        .to_string(),
                ],
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            Card::new(
                "本地链路",
                vec![
                    "第三方 provider 会先接入本地 gateway。".to_string(),
                    "hellox 会在本地把请求统一转换成 Anthropic Messages 原生兼容格式。".to_string(),
                    "云端托管能力暂不启用，但本地接口与后续扩展位会继续保留。".to_string(),
                ],
            ),
            Card::new(
                "本次将配置",
                vec![
                    "provider 与默认模型".to_string(),
                    "兼容 Base URL".to_string(),
                    "API Key（保存在本地 auth store，不写入明文 config）".to_string(),
                    "首次使用后的排障入口，如 `/doctor`".to_string(),
                ],
            ),
            Card::new(
                "当前语言",
                vec![
                    "当前界面语言：简体中文".to_string(),
                    "本轮引导的全部提示与说明都会跟随当前语言输出。".to_string(),
                ],
            ),
        ],
    }
}

pub(super) fn provider_cards(language: AppLanguage) -> Vec<Card> {
    match language {
        AppLanguage::English => vec![
            Card::new(
                "provider",
                vec![
                    "1. OpenAI-compatible endpoint (recommended for third-party channels)"
                        .to_string(),
                    "2. Anthropic-compatible endpoint".to_string(),
                    "3. Exit setup".to_string(),
                ],
            ),
            Card::new(
                "recommendation",
                vec![
                    "Choose OpenAI-compatible when your provider offers a standard Chat Completions-compatible channel."
                        .to_string(),
                    "Choose Anthropic-compatible when you already have a native Anthropic-compatible endpoint."
                        .to_string(),
                ],
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            Card::new(
                "provider",
                vec![
                    "1. OpenAI Compatible 接口（推荐，适配第三方渠道）".to_string(),
                    "2. Anthropic 兼容接口".to_string(),
                    "3. 退出引导".to_string(),
                ],
            ),
            Card::new(
                "推荐说明",
                vec![
                    "如果你的供应商提供标准 Chat Completions 兼容渠道，优先选择 OpenAI Compatible。"
                        .to_string(),
                    "如果你已经有原生 Anthropic 兼容入口，再选择 Anthropic 兼容接口。"
                        .to_string(),
                ],
            ),
        ],
    }
}

pub(super) fn provider_prompt(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Choose provider [1/2/3, Enter=1]: ",
        AppLanguage::SimplifiedChinese => "请选择 provider [1/2/3，直接回车=1]：",
    }
}

pub(super) fn provider_invalid(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Enter 1, 2, or 3. Press Enter to accept the recommended option.",
        AppLanguage::SimplifiedChinese => "请输入 1、2 或 3；直接回车可接受推荐选项。",
    }
}

pub(super) fn model_cards(language: AppLanguage, provider: ProviderOption) -> Vec<Card> {
    match (provider, language) {
        (ProviderOption::OpenAiCompatible, AppLanguage::English) => vec![Card::new(
            "default model",
            vec![
                "1. OpenAI Opus — highest capability for complex local coding tasks".to_string(),
                "2. OpenAI Sonnet — balanced speed and quality for everyday work".to_string(),
            ],
        )],
        (ProviderOption::OpenAiCompatible, AppLanguage::SimplifiedChinese) => vec![Card::new(
            "默认模型",
            vec![
                "1. OpenAI Opus —— 更高能力，适合复杂本地编码任务".to_string(),
                "2. OpenAI Sonnet —— 速度与质量更均衡，适合日常工作".to_string(),
            ],
        )],
        (ProviderOption::Anthropic, AppLanguage::English) => vec![Card::new(
            "default model",
            vec![
                "1. Opus — highest capability".to_string(),
                "2. Sonnet — balanced default".to_string(),
                "3. Haiku — fastest lightweight option".to_string(),
            ],
        )],
        (ProviderOption::Anthropic, AppLanguage::SimplifiedChinese) => vec![Card::new(
            "默认模型",
            vec![
                "1. Opus —— 最高能力".to_string(),
                "2. Sonnet —— 默认更均衡".to_string(),
                "3. Haiku —— 更快、更轻量".to_string(),
            ],
        )],
        (ProviderOption::Exit, _) => Vec::new(),
    }
}

pub(super) fn model_prompt(language: AppLanguage, provider: ProviderOption) -> &'static str {
    match (provider, language) {
        (ProviderOption::OpenAiCompatible, AppLanguage::English) => "Choose model [1/2, Enter=1]: ",
        (ProviderOption::OpenAiCompatible, AppLanguage::SimplifiedChinese) => {
            "请选择默认模型 [1/2，直接回车=1]："
        }
        (ProviderOption::Anthropic, AppLanguage::English) => "Choose model [1/2/3, Enter=1]: ",
        (ProviderOption::Anthropic, AppLanguage::SimplifiedChinese) => {
            "请选择默认模型 [1/2/3，直接回车=1]："
        }
        (ProviderOption::Exit, AppLanguage::English) => "Press Enter to continue: ",
        (ProviderOption::Exit, AppLanguage::SimplifiedChinese) => "直接回车继续：",
    }
}

pub(super) fn model_invalid(language: AppLanguage, provider: ProviderOption) -> &'static str {
    match (provider, language) {
        (ProviderOption::OpenAiCompatible, AppLanguage::English) => {
            "Enter 1 or 2. Press Enter to accept the recommended model."
        }
        (ProviderOption::OpenAiCompatible, AppLanguage::SimplifiedChinese) => {
            "请输入 1 或 2；直接回车可接受推荐模型。"
        }
        (ProviderOption::Anthropic, AppLanguage::English) => {
            "Enter 1, 2, or 3. Press Enter to accept the recommended model."
        }
        (ProviderOption::Anthropic, AppLanguage::SimplifiedChinese) => {
            "请输入 1、2 或 3；直接回车可接受推荐模型。"
        }
        (ProviderOption::Exit, AppLanguage::English) => "Press Enter to continue.",
        (ProviderOption::Exit, AppLanguage::SimplifiedChinese) => "直接回车继续。",
    }
}

pub(super) fn endpoint_cards(language: AppLanguage, provider: ProviderOption) -> Vec<Card> {
    match (provider, language) {
        (ProviderOption::OpenAiCompatible, AppLanguage::English) => vec![
            Card::new(
                "endpoint",
                vec![
                    "Enter the OpenAI-compatible base URL for your provider.".to_string(),
                    "Examples: https://api.openai.com/v1, https://openrouter.ai/api/v1".to_string(),
                    "Press Enter to keep the current value shown in the prompt.".to_string(),
                ],
            ),
            Card::new(
                "API key",
                vec![
                    "The key is stored in the local auth store, not written into config.toml."
                        .to_string(),
                    "You can rotate it later with `hellox auth set-key openai`.".to_string(),
                ],
            ),
        ],
        (ProviderOption::OpenAiCompatible, AppLanguage::SimplifiedChinese) => vec![
            Card::new(
                "连接入口",
                vec![
                    "请输入第三方供应商提供的 OpenAI Compatible Base URL。".to_string(),
                    "示例：https://api.openai.com/v1、https://openrouter.ai/api/v1".to_string(),
                    "提示里会显示当前值，直接回车即可保留。".to_string(),
                ],
            ),
            Card::new(
                "API Key",
                vec![
                    "Key 会保存在本地 auth store，不会明文写入 config.toml。".to_string(),
                    "后续可通过 `hellox auth set-key openai` 重新更新。".to_string(),
                ],
            ),
        ],
        (ProviderOption::Anthropic, AppLanguage::English) => vec![
            Card::new(
                "endpoint",
                vec![
                    "Enter the Anthropic-compatible base URL for your provider.".to_string(),
                    "Example: https://api.anthropic.com".to_string(),
                    "Press Enter to keep the current value shown in the prompt.".to_string(),
                ],
            ),
            Card::new(
                "API key",
                vec![
                    "The key is stored in the local auth store, not written into config.toml."
                        .to_string(),
                    "You can rotate it later with `hellox auth set-key anthropic`.".to_string(),
                ],
            ),
        ],
        (ProviderOption::Anthropic, AppLanguage::SimplifiedChinese) => vec![
            Card::new(
                "连接入口",
                vec![
                    "请输入可用的 Anthropic 兼容 Base URL。".to_string(),
                    "示例：https://api.anthropic.com".to_string(),
                    "提示里会显示当前值，直接回车即可保留。".to_string(),
                ],
            ),
            Card::new(
                "API Key",
                vec![
                    "Key 会保存在本地 auth store，不会明文写入 config.toml。".to_string(),
                    "后续可通过 `hellox auth set-key anthropic` 重新更新。".to_string(),
                ],
            ),
        ],
        (ProviderOption::Exit, _) => Vec::new(),
    }
}

pub(super) fn endpoint_prompt(
    language: AppLanguage,
    provider: ProviderOption,
    current: &str,
) -> String {
    match (provider, language) {
        (ProviderOption::OpenAiCompatible, AppLanguage::English) => {
            format!("Base URL (current: {current}, Enter keeps it): ")
        }
        (ProviderOption::OpenAiCompatible, AppLanguage::SimplifiedChinese) => {
            format!("Base URL（当前：{current}，直接回车保留）：")
        }
        (ProviderOption::Anthropic, AppLanguage::English) => {
            format!("Base URL (current: {current}, Enter keeps it): ")
        }
        (ProviderOption::Anthropic, AppLanguage::SimplifiedChinese) => {
            format!("Base URL（当前：{current}，直接回车保留）：")
        }
        (ProviderOption::Exit, AppLanguage::English) => "Press Enter to continue: ".to_string(),
        (ProviderOption::Exit, AppLanguage::SimplifiedChinese) => "直接回车继续：".to_string(),
    }
}

pub(super) fn api_key_prompt(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "API key (stored locally): ",
        AppLanguage::SimplifiedChinese => "API Key（仅保存在本地）：",
    }
}

pub(super) fn required_value_invalid(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "This field cannot be empty.",
        AppLanguage::SimplifiedChinese => "此项不能为空。",
    }
}

pub(super) fn review_cards(
    language: AppLanguage,
    provider: ProviderOption,
    model: ModelPreset,
    base_url: &str,
) -> Vec<Card> {
    let provider_label = provider.step_title(language);
    let model_label = model.display_name(language);
    match language {
        AppLanguage::English => vec![
            Card::new(
                "review",
                vec![
                    format!("provider: {provider_label}"),
                    format!("default model: {model_label}"),
                    format!("base URL: {base_url}"),
                    "gateway: local gateway auto-starts when needed".to_string(),
                ],
            ),
            Card::new(
                "security notes",
                vec![
                    "AI can make mistakes. Review generated changes before you trust them."
                        .to_string(),
                    "Only continue in folders that you created or trust.".to_string(),
                    "After saving, startup continues into workspace trust checks when needed."
                        .to_string(),
                ],
            ),
            Card::new(
                "terminal tips",
                vec![
                    "Enter sends your prompt".to_string(),
                    "Shift+Enter inserts a newline".to_string(),
                    "Type `/` then press Tab to browse slash commands".to_string(),
                    "Run `/doctor` if provider or gateway requests fail".to_string(),
                ],
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            Card::new(
                "确认摘要",
                vec![
                    format!("provider：{provider_label}"),
                    format!("默认模型：{model_label}"),
                    format!("Base URL：{base_url}"),
                    "gateway：本地 gateway 会在需要时自动拉起".to_string(),
                ],
            ),
            Card::new(
                "安全说明",
                vec![
                    "AI 可能出错；生成的改动需要你自行复核后再信任。".to_string(),
                    "仅在你创建或明确可信的目录中继续使用。".to_string(),
                    "保存后，若当前工作区尚未信任，启动流程会继续进入目录信任确认。".to_string(),
                ],
            ),
            Card::new(
                "终端提示",
                vec![
                    "Enter 发送当前输入".to_string(),
                    "Shift+Enter 插入换行".to_string(),
                    "输入 `/` 打开斜杠命令菜单，再按 Tab 补全".to_string(),
                    "若 provider 或 gateway 失败，可先运行 `/doctor`".to_string(),
                ],
            ),
        ],
    }
}

pub(super) fn existing_setup_cards(
    language: AppLanguage,
    provider_name: &str,
    provider_kind: &str,
    model: &str,
    base_url: &str,
) -> Vec<Card> {
    match language {
        AppLanguage::English => vec![
            Card::new(
                "existing credentials detected",
                vec![
                    format!("provider: {provider_name} ({provider_kind})"),
                    format!("default model: {model}"),
                    format!("base URL: {base_url}"),
                ],
            ),
            Card::new(
                "next",
                vec![
                    "hellox will mark setup complete and continue startup.".to_string(),
                    "If the workspace is not trusted yet, trust checks will appear next."
                        .to_string(),
                ],
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            Card::new(
                "已检测到现有凭据",
                vec![
                    format!("provider：{provider_name}（{provider_kind}）"),
                    format!("默认模型：{model}"),
                    format!("Base URL：{base_url}"),
                ],
            ),
            Card::new(
                "接下来",
                vec![
                    "hellox 会将首次配置标记为完成，并继续启动流程。".to_string(),
                    "如果当前工作区还未信任，随后会继续出现目录信任确认。".to_string(),
                ],
            ),
        ],
    }
}

pub(super) fn success_cards(
    language: AppLanguage,
    provider: ProviderOption,
    model: ModelPreset,
    base_url: &str,
) -> Vec<Card> {
    let provider_label = provider.step_title(language);
    let model_label = model.display_name(language);
    match language {
        AppLanguage::English => vec![
            Card::new(
                "setup complete",
                vec![
                    format!("provider: {provider_label}"),
                    format!("default model: {model_label}"),
                    format!("base URL: {base_url}"),
                    "gateway: ready to route through local Anthropic Messages-compatible translation"
                        .to_string(),
                ],
            ),
            Card::new(
                "you can change this later",
                vec![
                    "`/model` changes the active model".to_string(),
                    "`hellox auth set-key <provider>` rotates credentials".to_string(),
                    "`/doctor` verifies provider and gateway readiness".to_string(),
                ],
            ),
        ],
        AppLanguage::SimplifiedChinese => vec![
            Card::new(
                "配置完成",
                vec![
                    format!("provider：{provider_label}"),
                    format!("默认模型：{model_label}"),
                    format!("Base URL：{base_url}"),
                    "gateway：已准备好通过本地 Anthropic Messages 兼容转换链路发起请求"
                        .to_string(),
                ],
            ),
            Card::new(
                "后续可调整",
                vec![
                    "`/model` 可切换当前模型".to_string(),
                    "`hellox auth set-key <provider>` 可更新凭据".to_string(),
                    "`/doctor` 可检查 provider 与 gateway 状态".to_string(),
                ],
            ),
        ],
    }
}

pub(super) fn finish_prompt(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Press Enter to save and continue: ",
        AppLanguage::SimplifiedChinese => "按 Enter 保存并继续：",
    }
}

pub(super) fn detected_finish_prompt(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Press Enter to continue into startup: ",
        AppLanguage::SimplifiedChinese => "按 Enter 继续进入启动流程：",
    }
}

pub(super) fn step_label(language: AppLanguage, index: usize, total: usize, title: &str) -> String {
    match language {
        AppLanguage::English => format!("Step {index}/{total} — {title}"),
        AppLanguage::SimplifiedChinese => format!("步骤 {index}/{total} —— {title}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{intro_cards, model_cards, review_cards, step_label, ModelPreset, ProviderOption};
    use crate::startup::AppLanguage;

    #[test]
    fn chinese_intro_cards_explain_local_gateway_path() {
        let cards = intro_cards(AppLanguage::SimplifiedChinese);
        let flattened = cards
            .into_iter()
            .flat_map(|card| card.lines.into_iter())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(flattened.contains("本地 gateway"));
        assert!(flattened.contains("Anthropic Messages"));
    }

    #[test]
    fn provider_specific_model_cards_are_localized() {
        let cards = model_cards(
            AppLanguage::SimplifiedChinese,
            ProviderOption::OpenAiCompatible,
        );
        let flattened = cards
            .into_iter()
            .flat_map(|card| card.lines.into_iter())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(flattened.contains("OpenAI Opus"));
        assert!(flattened.contains("均衡"));
    }

    #[test]
    fn review_cards_include_trust_handoff_message() {
        let cards = review_cards(
            AppLanguage::SimplifiedChinese,
            ProviderOption::OpenAiCompatible,
            ModelPreset::OpenAiOpus,
            "https://openrouter.ai/api/v1",
        );
        let flattened = cards
            .into_iter()
            .flat_map(|card| card.lines.into_iter())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(flattened.contains("目录信任确认"));
    }

    #[test]
    fn step_label_uses_expected_format() {
        assert_eq!(
            step_label(AppLanguage::English, 2, 4, "Model"),
            "Step 2/4 — Model"
        );
        assert_eq!(
            step_label(AppLanguage::SimplifiedChinese, 2, 4, "默认模型"),
            "步骤 2/4 —— 默认模型"
        );
    }
}
