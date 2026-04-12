use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_auth::{get_provider_key, set_provider_key, LocalAuthStoreBackend};
use hellox_config::{save_config, HelloxConfig, ProviderConfig};

use super::AppLanguage;

#[derive(Clone, Debug)]
pub struct ProviderReadiness {
    pub provider_name: String,
    pub provider_kind: &'static str,
    pub has_api_key: bool,
}

#[derive(Debug, Default)]
pub struct OnboardingOutcome {
    pub exit_requested: bool,
    pub model_override: Option<String>,
}

pub fn resolve_provider_readiness(config: &HelloxConfig, model: &str) -> Result<ProviderReadiness> {
    let profiles = hellox_config::materialize_profiles(config);
    let profile = profiles
        .get(model)
        .or_else(|| {
            profiles
                .values()
                .find(|profile| profile.upstream_model == model)
        })
        .ok_or_else(|| anyhow!("unknown model profile `{model}`"))?;
    let provider = config
        .providers
        .get(&profile.provider)
        .ok_or_else(|| anyhow!("unknown provider `{}`", profile.provider))?;
    let auth_store = LocalAuthStoreBackend::default().load_auth_store().ok();

    let (provider_kind, env_var) = match provider {
        ProviderConfig::Anthropic { api_key_env, .. } => ("anthropic", api_key_env.as_str()),
        ProviderConfig::OpenAiCompatible { api_key_env, .. } => {
            ("openai-compatible", api_key_env.as_str())
        }
    };

    let has_api_key = env::var(env_var).is_ok()
        || auth_store
            .as_ref()
            .and_then(|store| get_provider_key(store, &profile.provider))
            .is_some();

    Ok(ProviderReadiness {
        provider_name: profile.provider.clone(),
        provider_kind,
        has_api_key,
    })
}

pub fn run_interactive_provider_onboarding(
    config_path: &Path,
    config: &mut HelloxConfig,
    selected_model: &mut String,
    language: AppLanguage,
) -> Result<OnboardingOutcome> {
    let readiness = resolve_provider_readiness(config, selected_model)?;
    if config.ui.has_completed_onboarding && readiness.has_api_key {
        return Ok(OnboardingOutcome::default());
    }
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return Ok(OnboardingOutcome::default());
    }

    println!();
    for line in onboarding_intro_lines(language) {
        println!("{line}");
    }

    if readiness.has_api_key {
        config.ui.has_completed_onboarding = true;
        save_config(Some(config_path.to_path_buf()), config)?;
        println!("{}", onboarding_existing_config_text(language));
        println!();
        return Ok(OnboardingOutcome::default());
    }

    let selection = prompt_provider_selection(language)?;
    if selection == 3 {
        return Ok(OnboardingOutcome {
            exit_requested: true,
            model_override: None,
        });
    }

    let mut auth_store = LocalAuthStoreBackend::default().load_auth_store()?;
    match selection {
        1 => {
            let base_url = prompt_optional(
                language,
                anthropic_base_url_prompt(language),
                current_anthropic_base_url(config),
            )?;
            let api_key = prompt_required(language, api_key_prompt(language))?;
            config.providers.insert(
                "anthropic".to_string(),
                ProviderConfig::Anthropic {
                    base_url,
                    anthropic_version: current_anthropic_version(config),
                    api_key_env: current_anthropic_env(config),
                },
            );
            config.session.model = "opus".to_string();
            *selected_model = "opus".to_string();
            set_provider_key(&mut auth_store, "anthropic".to_string(), api_key);
        }
        2 => {
            let base_url = prompt_required(
                language,
                &openai_base_url_prompt(language, current_openai_base_url(config)),
            )?;
            let api_key = prompt_required(language, api_key_prompt(language))?;
            config.providers.insert(
                "openai".to_string(),
                ProviderConfig::OpenAiCompatible {
                    base_url,
                    api_key_env: current_openai_env(config),
                },
            );
            config.session.model = "openai_opus".to_string();
            *selected_model = "openai_opus".to_string();
            set_provider_key(&mut auth_store, "openai".to_string(), api_key);
        }
        _ => return Err(anyhow!("unsupported onboarding selection `{selection}`")),
    }

    config.ui.has_completed_onboarding = true;
    save_config(Some(config_path.to_path_buf()), config)?;
    LocalAuthStoreBackend::default().save_auth_store(&auth_store)?;

    println!("{}", onboarding_success_text(language, selected_model));
    println!();
    Ok(OnboardingOutcome {
        exit_requested: false,
        model_override: Some(selected_model.clone()),
    })
}

fn prompt_provider_selection(language: AppLanguage) -> Result<u8> {
    loop {
        for line in provider_selection_lines(language) {
            println!("{line}");
        }
        print!("{}", provider_selection_prompt(language));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim() {
            "1" => return Ok(1),
            "2" => return Ok(2),
            "3" => return Ok(3),
            _ => println!("{}", provider_selection_invalid(language)),
        }
    }
}

fn prompt_required(language: AppLanguage, prompt: &str) -> Result<String> {
    loop {
        print!("{prompt}");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let value = input.trim().to_string();
        if !value.is_empty() {
            return Ok(value);
        }
        println!("{}", required_value_invalid(language));
    }
}

fn prompt_optional(_language: AppLanguage, prompt: &str, fallback: String) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let value = input.trim();
    if value.is_empty() {
        Ok(fallback)
    } else {
        Ok(value.to_string())
    }
}

fn onboarding_intro_lines(language: AppLanguage) -> Vec<&'static str> {
    match language {
        AppLanguage::English => vec![
            "First-time setup:",
            "hellox uses third-party providers through the local gateway and converts requests into Anthropic Messages-compatible calls.",
        ],
        AppLanguage::SimplifiedChinese => vec![
            "首次配置：",
            "hellox 会通过本地 gateway 接入第三方 provider，并统一转换成 Anthropic Messages 兼容请求。",
        ],
    }
}

fn onboarding_existing_config_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => {
            "Existing provider credentials were detected. Setup is marked as complete."
        }
        AppLanguage::SimplifiedChinese => "已检测到现有 provider 凭据，首次引导已标记完成。",
    }
}

fn provider_selection_lines(language: AppLanguage) -> Vec<&'static str> {
    match language {
        AppLanguage::English => vec![
            "Choose an API provider:",
            "  1. Anthropic-compatible endpoint",
            "  2. OpenAI-compatible endpoint (recommended for third-party channels)",
            "  3. Exit",
        ],
        AppLanguage::SimplifiedChinese => vec![
            "请选择 API provider：",
            "  1. Anthropic 兼容接口",
            "  2. OpenAI Compatible 接口（推荐，适配第三方渠道）",
            "  3. 退出",
        ],
    }
}

fn provider_selection_prompt(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Select [1/2/3]: ",
        AppLanguage::SimplifiedChinese => "请选择 [1/2/3]：",
    }
}

fn provider_selection_invalid(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Please enter 1, 2, or 3.",
        AppLanguage::SimplifiedChinese => "请输入 1、2 或 3。",
    }
}

fn required_value_invalid(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "This value cannot be empty.",
        AppLanguage::SimplifiedChinese => "此项不能为空。",
    }
}

fn api_key_prompt(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "API key: ",
        AppLanguage::SimplifiedChinese => "API Key：",
    }
}

fn anthropic_base_url_prompt(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Anthropic base URL (press Enter to keep current): ",
        AppLanguage::SimplifiedChinese => "Anthropic Base URL（直接回车保持当前值）：",
    }
}

fn openai_base_url_prompt(language: AppLanguage, current: String) -> String {
    match language {
        AppLanguage::English => {
            format!("OpenAI-compatible base URL (current: {current}): ")
        }
        AppLanguage::SimplifiedChinese => {
            format!("OpenAI Compatible Base URL（当前：{current}）：")
        }
    }
}

fn onboarding_success_text(language: AppLanguage, model: &str) -> String {
    match language {
        AppLanguage::English => {
            format!("Provider setup completed. Default session model is now `{model}`.")
        }
        AppLanguage::SimplifiedChinese => {
            format!("Provider 配置完成，默认会话模型已切换为 `{model}`。")
        }
    }
}

fn current_anthropic_base_url(config: &HelloxConfig) -> String {
    match config.providers.get("anthropic") {
        Some(ProviderConfig::Anthropic { base_url, .. }) => base_url.clone(),
        _ => "https://api.anthropic.com".to_string(),
    }
}

fn current_anthropic_version(config: &HelloxConfig) -> String {
    match config.providers.get("anthropic") {
        Some(ProviderConfig::Anthropic {
            anthropic_version, ..
        }) => anthropic_version.clone(),
        _ => "2023-06-01".to_string(),
    }
}

fn current_anthropic_env(config: &HelloxConfig) -> String {
    match config.providers.get("anthropic") {
        Some(ProviderConfig::Anthropic { api_key_env, .. }) => api_key_env.clone(),
        _ => "ANTHROPIC_API_KEY".to_string(),
    }
}

fn current_openai_base_url(config: &HelloxConfig) -> String {
    match config.providers.get("openai") {
        Some(ProviderConfig::OpenAiCompatible { base_url, .. }) => base_url.clone(),
        _ => "https://api.openai.com/v1".to_string(),
    }
}

fn current_openai_env(config: &HelloxConfig) -> String {
    match config.providers.get("openai") {
        Some(ProviderConfig::OpenAiCompatible { api_key_env, .. }) => api_key_env.clone(),
        _ => "OPENAI_API_KEY".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use hellox_config::HelloxConfig;
    use std::env;

    use super::resolve_provider_readiness;

    #[test]
    fn provider_readiness_detects_env_keys() {
        let original = env::var_os("OPENAI_API_KEY");
        env::set_var("OPENAI_API_KEY", "sk-openai");
        let mut config = HelloxConfig::default();
        config.session.model = "openai_opus".to_string();

        let readiness = resolve_provider_readiness(&config, "openai_opus").expect("readiness");
        assert_eq!(readiness.provider_name, "openai");
        assert_eq!(readiness.provider_kind, "openai-compatible");
        assert!(readiness.has_api_key);

        if let Some(value) = original {
            env::set_var("OPENAI_API_KEY", value);
        } else {
            env::remove_var("OPENAI_API_KEY");
        }
    }
}
