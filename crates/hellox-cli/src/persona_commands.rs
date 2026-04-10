use anyhow::{anyhow, Result};
use hellox_config::{load_or_default, save_config};

use crate::cli_types::PersonaCommands;
use crate::personas::{
    discover_personas, format_persona_detail, format_persona_list, load_persona,
};
use crate::style_command_support::{normalize_path, resolve_config_path, workspace_root};
use crate::style_panels::render_persona_panel;

pub fn handle_persona_command(command: PersonaCommands) -> Result<()> {
    match command {
        PersonaCommands::Panel {
            persona_name,
            cwd,
            config,
        } => {
            let config_path = resolve_config_path(config);
            let config = load_or_default(Some(config_path.clone()))?;
            let workspace_root = workspace_root(cwd)?;
            println!(
                "{}",
                render_persona_panel(
                    &config_path,
                    &workspace_root,
                    config.prompt.persona.as_deref(),
                    None,
                    persona_name.as_deref(),
                )?
            );
        }
        PersonaCommands::List { cwd, config } => {
            let config = load_or_default(Some(resolve_config_path(config)))?;
            let workspace_root = workspace_root(cwd)?;
            let personas = discover_personas(&workspace_root)?;
            println!(
                "{}",
                format_persona_list(
                    &personas,
                    config.prompt.persona.as_deref(),
                    config.prompt.persona.as_deref(),
                )
            );
        }
        PersonaCommands::Show {
            persona_name,
            cwd,
            config,
        } => {
            let config = load_or_default(Some(resolve_config_path(config)))?;
            let workspace_root = workspace_root(cwd)?;
            let persona_name = match persona_name.or_else(|| config.prompt.persona.clone()) {
                Some(persona_name) => persona_name,
                None => return Err(anyhow!("No default persona is configured")),
            };
            let persona = load_persona(&persona_name, &workspace_root)?;
            println!(
                "{}",
                format_persona_detail(
                    &persona,
                    config.prompt.persona.as_deref(),
                    config.prompt.persona.as_deref(),
                )
            );
        }
        PersonaCommands::SetDefault {
            persona_name,
            cwd,
            config,
        } => {
            let config_path = resolve_config_path(config);
            let mut config = load_or_default(Some(config_path.clone()))?;
            let workspace_root = workspace_root(cwd)?;
            load_persona(&persona_name, &workspace_root)?;
            config.prompt.persona = Some(persona_name.clone());
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Default persona set to `{persona_name}` in `{}`.",
                normalize_path(&config_path)
            );
        }
        PersonaCommands::ClearDefault { config } => {
            let config_path = resolve_config_path(config);
            let mut config = load_or_default(Some(config_path.clone()))?;
            config.prompt.persona = None;
            save_config(Some(config_path.clone()), &config)?;
            println!(
                "Cleared default persona in `{}`.",
                normalize_path(&config_path)
            );
        }
    }

    Ok(())
}
