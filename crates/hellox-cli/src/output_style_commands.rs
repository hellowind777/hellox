use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use hellox_config::{default_config_path, load_or_default, save_config};

use crate::cli_types::OutputStyleCommands;
use crate::output_styles::{
    discover_output_styles, format_output_style_detail, format_output_style_list, load_output_style,
};
use crate::style_panels::render_output_style_panel;

pub fn handle_output_style_command(command: OutputStyleCommands) -> Result<()> {
    match command {
        OutputStyleCommands::Panel {
            style_name,
            cwd,
            config,
        } => {
            let config_path = resolve_config_path(config);
            let config = load_or_default(Some(config_path.clone()))?;
            let workspace_root = workspace_root(cwd)?;
            println!(
                "{}",
                render_output_style_panel(
                    &config_path,
                    &workspace_root,
                    config.output_style.default.as_deref(),
                    None,
                    style_name.as_deref(),
                )?
            );
        }
        OutputStyleCommands::List { cwd, config } => {
            let config = load_or_default(Some(resolve_config_path(config)))?;
            let workspace_root = workspace_root(cwd)?;
            let styles = discover_output_styles(&workspace_root)?;
            println!(
                "{}",
                format_output_style_list(&styles, config.output_style.default.as_deref())
            );
        }
        OutputStyleCommands::Show {
            style_name,
            cwd,
            config,
        } => {
            let config = load_or_default(Some(resolve_config_path(config)))?;
            let workspace_root = workspace_root(cwd)?;
            let style_name = match style_name.or_else(|| config.output_style.default.clone()) {
                Some(style_name) => style_name,
                None => return Err(anyhow!("No default output style is configured")),
            };
            let style = load_output_style(&style_name, &workspace_root)?;
            println!(
                "{}",
                format_output_style_detail(
                    &style,
                    config.output_style.default.as_deref() == Some(style.name.as_str()),
                    false,
                )
            );
        }
        OutputStyleCommands::SetDefault {
            style_name,
            cwd,
            config,
        } => {
            let config_path = resolve_config_path(config);
            let mut config = load_or_default(Some(config_path.clone()))?;
            let workspace_root = workspace_root(cwd)?;
            load_output_style(&style_name, &workspace_root)?;
            config.output_style.default = Some(style_name.clone());
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Default output style set to `{style_name}` in `{}`.",
                normalize_path(&config_path)
            );
        }
        OutputStyleCommands::ClearDefault { config } => {
            let config_path = resolve_config_path(config);
            let mut config = load_or_default(Some(config_path.clone()))?;
            config.output_style.default = None;
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Cleared default output style in `{}`.",
                normalize_path(&config_path)
            );
        }
    }

    Ok(())
}

fn resolve_config_path(value: Option<PathBuf>) -> PathBuf {
    value.unwrap_or_else(default_config_path)
}

fn workspace_root(value: Option<PathBuf>) -> Result<PathBuf> {
    Ok(match value {
        Some(path) => path,
        None => env::current_dir()?,
    })
}

fn normalize_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_config::load_or_default;

    use super::handle_output_style_command;
    use crate::cli_types::OutputStyleCommands;
    use crate::output_styles::project_output_styles_root;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-output-style-command-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn set_and_clear_default_output_style_updates_config() {
        let root = temp_dir();
        let config_path = root.join("config.toml");
        let styles_root = project_output_styles_root(&root);
        fs::create_dir_all(&styles_root).expect("create styles root");
        fs::write(styles_root.join("concise.md"), "Keep answers short.\n").expect("write style");

        handle_output_style_command(OutputStyleCommands::SetDefault {
            style_name: "concise".to_string(),
            cwd: Some(root.clone()),
            config: Some(config_path.clone()),
        })
        .expect("set default");

        let config = load_or_default(Some(config_path.clone())).expect("load config");
        assert_eq!(config.output_style.default.as_deref(), Some("concise"));

        handle_output_style_command(OutputStyleCommands::ClearDefault {
            config: Some(config_path.clone()),
        })
        .expect("clear default");

        let config = load_or_default(Some(config_path)).expect("reload config");
        assert_eq!(config.output_style.default, None);
    }
}
