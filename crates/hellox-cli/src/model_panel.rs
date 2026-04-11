use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_config::{HelloxConfig, ModelPricing, ProviderConfig};
use hellox_tui::{render_panel, render_table, KeyValueRow, PanelSection, Table};

use crate::style_command_support::normalize_path;

pub(crate) fn render_model_panel(
    config_path: &Path,
    config: &HelloxConfig,
    profile_name: Option<&str>,
    active_model: Option<&str>,
) -> Result<String> {
    let profile_name = profile_name
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match profile_name {
        Some(profile_name) => {
            render_model_detail_panel(config_path, config, profile_name, active_model)
        }
        None => Ok(render_model_list_panel(config_path, config, active_model)),
    }
}

pub(crate) fn model_panel_profile_names(config: &HelloxConfig) -> Vec<String> {
    config.profiles.keys().cloned().collect()
}

fn render_model_list_panel(
    config_path: &Path,
    config: &HelloxConfig,
    active_model: Option<&str>,
) -> String {
    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("default_model", config.session.model.clone()),
        KeyValueRow::new("active_model", active_model.unwrap_or("(none)")),
        KeyValueRow::new("profiles", config.profiles.len().to_string()),
        KeyValueRow::new("providers", config.providers.len().to_string()),
    ];
    let sections = vec![
        PanelSection::new(
            "Profiles",
            render_table(&build_profile_table(config, active_model)),
        ),
        PanelSection::new("Action palette", model_list_cli_palette()),
        PanelSection::new("REPL palette", model_list_repl_palette()),
    ];

    render_panel("Model panel", &metadata, &sections)
}

fn render_model_detail_panel(
    config_path: &Path,
    config: &HelloxConfig,
    profile_name: &str,
    active_model: Option<&str>,
) -> Result<String> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| anyhow!("Model profile `{profile_name}` was not found"))?;

    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("profile", profile_name.to_string()),
        KeyValueRow::new("default", yes_no(config.session.model == profile_name)),
        KeyValueRow::new("active", yes_no(active_model == Some(profile_name))),
        KeyValueRow::new("provider", profile.provider.clone()),
        KeyValueRow::new("provider_kind", provider_kind(config, &profile.provider)),
        KeyValueRow::new("upstream_model", profile.upstream_model.clone()),
        KeyValueRow::new(
            "display_name",
            profile.display_name.as_deref().unwrap_or(profile_name),
        ),
        KeyValueRow::new("pricing", pricing_label(profile.pricing.as_ref())),
    ];
    let sections = vec![
        PanelSection::new("Provider", provider_lines(config, &profile.provider)),
        PanelSection::new("Action palette", model_detail_cli_palette(profile_name)),
        PanelSection::new("REPL palette", model_detail_repl_palette(profile_name)),
    ];

    Ok(render_panel(
        &format!("Model panel: {profile_name}"),
        &metadata,
        &sections,
    ))
}

fn build_profile_table(config: &HelloxConfig, active_model: Option<&str>) -> Table {
    let rows = config
        .profiles
        .iter()
        .enumerate()
        .map(|(index, (profile_name, profile))| {
            vec![
                (index + 1).to_string(),
                profile_name.clone(),
                yes_no(config.session.model == *profile_name),
                yes_no(active_model == Some(profile_name.as_str())),
                profile.provider.clone(),
                profile.upstream_model.clone(),
                profile
                    .display_name
                    .clone()
                    .unwrap_or_else(|| profile_name.clone()),
                pricing_label(profile.pricing.as_ref()),
                format!("hellox model panel {profile_name}"),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "#".to_string(),
            "profile".to_string(),
            "default".to_string(),
            "active".to_string(),
            "provider".to_string(),
            "upstream".to_string(),
            "display".to_string(),
            "pricing".to_string(),
            "open".to_string(),
        ],
        rows,
    )
}

fn provider_lines(config: &HelloxConfig, provider_name: &str) -> Vec<String> {
    match config.providers.get(provider_name) {
        Some(ProviderConfig::Anthropic {
            base_url,
            anthropic_version,
            api_key_env,
        }) => vec![
            "kind: anthropic".to_string(),
            format!("base_url: {base_url}"),
            format!("anthropic_version: {anthropic_version}"),
            format!("api_key_env: {api_key_env}"),
        ],
        Some(ProviderConfig::OpenAiCompatible {
            base_url,
            api_key_env,
        }) => vec![
            "kind: openai_compatible".to_string(),
            format!("base_url: {base_url}"),
            format!("api_key_env: {api_key_env}"),
        ],
        None => vec![format!("provider `{provider_name}` is not configured")],
    }
}

fn provider_kind(config: &HelloxConfig, provider_name: &str) -> &'static str {
    match config.providers.get(provider_name) {
        Some(ProviderConfig::Anthropic { .. }) => "anthropic",
        Some(ProviderConfig::OpenAiCompatible { .. }) => "openai_compatible",
        None => "missing",
    }
}

fn model_list_cli_palette() -> Vec<String> {
    vec![
        "- open panel: `hellox model panel <profile-name>`".to_string(),
        "- show raw: `hellox model show <profile-name>`".to_string(),
        "- set default: `hellox model set-default <profile-name>`".to_string(),
        "- list profiles: `hellox model list`".to_string(),
    ]
}

fn model_list_repl_palette() -> Vec<String> {
    vec![
        "- open panel: `/model panel [profile-name]`".to_string(),
        "- numeric open: render `/model panel`, then enter `1..n`".to_string(),
        "- show raw: `/model show [profile-name]`".to_string(),
        "- use for session: `/model use <profile-name>`".to_string(),
        "- persist default: `/model default <profile-name>`".to_string(),
    ]
}

fn model_detail_cli_palette(profile_name: &str) -> Vec<String> {
    vec![
        "- back to list: `hellox model panel`".to_string(),
        format!("- show raw: `hellox model show {profile_name}`"),
        format!("- set default: `hellox model set-default {profile_name}`"),
        format!(
            "- save updated profile: `hellox model save {profile_name} --provider <provider> --upstream-model <model>`"
        ),
    ]
}

fn model_detail_repl_palette(profile_name: &str) -> Vec<String> {
    vec![
        "- back to list: `/model panel`".to_string(),
        format!("- show raw: `/model show {profile_name}`"),
        format!("- use for session: `/model use {profile_name}`"),
        format!("- persist default: `/model default {profile_name}`"),
    ]
}

fn pricing_label(pricing: Option<&ModelPricing>) -> String {
    pricing
        .map(|pricing| {
            format!(
                "in=${:.2}/1M out=${:.2}/1M",
                pricing.input_per_million_usd, pricing.output_per_million_usd
            )
        })
        .unwrap_or_else(|| "(none)".to_string())
}

fn yes_no(value: bool) -> String {
    if value {
        "YES".to_string()
    } else {
        "NO".to_string()
    }
}
