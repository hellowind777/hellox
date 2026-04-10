use anyhow::{anyhow, Result};
use hellox_config::{load_or_default, save_config};

use crate::cli_types::PromptFragmentCommands;
use crate::prompt_fragments::{
    discover_prompt_fragments, format_prompt_fragment_detail, format_prompt_fragment_list,
    load_prompt_fragment,
};
use crate::style_command_support::{normalize_path, resolve_config_path, workspace_root};
use crate::style_panels::render_prompt_fragment_panel;

pub fn handle_prompt_fragment_command(command: PromptFragmentCommands) -> Result<()> {
    match command {
        PromptFragmentCommands::Panel {
            fragment_name,
            cwd,
            config,
        } => {
            let config_path = resolve_config_path(config);
            let config = load_or_default(Some(config_path.clone()))?;
            let workspace_root = workspace_root(cwd)?;
            let active_fragments = Vec::new();
            println!(
                "{}",
                render_prompt_fragment_panel(
                    &config_path,
                    &workspace_root,
                    &config.prompt.fragments,
                    &active_fragments,
                    fragment_name.as_deref(),
                )?
            );
        }
        PromptFragmentCommands::List { cwd, config } => {
            let config = load_or_default(Some(resolve_config_path(config)))?;
            let workspace_root = workspace_root(cwd)?;
            let fragments = discover_prompt_fragments(&workspace_root)?;
            println!(
                "{}",
                format_prompt_fragment_list(
                    &fragments,
                    &config.prompt.fragments,
                    &config.prompt.fragments,
                )
            );
        }
        PromptFragmentCommands::Show {
            fragment_name,
            cwd,
            config,
        } => {
            let config = load_or_default(Some(resolve_config_path(config)))?;
            let workspace_root = workspace_root(cwd)?;
            let fragment_name =
                match fragment_name.or_else(|| config.prompt.fragments.first().cloned()) {
                    Some(fragment_name) => fragment_name,
                    None => return Err(anyhow!("No default prompt fragment is configured")),
                };
            let fragment = load_prompt_fragment(&fragment_name, &workspace_root)?;
            println!(
                "{}",
                format_prompt_fragment_detail(
                    &fragment,
                    &config.prompt.fragments,
                    &config.prompt.fragments,
                )
            );
        }
        PromptFragmentCommands::SetDefault {
            fragment_names,
            cwd,
            config,
        } => {
            if fragment_names.is_empty() {
                return Err(anyhow!(
                    "At least one prompt fragment name is required for `set-default`"
                ));
            }
            let config_path = resolve_config_path(config);
            let mut config = load_or_default(Some(config_path.clone()))?;
            let workspace_root = workspace_root(cwd)?;
            for fragment_name in &fragment_names {
                load_prompt_fragment(fragment_name, &workspace_root)?;
            }
            config.prompt.fragments = fragment_names.clone();
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Default prompt fragments set to `{}` in `{}`.",
                fragment_names.join(", "),
                normalize_path(&config_path)
            );
        }
        PromptFragmentCommands::ClearDefault { config } => {
            let config_path = resolve_config_path(config);
            let mut config = load_or_default(Some(config_path.clone()))?;
            config.prompt.fragments.clear();
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Cleared default prompt fragments in `{}`.",
                normalize_path(&config_path)
            );
        }
    }

    Ok(())
}
