mod gateway;
mod localization;
mod onboarding;
mod onboarding_copy;
#[cfg(test)]
mod test_support;
mod trust;
mod trust_copy;

use std::path::PathBuf;

use anyhow::Result;
use hellox_agent::StoredSession;
use hellox_config::{default_config_path, load_or_default, HelloxConfig};

use gateway::ensure_gateway_ready;
pub use localization::{resolve_app_language, AppLanguage};
use onboarding::{resolve_provider_readiness, run_interactive_provider_onboarding};
pub use trust::ensure_workspace_trusted;

pub enum LaunchPreparation {
    Continue { model_override: Option<String> },
    Exit,
}

struct LaunchContext {
    config_path: PathBuf,
    config: HelloxConfig,
    selected_model: String,
    language: AppLanguage,
}

pub fn prepare_interactive_session_launch(
    config_arg: Option<PathBuf>,
    session_id: Option<&str>,
    model_override: Option<&str>,
    gateway_url: Option<&str>,
) -> Result<LaunchPreparation> {
    let mut context = resolve_launch_context(config_arg, session_id, model_override)?;
    let outcome = run_interactive_provider_onboarding(
        &context.config_path,
        &mut context.config,
        &mut context.selected_model,
        context.language,
    )?;
    if outcome.exit_requested {
        return Ok(LaunchPreparation::Exit);
    }
    ensure_gateway_ready(&context.config_path, &context.config, gateway_url)?;
    Ok(LaunchPreparation::Continue {
        model_override: outcome.model_override,
    })
}

pub fn prepare_noninteractive_session_launch(
    config_arg: Option<PathBuf>,
    session_id: Option<&str>,
    model_override: Option<&str>,
    gateway_url: Option<&str>,
) -> Result<Option<String>> {
    let context = resolve_launch_context(config_arg, session_id, model_override)?;
    let readiness = resolve_provider_readiness(&context.config, &context.selected_model)?;
    if !readiness.has_api_key {
        let message = match context.language {
            AppLanguage::English => format!(
                "Provider `{}` ({}) for model `{}` does not have an API key configured. Run `hellox` for the interactive setup flow, or use `hellox auth set-key {}` first.",
                readiness.provider_name,
                readiness.provider_kind,
                context.selected_model,
                readiness.provider_name
            ),
            AppLanguage::SimplifiedChinese => format!(
                "模型 `{}` 对应的 provider `{}`（{}）尚未配置 API Key。请先运行 `hellox` 进入交互式引导，或使用 `hellox auth set-key {}` 完成配置。",
                context.selected_model,
                readiness.provider_name,
                readiness.provider_kind,
                readiness.provider_name
            ),
        };
        return Err(anyhow::anyhow!(message));
    }
    ensure_gateway_ready(&context.config_path, &context.config, gateway_url)?;
    Ok(None)
}

pub fn format_prompt_submission_error(
    language: AppLanguage,
    error: &anyhow::Error,
    config: &HelloxConfig,
    model: &str,
) -> String {
    let text = error.to_string();
    let lower = text.to_ascii_lowercase();
    let readiness = resolve_provider_readiness(config, model).ok();

    if lower.contains("missing api key") {
        return match (language, readiness) {
            (AppLanguage::English, Some(readiness)) => format!(
                "The current provider `{}` ({}) is missing an API key.\nRun `hellox` for onboarding, or use `hellox auth set-key {}` and try again.",
                readiness.provider_name,
                readiness.provider_kind,
                readiness.provider_name
            ),
            (AppLanguage::SimplifiedChinese, Some(readiness)) => format!(
                "当前 provider `{}`（{}）缺少 API Key。\n请先运行 `hellox` 完成引导，或执行 `hellox auth set-key {}` 后重试。",
                readiness.provider_name,
                readiness.provider_kind,
                readiness.provider_name
            ),
            (AppLanguage::English, None) => "An API key is missing for the current model provider.".to_string(),
            (AppLanguage::SimplifiedChinese, None) => "当前模型对应的 provider 缺少 API Key。".to_string(),
        };
    }

    if lower.contains("failed to send request to hellox gateway")
        || lower.contains("hellox gateway returned an error status")
    {
        return match language {
            AppLanguage::English => format!(
                "The local hellox gateway request failed.\nRun `/doctor` to inspect provider and gateway status.\nOriginal error: {text}"
            ),
            AppLanguage::SimplifiedChinese => format!(
                "本地 hellox gateway 请求失败。\n请先运行 `/doctor` 检查 provider 与 gateway 状态。\n原始错误：{text}"
            ),
        };
    }

    match language {
        AppLanguage::English => format!("Request failed.\n{text}"),
        AppLanguage::SimplifiedChinese => format!("请求失败。\n{text}"),
    }
}

fn resolve_launch_context(
    config_arg: Option<PathBuf>,
    session_id: Option<&str>,
    model_override: Option<&str>,
) -> Result<LaunchContext> {
    let config_path = config_arg.unwrap_or_else(default_config_path);
    let config = load_or_default(Some(config_path.clone()))?;
    let language = resolve_app_language(&config);
    let selected_model = resolve_selected_model(&config, session_id, model_override)?;
    Ok(LaunchContext {
        config_path,
        config,
        selected_model,
        language,
    })
}

fn resolve_selected_model(
    config: &HelloxConfig,
    session_id: Option<&str>,
    model_override: Option<&str>,
) -> Result<String> {
    if let Some(model_override) = model_override.filter(|value| !value.trim().is_empty()) {
        return Ok(model_override.to_string());
    }
    if let Some(session_id) = session_id {
        let stored = StoredSession::load(session_id)?;
        return Ok(stored.snapshot.model);
    }
    Ok(config.session.model.clone())
}
