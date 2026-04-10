use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use hellox_config::{
    default_config_path, load_or_default, save_config, HelloxConfig, ModelPricing, PermissionMode,
    ProfileConfig,
};

use crate::cli_types::{ModelCommands, PermissionsCommands};
use crate::model_panel::render_model_panel;

pub fn handle_model_command(command: ModelCommands) -> Result<()> {
    println!("{}", model_command_text(command)?);
    Ok(())
}

pub(crate) fn model_command_text(command: ModelCommands) -> Result<String> {
    match command {
        ModelCommands::Panel {
            profile_name,
            config,
        } => {
            let (config_path, config) = load_config(config)?;
            render_model_panel(&config_path, &config, profile_name.as_deref(), None)
        }
        ModelCommands::List { config } => {
            let (_, config) = load_config(config)?;
            Ok(format_model_list(&config.session.model, &config.profiles))
        }
        ModelCommands::Show {
            profile_name,
            config,
        } => {
            let (config_path, config) = load_config(config)?;
            let profile_name = profile_name.unwrap_or_else(|| config.session.model.clone());
            let profile = config
                .profiles
                .get(&profile_name)
                .ok_or_else(|| anyhow!("Model profile `{profile_name}` was not found"))?;
            Ok(format_model_detail(
                &profile_name,
                profile,
                config.session.model == profile_name,
                &config_path,
            ))
        }
        ModelCommands::SetDefault {
            profile_name,
            config,
        } => {
            let (config_path, mut config) = load_config(config)?;
            ensure_profile_exists(&config, &profile_name)?;
            config.session.model = profile_name.clone();
            save_config(Some(config_path.clone()), &config)?;
            Ok(format!(
                "Default model set to `{profile_name}` in `{}`.",
                normalize_path(&config_path)
            ))
        }
        ModelCommands::Save {
            profile_name,
            provider,
            upstream_model,
            display_name,
            input_price,
            output_price,
            set_default,
            config,
        } => {
            let (config_path, mut config) = load_config(config)?;
            ensure_provider_exists(&config, &provider)?;
            let pricing = build_pricing(input_price, output_price)?;
            let existed = config.profiles.contains_key(&profile_name);
            config.profiles.insert(
                profile_name.clone(),
                ProfileConfig {
                    provider: provider.clone(),
                    upstream_model: upstream_model.clone(),
                    display_name,
                    pricing,
                },
            );
            if set_default {
                config.session.model = profile_name.clone();
            }
            save_config(Some(config_path.clone()), &config)?;
            Ok(format!(
                "{} model profile `{}` in `{}`{}.",
                if existed { "Updated" } else { "Saved" },
                profile_name,
                normalize_path(&config_path),
                if set_default {
                    " and set it as default"
                } else {
                    ""
                }
            ))
        }
        ModelCommands::Remove {
            profile_name,
            config,
        } => {
            let (config_path, mut config) = load_config(config)?;
            if config.profiles.remove(&profile_name).is_none() {
                return Err(anyhow!("Model profile `{profile_name}` was not found"));
            }
            let mut default_note = String::new();
            if config.session.model == profile_name {
                let next_default = config.profiles.keys().next().cloned().ok_or_else(|| {
                    anyhow!("Cannot remove the last remaining default model profile")
                })?;
                config.session.model = next_default.clone();
                default_note = format!(" Default model switched to `{next_default}`.");
            }
            save_config(Some(config_path.clone()), &config)?;
            Ok(format!(
                "Removed model profile `{}` from `{}`.{}",
                profile_name,
                normalize_path(&config_path),
                default_note
            ))
        }
    }
}

pub fn handle_permissions_command(command: PermissionsCommands) -> Result<()> {
    match command {
        PermissionsCommands::Show { config } => {
            let (config_path, config) = load_config(config)?;
            println!(
                "{}",
                format_permissions_detail(config.permissions.mode, &config_path)
            );
        }
        PermissionsCommands::Set { mode, config } => {
            let (config_path, mut config) = load_config(config)?;
            config.permissions.mode = mode.clone();
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Default permission mode set to `{mode}` in `{}`.",
                normalize_path(&config_path)
            );
        }
    }

    Ok(())
}

fn load_config(config: Option<PathBuf>) -> Result<(PathBuf, HelloxConfig)> {
    let config_path = config.unwrap_or_else(default_config_path);
    let config = load_or_default(Some(config_path.clone()))?;
    Ok((config_path, config))
}

fn ensure_profile_exists(config: &HelloxConfig, profile_name: &str) -> Result<()> {
    if config.profiles.contains_key(profile_name) {
        Ok(())
    } else {
        Err(anyhow!("Model profile `{profile_name}` was not found"))
    }
}

fn ensure_provider_exists(config: &HelloxConfig, provider_name: &str) -> Result<()> {
    if config.providers.contains_key(provider_name) {
        Ok(())
    } else {
        Err(anyhow!(
            "Provider `{provider_name}` was not found. Available providers: {}",
            config
                .providers
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

fn build_pricing(
    input_price: Option<f64>,
    output_price: Option<f64>,
) -> Result<Option<ModelPricing>> {
    match (input_price, output_price) {
        (None, None) => Ok(None),
        (Some(input_per_million_usd), Some(output_per_million_usd)) => Ok(Some(ModelPricing {
            input_per_million_usd,
            output_per_million_usd,
        })),
        _ => Err(anyhow!(
            "Both `--input-price` and `--output-price` must be provided together"
        )),
    }
}

fn format_model_list(default_model: &str, profiles: &BTreeMap<String, ProfileConfig>) -> String {
    if profiles.is_empty() {
        return "No model profiles configured.".to_string();
    }

    let mut lines =
        vec!["profile\tdefault\tdisplay_name\tprovider\tupstream_model\tpricing".to_string()];
    for (name, profile) in profiles {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            name,
            if name == default_model { "yes" } else { "no" },
            profile.display_name.as_deref().unwrap_or(name),
            profile.provider,
            profile.upstream_model,
            format_pricing(profile.pricing.as_ref())
        ));
    }
    lines.join("\n")
}

fn format_model_detail(
    profile_name: &str,
    profile: &ProfileConfig,
    is_default: bool,
    config_path: &Path,
) -> String {
    format!(
        "profile: {}\ndefault: {}\nprovider: {}\nupstream_model: {}\ndisplay_name: {}\npricing: {}\nconfig_path: {}",
        profile_name,
        is_default,
        profile.provider,
        profile.upstream_model,
        profile.display_name.as_deref().unwrap_or(profile_name),
        format_pricing(profile.pricing.as_ref()),
        normalize_path(config_path)
    )
}

fn format_pricing(pricing: Option<&ModelPricing>) -> String {
    pricing
        .map(|pricing| {
            format!(
                "input=${:.2}/1M output=${:.2}/1M",
                pricing.input_per_million_usd, pricing.output_per_million_usd
            )
        })
        .unwrap_or_else(|| "(none)".to_string())
}

fn format_permissions_detail(mode: PermissionMode, config_path: &Path) -> String {
    format!(
        "default_permission_mode: {}\nsupported_modes: {}\nconfig_path: {}",
        mode,
        PermissionMode::supported_values().join(", "),
        normalize_path(config_path)
    )
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_config::{load_or_default, save_config, HelloxConfig, PermissionMode};

    use super::{
        format_model_list, handle_model_command, handle_permissions_command, model_command_text,
    };
    use crate::cli_types::{ModelCommands, PermissionsCommands};

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-settings-commands-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn config_path(root: &PathBuf) -> PathBuf {
        root.join("config.toml")
    }

    #[test]
    fn model_set_default_persists_selected_profile() {
        let root = temp_dir();
        let path = config_path(&root);
        save_config(Some(path.clone()), &HelloxConfig::default()).expect("save config");

        handle_model_command(ModelCommands::SetDefault {
            profile_name: "sonnet".to_string(),
            config: Some(path.clone()),
        })
        .expect("set default model");

        let config = load_or_default(Some(path)).expect("load config");
        assert_eq!(config.session.model, "sonnet");
    }

    #[test]
    fn permissions_set_persists_mode() {
        let root = temp_dir();
        let path = config_path(&root);
        save_config(Some(path.clone()), &HelloxConfig::default()).expect("save config");

        handle_permissions_command(PermissionsCommands::Set {
            mode: PermissionMode::AcceptEdits,
            config: Some(path.clone()),
        })
        .expect("set permissions");

        let config = load_or_default(Some(path)).expect("load config");
        assert_eq!(config.permissions.mode, PermissionMode::AcceptEdits);
    }

    #[test]
    fn model_list_marks_current_default() {
        let config = HelloxConfig::default();
        let text = format_model_list(&config.session.model, &config.profiles);
        assert!(text.contains("opus\tyes"));
        assert!(text.contains("sonnet\tno"));
        assert!(text.contains("input=$15.00/1M output=$75.00/1M"));
    }

    #[test]
    fn model_save_persists_profile_and_pricing() {
        let root = temp_dir();
        let path = config_path(&root);
        save_config(Some(path.clone()), &HelloxConfig::default()).expect("save config");

        let text = model_command_text(ModelCommands::Save {
            profile_name: "custom".to_string(),
            provider: "openai".to_string(),
            upstream_model: "gpt-4.1-mini".to_string(),
            display_name: Some("Custom".to_string()),
            input_price: Some(0.25),
            output_price: Some(1.25),
            set_default: true,
            config: Some(path.clone()),
        })
        .expect("save profile");

        assert!(text.contains("Saved model profile `custom`"));
        let config = load_or_default(Some(path)).expect("load config");
        assert_eq!(config.session.model, "custom");
        let profile = config.profiles.get("custom").expect("custom profile");
        assert_eq!(profile.provider, "openai");
        assert_eq!(profile.upstream_model, "gpt-4.1-mini");
        assert_eq!(profile.display_name.as_deref(), Some("Custom"));
        let pricing = profile.pricing.as_ref().expect("custom pricing");
        assert_eq!(pricing.input_per_million_usd, 0.25);
        assert_eq!(pricing.output_per_million_usd, 1.25);
    }

    #[test]
    fn model_remove_switches_default_when_needed() {
        let root = temp_dir();
        let path = config_path(&root);
        save_config(Some(path.clone()), &HelloxConfig::default()).expect("save config");

        let text = model_command_text(ModelCommands::Remove {
            profile_name: "opus".to_string(),
            config: Some(path.clone()),
        })
        .expect("remove profile");

        assert!(text.contains("Removed model profile `opus`"));
        assert!(text.contains("Default model switched to `haiku`."));
        let config = load_or_default(Some(path)).expect("load config");
        assert_eq!(config.session.model, "haiku");
        assert!(!config.profiles.contains_key("opus"));
    }
}
