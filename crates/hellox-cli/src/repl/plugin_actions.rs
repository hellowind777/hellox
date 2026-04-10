use anyhow::Result;
use hellox_config::{load_or_default, save_config};
use hellox_marketplace::{
    add_marketplace, build_marketplace, format_marketplace_detail, format_marketplace_list,
    get_marketplace, remove_marketplace, set_marketplace_enabled,
};
use hellox_plugin::{
    format_plugin_detail, format_plugin_list, inspect_plugin, install_plugin,
    load_installed_plugins, remove_plugin, set_plugin_enabled,
};

use super::commands::{MarketplaceCommand, PluginCommand};
use super::ReplMetadata;
use crate::plugin_panel::render_plugin_panel;

pub(super) fn handle_plugin_command(
    command: PluginCommand,
    metadata: &ReplMetadata,
) -> Result<String> {
    let mut config = load_or_default(Some(metadata.config_path.clone()))?;

    match command {
        PluginCommand::Help => Ok(help_text()),
        PluginCommand::Panel { plugin_id } => {
            render_plugin_panel(&metadata.config_path, &config, plugin_id.as_deref())
        }
        PluginCommand::List => Ok(format_plugin_list(&load_installed_plugins(&config))),
        PluginCommand::Show { plugin_id: None } => {
            Ok("Usage: /plugin show <plugin-id>".to_string())
        }
        PluginCommand::Show {
            plugin_id: Some(plugin_id),
        } => Ok(format_plugin_detail(&inspect_plugin(&config, &plugin_id)?)),
        PluginCommand::Install { source: None, .. } => {
            Ok("Usage: /plugin install <path>".to_string())
        }
        PluginCommand::Install {
            source: Some(source),
            disabled,
        } => {
            let result = install_plugin(
                &mut config,
                &std::path::PathBuf::from(&source),
                &metadata.plugins_root,
                !disabled,
            )?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!(
                "Installed plugin `{}` to `{}`.",
                result.plugin_id,
                format_path(&result.install_path)
            ))
        }
        PluginCommand::Enable { plugin_id: None } => {
            Ok("Usage: /plugin enable <plugin-id>".to_string())
        }
        PluginCommand::Enable {
            plugin_id: Some(plugin_id),
        } => {
            set_plugin_enabled(&mut config, &plugin_id, true)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Enabled plugin `{plugin_id}`."))
        }
        PluginCommand::Disable { plugin_id: None } => {
            Ok("Usage: /plugin disable <plugin-id>".to_string())
        }
        PluginCommand::Disable {
            plugin_id: Some(plugin_id),
        } => {
            set_plugin_enabled(&mut config, &plugin_id, false)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            Ok(format!("Disabled plugin `{plugin_id}`."))
        }
        PluginCommand::Remove { plugin_id: None } => {
            Ok("Usage: /plugin remove <plugin-id>".to_string())
        }
        PluginCommand::Remove {
            plugin_id: Some(plugin_id),
        } => {
            let result = remove_plugin(&mut config, &plugin_id, &metadata.plugins_root)?;
            save_config(Some(metadata.config_path.clone()), &config)?;
            match result.removed_path {
                Some(path) => Ok(format!(
                    "Removed plugin `{}` from `{}`.",
                    result.plugin_id,
                    format_path(&path)
                )),
                None => Ok(format!("Removed plugin `{}`.", result.plugin_id)),
            }
        }
        PluginCommand::Marketplace(command) => {
            handle_marketplace_command(command, &mut config, metadata)
        }
    }
}

fn handle_marketplace_command(
    command: MarketplaceCommand,
    config: &mut hellox_config::HelloxConfig,
    metadata: &ReplMetadata,
) -> Result<String> {
    match command {
        MarketplaceCommand::List => Ok(format_marketplace_list(config)),
        MarketplaceCommand::Show {
            marketplace_name: None,
        } => Ok("Usage: /plugin marketplace show <name>".to_string()),
        MarketplaceCommand::Show {
            marketplace_name: Some(marketplace_name),
        } => Ok(format_marketplace_detail(
            &marketplace_name,
            get_marketplace(config, &marketplace_name)?,
        )),
        MarketplaceCommand::Add {
            marketplace_name: None,
            url: _,
        } => Ok("Usage: /plugin marketplace add <name> <url>".to_string()),
        MarketplaceCommand::Add { url: None, .. } => {
            Ok("Usage: /plugin marketplace add <name> <url>".to_string())
        }
        MarketplaceCommand::Add {
            marketplace_name: Some(marketplace_name),
            url: Some(url),
        } => {
            add_marketplace(
                config,
                marketplace_name.clone(),
                build_marketplace(url, None),
            )?;
            save_config(Some(metadata.config_path.clone()), config)?;
            Ok(format!("Added plugin marketplace `{marketplace_name}`."))
        }
        MarketplaceCommand::Enable {
            marketplace_name: None,
        } => Ok("Usage: /plugin marketplace enable <name>".to_string()),
        MarketplaceCommand::Enable {
            marketplace_name: Some(marketplace_name),
        } => {
            set_marketplace_enabled(config, &marketplace_name, true)?;
            save_config(Some(metadata.config_path.clone()), config)?;
            Ok(format!("Enabled plugin marketplace `{marketplace_name}`."))
        }
        MarketplaceCommand::Disable {
            marketplace_name: None,
        } => Ok("Usage: /plugin marketplace disable <name>".to_string()),
        MarketplaceCommand::Disable {
            marketplace_name: Some(marketplace_name),
        } => {
            set_marketplace_enabled(config, &marketplace_name, false)?;
            save_config(Some(metadata.config_path.clone()), config)?;
            Ok(format!("Disabled plugin marketplace `{marketplace_name}`."))
        }
        MarketplaceCommand::Remove {
            marketplace_name: None,
        } => Ok("Usage: /plugin marketplace remove <name>".to_string()),
        MarketplaceCommand::Remove {
            marketplace_name: Some(marketplace_name),
        } => {
            remove_marketplace(config, &marketplace_name)?;
            save_config(Some(metadata.config_path.clone()), config)?;
            Ok(format!("Removed plugin marketplace `{marketplace_name}`."))
        }
        MarketplaceCommand::Help => Ok(help_text()),
    }
}

fn help_text() -> String {
    [
        "Usage:",
        "  /plugin",
        "  /plugin panel [plugin-id]",
        "  /plugin show <plugin-id>",
        "  /plugin install <path>",
        "  /plugin enable <plugin-id>",
        "  /plugin disable <plugin-id>",
        "  /plugin remove <plugin-id>",
        "  /plugin marketplace",
        "  /plugin marketplace add <name> <url>",
        "  /plugin marketplace enable <name>",
        "  /plugin marketplace disable <name>",
        "  /plugin marketplace remove <name>",
    ]
    .join("\n")
}

fn format_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}
