use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use hellox_config::{
    default_config_path, load_or_default, save_config, HelloxConfig, PermissionMode,
};

use crate::cli_types::ConfigCommands;
use crate::config_panel::render_config_panel;

pub fn handle_config_command(command: ConfigCommands) -> Result<()> {
    println!("{}", config_command_text(command)?);
    Ok(())
}

pub(crate) fn config_command_text(command: ConfigCommands) -> Result<String> {
    match command {
        ConfigCommands::Path => Ok(config_path_text(&default_config_path())),
        ConfigCommands::Show { config } => {
            let (config_path, config) = load_config(config)?;
            Ok(format_config_detail(&config_path, &config))
        }
        ConfigCommands::Panel { focus_key, config } => {
            let (config_path, config) = load_config(config)?;
            match render_config_panel(&config_path, &config, focus_key.as_deref()) {
                Ok(panel) => Ok(panel),
                Err(error) => Ok(format!("Unable to render config panel: {error}")),
            }
        }
        ConfigCommands::Keys => Ok(format_supported_config_keys()),
        ConfigCommands::Set { key, value, config } => {
            let (config_path, mut current) = load_config(config)?;
            apply_config_update(&mut current, &key, Some(&value), false)?;
            save_config(Some(config_path.clone()), &current)?;
            Ok(format!(
                "Updated config key `{key}` in `{}`.\nresolved_value: {}",
                normalize_path(&config_path),
                render_resolved_value(&current, &key),
            ))
        }
        ConfigCommands::Clear { key, config } => {
            let (config_path, mut current) = load_config(config)?;
            apply_config_update(&mut current, &key, None, true)?;
            save_config(Some(config_path.clone()), &current)?;
            Ok(format!(
                "Cleared config key `{key}` in `{}`.\nresolved_value: {}",
                normalize_path(&config_path),
                render_resolved_value(&current, &key),
            ))
        }
    }
}

pub(crate) fn config_path_text(path: &Path) -> String {
    normalize_path(path)
}

fn load_config(config: Option<PathBuf>) -> Result<(PathBuf, HelloxConfig)> {
    let config_path = config.unwrap_or_else(default_config_path);
    let config = load_or_default(Some(config_path.clone()))?;
    Ok((config_path, config))
}

fn format_config_detail(config_path: &Path, config: &HelloxConfig) -> String {
    let rendered = toml::to_string_pretty(config)
        .unwrap_or_else(|error| format!("failed to render config: {error}"));
    format!(
        "config_path: {}\n\n{}",
        normalize_path(config_path),
        rendered
    )
}

fn format_supported_config_keys() -> String {
    [
        "key\tvalue_type\tclearable\tdescription",
        "gateway.listen\tstring\tno\tGateway listen address",
        "output_style.default\tstring\tyes\tDefault output style name",
        "permissions.mode\tstring\tno\tDefault permission mode",
        "prompt.fragments\tstring-list\tyes\tDefault prompt fragments (comma-separated)",
        "prompt.persona\tstring\tyes\tDefault persona name",
        "session.model\tstring\tno\tDefault session model profile",
        "session.persist\tbool\tno\tPersist session snapshots by default",
    ]
    .join("\n")
}

fn apply_config_update(
    config: &mut HelloxConfig,
    key: &str,
    value: Option<&str>,
    clear: bool,
) -> Result<()> {
    match key {
        "session.model" => {
            config.session.model = required_string_value(value, clear, key)?;
        }
        "session.persist" => {
            config.session.persist = required_bool_value(value, clear, key)?;
        }
        "permissions.mode" => {
            let value = required_string_value(value, clear, key)?;
            config.permissions.mode = value
                .parse::<PermissionMode>()
                .map_err(anyhow::Error::msg)?;
        }
        "gateway.listen" => {
            config.gateway.listen = required_string_value(value, clear, key)?;
        }
        "output_style.default" => {
            config.output_style.default = if clear {
                None
            } else {
                Some(required_string_value(value, false, key)?)
            };
        }
        "prompt.persona" => {
            config.prompt.persona = if clear {
                None
            } else {
                Some(required_string_value(value, false, key)?)
            };
        }
        "prompt.fragments" => {
            config.prompt.fragments = if clear {
                Vec::new()
            } else {
                required_string_list_value(value, false, key)?
            };
        }
        _ => {
            return Err(anyhow!(
                "unsupported config key `{key}`; use `hellox config keys` to list supported keys"
            ));
        }
    }

    Ok(())
}

fn render_resolved_value(config: &HelloxConfig, key: &str) -> String {
    match key {
        "session.model" => config.session.model.clone(),
        "session.persist" => config.session.persist.to_string(),
        "permissions.mode" => config.permissions.mode.to_string(),
        "gateway.listen" => config.gateway.listen.clone(),
        "output_style.default" => config
            .output_style
            .default
            .clone()
            .unwrap_or_else(|| "(none)".to_string()),
        "prompt.persona" => config
            .prompt
            .persona
            .clone()
            .unwrap_or_else(|| "(none)".to_string()),
        "prompt.fragments" => {
            if config.prompt.fragments.is_empty() {
                "(none)".to_string()
            } else {
                config.prompt.fragments.join(", ")
            }
        }
        _ => "(unsupported)".to_string(),
    }
}

fn required_string_value(value: Option<&str>, clear: bool, key: &str) -> Result<String> {
    if clear {
        return Err(anyhow!("config key `{key}` does not support `clear`"));
    }
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("config key `{key}` requires a non-empty string value"))
}

fn required_bool_value(value: Option<&str>, clear: bool, key: &str) -> Result<bool> {
    if clear {
        return Err(anyhow!("config key `{key}` does not support `clear`"));
    }
    match value
        .map(str::trim)
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("true" | "1" | "yes" | "on") => Ok(true),
        Some("false" | "0" | "no" | "off") => Ok(false),
        _ => Err(anyhow!(
            "config key `{key}` requires a boolean value (`true`/`false`)"
        )),
    }
}

fn required_string_list_value(value: Option<&str>, clear: bool, key: &str) -> Result<Vec<String>> {
    if clear {
        return Err(anyhow!("config key `{key}` does not support `clear`"));
    }

    let values = value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if values.is_empty() {
        return Err(anyhow!(
            "config key `{key}` requires one or more comma-separated values"
        ));
    }

    Ok(values)
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

    use hellox_config::{load_or_default, save_config, HelloxConfig};

    use super::config_command_text;
    use crate::cli_types::ConfigCommands;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-config-command-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn set_and_clear_config_values() {
        let root = temp_dir();
        let config_path = root.join("config.toml");

        let set_text = config_command_text(ConfigCommands::Set {
            key: "prompt.persona".to_string(),
            value: "reviewer".to_string(),
            config: Some(config_path.clone()),
        })
        .expect("set config");
        assert!(set_text.contains("reviewer"));

        let current = load_or_default(Some(config_path.clone())).expect("load config");
        assert_eq!(current.prompt.persona.as_deref(), Some("reviewer"));

        let clear_text = config_command_text(ConfigCommands::Clear {
            key: "prompt.persona".to_string(),
            config: Some(config_path.clone()),
        })
        .expect("clear config");
        assert!(clear_text.contains("Cleared config key"));

        let reloaded = load_or_default(Some(config_path)).expect("reload config");
        assert_eq!(reloaded.prompt.persona, None);
    }

    #[test]
    fn set_list_value_parses_comma_separated_items() {
        let root = temp_dir();
        let config_path = root.join("config.toml");

        config_command_text(ConfigCommands::Set {
            key: "prompt.fragments".to_string(),
            value: "safety, reviewer".to_string(),
            config: Some(config_path.clone()),
        })
        .expect("set fragments");

        let current = load_or_default(Some(config_path)).expect("load config");
        assert_eq!(
            current.prompt.fragments,
            vec![String::from("safety"), String::from("reviewer")]
        );
    }

    #[test]
    fn show_renders_config_path_and_toml() {
        let root = temp_dir();
        let config_path = root.join("config.toml");
        save_config(Some(config_path.clone()), &HelloxConfig::default()).expect("save config");

        let text = config_command_text(ConfigCommands::Show {
            config: Some(config_path),
        })
        .expect("show config");

        assert!(text.contains("config_path:"));
        assert!(text.contains("[gateway]"));
    }

    #[test]
    fn panel_renders_selector_and_lens() {
        let root = temp_dir();
        let config_path = root.join("config.toml");
        save_config(Some(config_path.clone()), &HelloxConfig::default()).expect("save config");

        let text = config_command_text(ConfigCommands::Panel {
            focus_key: None,
            config: Some(config_path),
        })
        .expect("render panel");

        assert!(text.contains("== Resolved config selector =="));
        assert!(text.contains("== Focused config lens =="));
        assert!(text.contains("hellox config set gateway.listen <value>"));
        assert!(text.contains("/config set gateway.listen <value>"));
    }

    #[test]
    fn focused_panel_marks_selected_key() {
        let root = temp_dir();
        let config_path = root.join("config.toml");
        save_config(Some(config_path.clone()), &HelloxConfig::default()).expect("save config");

        let text = config_command_text(ConfigCommands::Panel {
            focus_key: Some("prompt.persona".to_string()),
            config: Some(config_path),
        })
        .expect("render focused panel");

        assert!(text.contains("> [6] prompt.persona"));
        assert!(text.contains("/config panel prompt.persona"));
    }
}
