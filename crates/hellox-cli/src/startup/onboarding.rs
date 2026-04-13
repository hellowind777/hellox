use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_auth::{get_provider_key, set_provider_key, LocalAuthStoreBackend};
use hellox_config::{save_config, HelloxConfig, ProviderConfig};
use hellox_tui::{render_cards, Card};

use crate::welcome_v2::welcome_v2_lines;

use super::interactive_select::{select_interactive, InteractiveOption};
use super::onboarding_copy::{
    api_key_prompt, choice_exit_pending_text, detected_finish_prompt, endpoint_cards,
    endpoint_prompt, existing_setup_cards, finish_prompt, interactive_choice_fallback_notice,
    intro_cards, model_cards, model_footer, model_invalid, model_prompt, provider_cards,
    provider_footer, provider_invalid, provider_prompt, required_value_invalid, review_cards,
    step_label, success_cards, ModelPreset, ProviderOption,
};
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
    resolve_provider_readiness_with_backend(config, model, &LocalAuthStoreBackend::default())
}

pub fn resolve_provider_readiness_for_config_path(
    config: &HelloxConfig,
    model: &str,
    config_path: &Path,
) -> Result<ProviderReadiness> {
    let auth_backend = LocalAuthStoreBackend::from_config_path(config_path);
    resolve_provider_readiness_with_backend(config, model, &auth_backend)
}

fn resolve_provider_readiness_with_backend(
    config: &HelloxConfig,
    model: &str,
    auth_backend: &LocalAuthStoreBackend,
) -> Result<ProviderReadiness> {
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
    let auth_store = auth_backend.load_auth_store().ok();

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
    let readiness =
        resolve_provider_readiness_for_config_path(config, selected_model, config_path)?;
    if config.ui.has_completed_onboarding && readiness.has_api_key {
        return Ok(OnboardingOutcome::default());
    }
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return Ok(OnboardingOutcome::default());
    }

    print_lines(&welcome_v2_lines(language));
    println!();
    print_cards(&intro_cards(language));

    if readiness.has_api_key {
        handle_existing_setup(config_path, config, selected_model, language, &readiness)?;
        return Ok(OnboardingOutcome::default());
    }

    let (provider, model) = loop {
        let provider = select_provider(language)?;
        if provider == ProviderOption::Exit {
            return Ok(OnboardingOutcome {
                exit_requested: true,
                model_override: None,
            });
        }

        if let Some(model) = select_model(language, provider)? {
            break (provider, model);
        }
    };
    let base_url = collect_provider_connection(language, provider, config)?;
    let api_key = prompt_required(language, api_key_prompt(language))?;

    println!();
    println!(
        "{}",
        step_label(
            language,
            4,
            4,
            match language {
                AppLanguage::English => "Review and continue",
                AppLanguage::SimplifiedChinese => "确认并继续",
            }
        )
    );
    print_cards(&review_cards(language, provider, model, &base_url));
    prompt_enter(finish_prompt(language))?;

    persist_onboarding_selection(
        config_path,
        config,
        selected_model,
        provider,
        model,
        base_url,
        api_key,
    )?;

    println!();
    print_cards(&success_cards(
        language,
        provider,
        model,
        &current_provider_base_url(
            config,
            provider
                .config_key()
                .ok_or_else(|| anyhow!("provider selection missing config key"))?,
        ),
    ));
    println!();
    Ok(OnboardingOutcome {
        exit_requested: false,
        model_override: Some(selected_model.clone()),
    })
}

fn handle_existing_setup(
    config_path: &Path,
    config: &mut HelloxConfig,
    selected_model: &str,
    language: AppLanguage,
    readiness: &ProviderReadiness,
) -> Result<()> {
    let base_url = current_provider_base_url(config, &readiness.provider_name);
    println!();
    println!(
        "{}",
        step_label(
            language,
            1,
            1,
            match language {
                AppLanguage::English => "Detected setup",
                AppLanguage::SimplifiedChinese => "检测到现有配置",
            }
        )
    );
    print_cards(&existing_setup_cards(
        language,
        &readiness.provider_name,
        readiness.provider_kind,
        selected_model,
        &base_url,
    ));
    prompt_enter(detected_finish_prompt(language))?;
    config.ui.has_completed_onboarding = true;
    save_config(Some(config_path.to_path_buf()), config)?;
    println!();
    Ok(())
}

fn select_provider(language: AppLanguage) -> Result<ProviderOption> {
    println!();
    println!(
        "{}",
        step_label(
            language,
            1,
            4,
            match language {
                AppLanguage::English => "Provider",
                AppLanguage::SimplifiedChinese => "选择 provider",
            }
        )
    );
    print_cards(&provider_cards(language));
    match select_interactive(
        &provider_options(language),
        0,
        provider_footer(language),
        choice_exit_pending_text(language),
    ) {
        Ok(Some(choice)) => Ok(choice),
        Ok(None) => Ok(ProviderOption::Exit),
        Err(error) => {
            println!();
            println!("{}", interactive_choice_fallback_notice(language));
            println!("{error}");
            prompt_choice(
                provider_prompt(language),
                ProviderOption::OpenAiCompatible,
                ProviderOption::from_input,
                provider_invalid(language),
            )
        }
    }
}

fn select_model(language: AppLanguage, provider: ProviderOption) -> Result<Option<ModelPreset>> {
    println!();
    println!(
        "{}",
        step_label(
            language,
            2,
            4,
            match language {
                AppLanguage::English => "Default model",
                AppLanguage::SimplifiedChinese => "选择默认模型",
            }
        )
    );
    print_cards(&model_cards(language, provider));
    let default = match provider {
        ProviderOption::OpenAiCompatible => ModelPreset::OpenAiOpus,
        ProviderOption::Anthropic => ModelPreset::AnthropicOpus,
        ProviderOption::Exit => return Ok(None),
    };
    match select_interactive(
        &model_options(language, provider),
        0,
        model_footer(language),
        choice_exit_pending_text(language),
    ) {
        Ok(Some(choice)) => Ok(Some(choice)),
        Ok(None) => Ok(None),
        Err(error) => {
            println!();
            println!("{}", interactive_choice_fallback_notice(language));
            println!("{error}");
            prompt_choice(
                model_prompt(language, provider),
                default,
                |value| ModelPreset::from_input(provider, value),
                model_invalid(language, provider),
            )
            .map(Some)
        }
    }
}

fn collect_provider_connection(
    language: AppLanguage,
    provider: ProviderOption,
    config: &HelloxConfig,
) -> Result<String> {
    println!();
    println!(
        "{}",
        step_label(
            language,
            3,
            4,
            match language {
                AppLanguage::English => "Endpoint and API key",
                AppLanguage::SimplifiedChinese => "配置连接入口与 API Key",
            }
        )
    );
    print_cards(&endpoint_cards(language, provider));
    let provider_name = provider
        .config_key()
        .ok_or_else(|| anyhow!("provider selection does not map to config key"))?;
    let current = current_provider_base_url(config, provider_name);
    prompt_optional(&endpoint_prompt(language, provider, &current), current)
}

fn persist_onboarding_selection(
    config_path: &Path,
    config: &mut HelloxConfig,
    selected_model: &mut String,
    provider: ProviderOption,
    model: ModelPreset,
    base_url: String,
    api_key: String,
) -> Result<()> {
    let auth_backend = LocalAuthStoreBackend::from_config_path(config_path);
    let mut auth_store = auth_backend.load_auth_store()?;
    match provider {
        ProviderOption::Anthropic => {
            config.providers.insert(
                "anthropic".to_string(),
                ProviderConfig::Anthropic {
                    base_url,
                    anthropic_version: current_anthropic_version(config),
                    api_key_env: current_anthropic_env(config),
                },
            );
            set_provider_key(&mut auth_store, "anthropic".to_string(), api_key);
        }
        ProviderOption::OpenAiCompatible => {
            config.providers.insert(
                "openai".to_string(),
                ProviderConfig::OpenAiCompatible {
                    base_url,
                    api_key_env: current_openai_env(config),
                },
            );
            set_provider_key(&mut auth_store, "openai".to_string(), api_key);
        }
        ProviderOption::Exit => return Err(anyhow!("cannot persist exit provider selection")),
    }

    config.session.model = model.profile_name().to_string();
    *selected_model = model.profile_name().to_string();
    config.ui.has_completed_onboarding = true;
    save_config(Some(config_path.to_path_buf()), config)?;
    auth_backend.save_auth_store(&auth_store)?;
    Ok(())
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

fn prompt_optional(prompt: &str, fallback: String) -> Result<String> {
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

fn prompt_choice<T, F>(prompt: &str, default: T, parser: F, invalid_text: &str) -> Result<T>
where
    T: Copy,
    F: Fn(&str) -> Option<T>,
{
    loop {
        print!("{prompt}");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default);
        }
        if let Some(choice) = parser(trimmed) {
            return Ok(choice);
        }
        println!("{invalid_text}");
    }
}

fn prompt_enter(prompt: &str) -> Result<()> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(())
}

fn print_cards(cards: &[Card]) {
    for line in render_cards(cards) {
        println!("{line}");
    }
}

fn print_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

fn provider_options(language: AppLanguage) -> Vec<InteractiveOption<ProviderOption>> {
    vec![
        InteractiveOption {
            label: match language {
                AppLanguage::English => "OpenAI-compatible endpoint",
                AppLanguage::SimplifiedChinese => "OpenAI Compatible 接口",
            }
            .to_string(),
            value: ProviderOption::OpenAiCompatible,
        },
        InteractiveOption {
            label: match language {
                AppLanguage::English => "Anthropic-compatible endpoint",
                AppLanguage::SimplifiedChinese => "Anthropic 兼容接口",
            }
            .to_string(),
            value: ProviderOption::Anthropic,
        },
        InteractiveOption {
            label: match language {
                AppLanguage::English => "Exit setup",
                AppLanguage::SimplifiedChinese => "退出引导",
            }
            .to_string(),
            value: ProviderOption::Exit,
        },
    ]
}

fn model_options(
    language: AppLanguage,
    provider: ProviderOption,
) -> Vec<InteractiveOption<ModelPreset>> {
    match provider {
        ProviderOption::OpenAiCompatible => vec![
            InteractiveOption {
                label: ModelPreset::OpenAiOpus.display_name(language).to_string(),
                value: ModelPreset::OpenAiOpus,
            },
            InteractiveOption {
                label: ModelPreset::OpenAiSonnet.display_name(language).to_string(),
                value: ModelPreset::OpenAiSonnet,
            },
        ],
        ProviderOption::Anthropic => vec![
            InteractiveOption {
                label: ModelPreset::AnthropicOpus
                    .display_name(language)
                    .to_string(),
                value: ModelPreset::AnthropicOpus,
            },
            InteractiveOption {
                label: ModelPreset::AnthropicSonnet
                    .display_name(language)
                    .to_string(),
                value: ModelPreset::AnthropicSonnet,
            },
            InteractiveOption {
                label: ModelPreset::AnthropicHaiku
                    .display_name(language)
                    .to_string(),
                value: ModelPreset::AnthropicHaiku,
            },
        ],
        ProviderOption::Exit => Vec::new(),
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

fn current_provider_base_url(config: &HelloxConfig, provider_name: &str) -> String {
    match provider_name {
        "anthropic" => current_anthropic_base_url(config),
        "openai" => current_openai_base_url(config),
        _ => current_openai_base_url(config),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_auth::{get_provider_key, LocalAuthStoreBackend};
    use hellox_config::HelloxConfig;
    use std::env;

    use super::{
        current_provider_base_url, persist_onboarding_selection, resolve_provider_readiness,
        resolve_provider_readiness_for_config_path,
    };
    use crate::startup::onboarding_copy::{ModelPreset, ProviderOption};

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-onboarding-tests-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

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

    #[test]
    fn config_scoped_readiness_does_not_reuse_global_provider_keys() {
        let scoped_root = temp_root();
        let other_root = temp_root();
        let config_path = scoped_root.join(".hellox").join("config.toml");
        let other_config_path = other_root.join(".hellox").join("config.toml");

        let mut other_store = LocalAuthStoreBackend::from_config_path(&other_config_path)
            .load_auth_store()
            .expect("load other auth store");
        hellox_auth::set_provider_key(
            &mut other_store,
            "openai".to_string(),
            "sk-other".to_string(),
        );
        LocalAuthStoreBackend::from_config_path(&other_config_path)
            .save_auth_store(&other_store)
            .expect("save other auth store");

        let mut config = HelloxConfig::default();
        config.session.model = "openai_opus".to_string();
        let readiness =
            resolve_provider_readiness_for_config_path(&config, "openai_opus", &config_path)
                .expect("config-scoped readiness");

        assert!(!readiness.has_api_key);
    }

    #[test]
    fn current_provider_base_url_matches_provider_name() {
        let config = HelloxConfig::default();
        assert_eq!(
            current_provider_base_url(&config, "openai"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            current_provider_base_url(&config, "anthropic"),
            "https://api.anthropic.com"
        );
    }

    #[test]
    fn persist_onboarding_selection_writes_config_and_auth_store() {
        let _guard = super::super::test_support::env_lock();
        let global_home = temp_root();
        let isolated_root = temp_root();
        let config_path = isolated_root.join(".hellox").join("config.toml");
        let original_home = env::var_os("HOME");
        let original_user_profile = env::var_os("USERPROFILE");
        env::set_var("HOME", &global_home);
        env::set_var("USERPROFILE", &global_home);

        let mut config = HelloxConfig::default();
        let mut selected_model = String::new();
        persist_onboarding_selection(
            &config_path,
            &mut config,
            &mut selected_model,
            ProviderOption::OpenAiCompatible,
            ModelPreset::OpenAiOpus,
            "https://openrouter.ai/api/v1".to_string(),
            "sk-test".to_string(),
        )
        .expect("persist onboarding selection");

        let auth_store = LocalAuthStoreBackend::from_config_path(&config_path)
            .load_auth_store()
            .expect("load auth store");
        let provider_keys_path = config_path
            .parent()
            .expect("config parent")
            .join("provider-keys.json");

        if let Some(value) = original_home {
            env::set_var("HOME", value);
        } else {
            env::remove_var("HOME");
        }
        if let Some(value) = original_user_profile {
            env::set_var("USERPROFILE", value);
        } else {
            env::remove_var("USERPROFILE");
        }

        assert!(config.ui.has_completed_onboarding);
        assert_eq!(config.session.model, "openai_opus");
        assert_eq!(selected_model, "openai_opus");
        assert!(config_path.exists());
        assert_eq!(
            get_provider_key(&auth_store, "openai")
                .expect("openai provider key")
                .api_key,
            "sk-test"
        );
        assert!(provider_keys_path.exists());
    }
}
