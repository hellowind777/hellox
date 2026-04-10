use std::path::Path;

use anyhow::Result;
use hellox_config::{default_config_path, load_or_default, plugins_root, save_config};
use hellox_marketplace::{
    add_marketplace, build_marketplace, format_marketplace_detail, format_marketplace_list,
    get_marketplace, remove_marketplace, set_marketplace_enabled,
};
use hellox_plugin::{
    format_plugin_detail, format_plugin_list, inspect_plugin, install_plugin,
    load_installed_plugins, remove_plugin, set_plugin_enabled,
};

use crate::cli_types::{MarketplaceCommands, PluginCommands};
use crate::plugin_panel::render_plugin_panel;

pub fn handle_plugin_command(command: PluginCommands) -> Result<()> {
    let config_path = default_config_path();
    let mut config = load_or_default(Some(config_path.clone()))?;
    let plugins_dir = plugins_root();

    match command {
        PluginCommands::Panel { plugin_id } => {
            println!(
                "{}",
                render_plugin_panel(&config_path, &config, plugin_id.as_deref())?
            );
        }
        PluginCommands::List => {
            println!("{}", format_plugin_list(&load_installed_plugins(&config)));
        }
        PluginCommands::Show { plugin_id } => {
            println!(
                "{}",
                format_plugin_detail(&inspect_plugin(&config, &plugin_id)?)
            );
        }
        PluginCommands::Install { source, disabled } => {
            let result = install_plugin(&mut config, &source, &plugins_dir, !disabled)?;
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Installed plugin `{}` to `{}`.",
                result.plugin_id,
                normalize_path(&result.install_path)
            );
        }
        PluginCommands::Enable { plugin_id } => {
            set_plugin_enabled(&mut config, &plugin_id, true)?;
            save_config(Some(config_path.clone()), &config)?;
            println!("Enabled plugin `{plugin_id}`.");
        }
        PluginCommands::Disable { plugin_id } => {
            set_plugin_enabled(&mut config, &plugin_id, false)?;
            save_config(Some(config_path.clone()), &config)?;
            println!("Disabled plugin `{plugin_id}`.");
        }
        PluginCommands::Remove { plugin_id } => {
            let result = remove_plugin(&mut config, &plugin_id, &plugins_dir)?;
            save_config(Some(config_path.clone()), &config)?;
            match result.removed_path {
                Some(path) => println!(
                    "Removed plugin `{}` from `{}`.",
                    result.plugin_id,
                    normalize_path(&path)
                ),
                None => println!("Removed plugin `{}`.", result.plugin_id),
            }
        }
        PluginCommands::Marketplace { command } => {
            handle_marketplace_command(command, &mut config, &config_path)?;
        }
    }

    Ok(())
}

fn handle_marketplace_command(
    command: MarketplaceCommands,
    config: &mut hellox_config::HelloxConfig,
    config_path: &Path,
) -> Result<()> {
    match command {
        MarketplaceCommands::List => {
            println!("{}", format_marketplace_list(config));
        }
        MarketplaceCommands::Show { marketplace_name } => {
            println!(
                "{}",
                format_marketplace_detail(
                    &marketplace_name,
                    get_marketplace(config, &marketplace_name)?
                )
            );
        }
        MarketplaceCommands::Add {
            marketplace_name,
            url,
            description,
        } => {
            add_marketplace(
                config,
                marketplace_name.clone(),
                build_marketplace(url, description),
            )?;
            save_config(Some(config_path.to_path_buf()), config)?;
            println!("Added plugin marketplace `{marketplace_name}`.");
        }
        MarketplaceCommands::Enable { marketplace_name } => {
            set_marketplace_enabled(config, &marketplace_name, true)?;
            save_config(Some(config_path.to_path_buf()), config)?;
            println!("Enabled plugin marketplace `{marketplace_name}`.");
        }
        MarketplaceCommands::Disable { marketplace_name } => {
            set_marketplace_enabled(config, &marketplace_name, false)?;
            save_config(Some(config_path.to_path_buf()), config)?;
            println!("Disabled plugin marketplace `{marketplace_name}`.");
        }
        MarketplaceCommands::Remove { marketplace_name } => {
            remove_marketplace(config, &marketplace_name)?;
            save_config(Some(config_path.to_path_buf()), config)?;
            println!("Removed plugin marketplace `{marketplace_name}`.");
        }
    }

    Ok(())
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
