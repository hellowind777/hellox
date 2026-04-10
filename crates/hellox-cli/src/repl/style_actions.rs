use std::path::Path;

use anyhow::Result;
use hellox_agent::{AgentSession, OutputStylePrompt, PersonaPrompt, PromptFragment};
use hellox_config::load_or_default;

use crate::output_styles::{
    discover_output_styles, format_output_style_detail, format_output_style_list, load_output_style,
};
use crate::personas::{
    discover_personas, format_persona_detail, format_persona_list, load_persona,
};
use crate::prompt_fragments::{
    discover_prompt_fragments, format_prompt_fragment_detail, format_prompt_fragment_list,
    load_prompt_fragment,
};
use crate::repl::ReplMetadata;
use crate::style_panels::{
    render_output_style_panel, render_persona_panel, render_prompt_fragment_panel,
};

use super::commands::{OutputStyleCommand, PersonaCommand, PromptFragmentCommand};

pub(super) fn handle_output_style_command(
    command: OutputStyleCommand,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    let config = runtime_config(metadata);

    match command {
        OutputStyleCommand::Panel { style_name } => Ok(
            match render_output_style_panel(
                &metadata.config_path,
                session.working_directory(),
                config.output_style.default.as_deref(),
                session.output_style_name(),
                style_name.as_deref(),
            ) {
                Ok(panel) => panel,
                Err(error) => format!("Unable to render output style panel: {error}"),
            },
        ),
        OutputStyleCommand::Current | OutputStyleCommand::List => {
            match discover_output_styles(session.working_directory()) {
                Ok(styles) => Ok(format_output_style_overview(
                    &styles,
                    session,
                    config.output_style.default.as_deref(),
                )),
                Err(error) => Ok(format!("Unable to inspect output styles: {error}")),
            }
        }
        OutputStyleCommand::Show { style_name } => {
            let Some(style_name) = style_name
                .or_else(|| session.output_style_name().map(ToString::to_string))
                .or_else(|| config.output_style.default.clone())
            else {
                return Ok("Usage: /output-style show <name>".to_string());
            };

            match load_output_style(&style_name, session.working_directory()) {
                Ok(style) => Ok(format_output_style_detail(
                    &style,
                    config.output_style.default.as_deref() == Some(style.name.as_str()),
                    session.output_style_name() == Some(style.name.as_str()),
                )),
                Err(error) => Ok(format!(
                    "Unable to load output style `{style_name}`: {error}"
                )),
            }
        }
        OutputStyleCommand::Use { style_name: None } => {
            Ok("Usage: /output-style use <name>".to_string())
        }
        OutputStyleCommand::Use {
            style_name: Some(style_name),
        } => match load_output_style(&style_name, session.working_directory()) {
            Ok(style) => {
                session.set_output_style(Some(OutputStylePrompt {
                    name: style.name.clone(),
                    prompt: style.prompt,
                }))?;
                Ok(format!(
                    "Active output style set to `{}` for the current session.",
                    style.name
                ))
            }
            Err(error) => Ok(format!(
                "Unable to load output style `{style_name}`: {error}"
            )),
        },
        OutputStyleCommand::Clear => match session.output_style_name() {
            Some(active_style) => {
                let active_style = active_style.to_string();
                session.set_output_style(None)?;
                Ok(format!(
                    "Cleared active output style `{active_style}` for the current session."
                ))
            }
            None => Ok("No active output style is set for the current session.".to_string()),
        },
        OutputStyleCommand::Help => Ok(output_style_help_text().to_string()),
    }
}

pub(super) fn handle_persona_command(
    command: PersonaCommand,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    let config = runtime_config(metadata);

    match command {
        PersonaCommand::Panel { persona_name } => Ok(
            match render_persona_panel(
                &metadata.config_path,
                session.working_directory(),
                config.prompt.persona.as_deref(),
                session.persona_name(),
                persona_name.as_deref(),
            ) {
                Ok(panel) => panel,
                Err(error) => format!("Unable to render persona panel: {error}"),
            },
        ),
        PersonaCommand::Current | PersonaCommand::List => {
            match discover_personas(session.working_directory()) {
                Ok(personas) => Ok(format_persona_overview(
                    &personas,
                    session,
                    config.prompt.persona.as_deref(),
                )),
                Err(error) => Ok(format!("Unable to inspect personas: {error}")),
            }
        }
        PersonaCommand::Show { persona_name } => {
            let Some(persona_name) = persona_name
                .or_else(|| session.persona_name().map(ToString::to_string))
                .or_else(|| config.prompt.persona.clone())
            else {
                return Ok("Usage: /persona show <name>".to_string());
            };

            match load_persona(&persona_name, session.working_directory()) {
                Ok(persona) => Ok(format_persona_detail(
                    &persona,
                    config.prompt.persona.as_deref(),
                    session.persona_name(),
                )),
                Err(error) => Ok(format!("Unable to load persona `{persona_name}`: {error}")),
            }
        }
        PersonaCommand::Use { persona_name: None } => Ok("Usage: /persona use <name>".to_string()),
        PersonaCommand::Use {
            persona_name: Some(persona_name),
        } => match load_persona(&persona_name, session.working_directory()) {
            Ok(persona) => {
                session.set_persona(Some(PersonaPrompt {
                    name: persona.name.clone(),
                    prompt: persona.prompt,
                }))?;
                Ok(format!(
                    "Active persona set to `{}` for the current session.",
                    persona.name
                ))
            }
            Err(error) => Ok(format!("Unable to load persona `{persona_name}`: {error}")),
        },
        PersonaCommand::Clear => match session.persona_name() {
            Some(active_persona) => {
                let active_persona = active_persona.to_string();
                session.set_persona(None)?;
                Ok(format!(
                    "Cleared active persona `{active_persona}` for the current session."
                ))
            }
            None => Ok("No active persona is set for the current session.".to_string()),
        },
        PersonaCommand::Help => Ok(persona_help_text().to_string()),
    }
}

pub(super) fn handle_prompt_fragment_command(
    command: PromptFragmentCommand,
    session: &mut AgentSession,
    metadata: &ReplMetadata,
) -> Result<String> {
    let config = runtime_config(metadata);

    match command {
        PromptFragmentCommand::Panel { fragment_name } => Ok(
            match render_prompt_fragment_panel(
                &metadata.config_path,
                session.working_directory(),
                &config.prompt.fragments,
                session.prompt_fragment_names(),
                fragment_name.as_deref(),
            ) {
                Ok(panel) => panel,
                Err(error) => format!("Unable to render prompt fragment panel: {error}"),
            },
        ),
        PromptFragmentCommand::Current | PromptFragmentCommand::List => {
            match discover_prompt_fragments(session.working_directory()) {
                Ok(fragments) => Ok(format_prompt_fragment_overview(
                    &fragments,
                    session,
                    &config.prompt.fragments,
                )),
                Err(error) => Ok(format!("Unable to inspect prompt fragments: {error}")),
            }
        }
        PromptFragmentCommand::Show { fragment_name } => {
            let Some(fragment_name) = fragment_name
                .or_else(|| session.prompt_fragment_names().first().cloned())
                .or_else(|| config.prompt.fragments.first().cloned())
            else {
                return Ok("Usage: /fragment show <name>".to_string());
            };

            match load_prompt_fragment(&fragment_name, session.working_directory()) {
                Ok(fragment) => Ok(format_prompt_fragment_detail(
                    &fragment,
                    &config.prompt.fragments,
                    session.prompt_fragment_names(),
                )),
                Err(error) => Ok(format!(
                    "Unable to load prompt fragment `{fragment_name}`: {error}"
                )),
            }
        }
        PromptFragmentCommand::Use { fragment_names } if fragment_names.is_empty() => {
            Ok("Usage: /fragment use <name> [name...]".to_string())
        }
        PromptFragmentCommand::Use { fragment_names } => {
            let prompt_fragments = fragment_names
                .iter()
                .map(|fragment_name| {
                    load_prompt_fragment(fragment_name, session.working_directory()).map(
                        |fragment| PromptFragment {
                            name: fragment.name,
                            prompt: fragment.prompt,
                        },
                    )
                })
                .collect::<Result<Vec<_>>>();

            match prompt_fragments {
                Ok(prompt_fragments) => {
                    session.set_prompt_fragments(prompt_fragments)?;
                    Ok(format!(
                        "Active prompt fragments set to `{}` for the current session.",
                        render_names(session.prompt_fragment_names())
                    ))
                }
                Err(error) => Ok(format!("Unable to load prompt fragments: {error}")),
            }
        }
        PromptFragmentCommand::Clear => {
            if session.prompt_fragment_names().is_empty() {
                Ok("No active prompt fragments are set for the current session.".to_string())
            } else {
                let active = render_names(session.prompt_fragment_names());
                session.set_prompt_fragments(Vec::new())?;
                Ok(format!(
                    "Cleared active prompt fragments `{active}` for the current session."
                ))
            }
        }
        PromptFragmentCommand::Help => Ok(prompt_fragment_help_text().to_string()),
    }
}

fn runtime_config(metadata: &ReplMetadata) -> hellox_config::HelloxConfig {
    load_or_default(Some(metadata.config_path.clone())).unwrap_or_else(|_| metadata.config.clone())
}

fn format_output_style_overview(
    styles: &[crate::output_styles::OutputStyleDefinition],
    session: &AgentSession,
    default_style: Option<&str>,
) -> String {
    format!(
        "active_output_style: {}\ndefault_output_style: {}\nworkspace_root: {}\n\n{}",
        session.output_style_name().unwrap_or("(none)"),
        default_style.unwrap_or("(none)"),
        normalize_path(session.working_directory()),
        format_output_style_list(styles, default_style)
    )
}

fn format_persona_overview(
    personas: &[crate::personas::PersonaDefinition],
    session: &AgentSession,
    default_persona: Option<&str>,
) -> String {
    format!(
        "active_persona: {}\ndefault_persona: {}\nworkspace_root: {}\n\n{}",
        session.persona_name().unwrap_or("(none)"),
        default_persona.unwrap_or("(none)"),
        normalize_path(session.working_directory()),
        format_persona_list(personas, default_persona, session.persona_name())
    )
}

fn format_prompt_fragment_overview(
    fragments: &[crate::prompt_fragments::PromptFragmentDefinition],
    session: &AgentSession,
    default_fragments: &[String],
) -> String {
    format!(
        "active_prompt_fragments: {}\ndefault_prompt_fragments: {}\nworkspace_root: {}\n\n{}",
        render_names(session.prompt_fragment_names()),
        render_names(default_fragments),
        normalize_path(session.working_directory()),
        format_prompt_fragment_list(
            fragments,
            default_fragments,
            session.prompt_fragment_names(),
        )
    )
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn render_names(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    }
}

fn output_style_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  /output-style              Show active, default, and discovered output styles\n",
        "  /output-style panel [name] Show an output-style dashboard or inspect one style\n",
        "  /output-style list         List discovered output styles\n",
        "  /output-style show <name>  Show a style prompt\n",
        "  /output-style use <name>   Apply a style to the current session\n",
        "  /output-style clear        Clear the active session style"
    )
}

fn persona_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  /persona             Show active, default, and discovered personas\n",
        "  /persona panel [name] Show a persona dashboard or inspect one persona\n",
        "  /persona list        List discovered personas\n",
        "  /persona show <name> Show a persona prompt\n",
        "  /persona use <name>  Apply a persona to the current session\n",
        "  /persona clear       Clear the active session persona"
    )
}

fn prompt_fragment_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  /fragment                   Show active, default, and discovered prompt fragments\n",
        "  /fragment panel [name]      Show a prompt-fragment dashboard or inspect one fragment\n",
        "  /fragment list              List discovered prompt fragments\n",
        "  /fragment show <name>       Show a prompt fragment\n",
        "  /fragment use <name> [...]  Apply one or more prompt fragments to the current session\n",
        "  /fragment clear             Clear active session prompt fragments\n",
        "  /prompt-fragment ...        Alias for /fragment"
    )
}
