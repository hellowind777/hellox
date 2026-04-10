use crate::command_types::{OutputStyleCommand, PersonaCommand, PromptFragmentCommand};

pub fn parse_output_style_command(remainder: &str) -> OutputStyleCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => OutputStyleCommand::Current,
        Some(action) if action == "panel" => OutputStyleCommand::Panel {
            style_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "list" => OutputStyleCommand::List,
        Some(action) if action == "show" => OutputStyleCommand::Show {
            style_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "use" => OutputStyleCommand::Use {
            style_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "clear" => OutputStyleCommand::Clear,
        Some(action) if action == "help" => OutputStyleCommand::Help,
        Some(_) => OutputStyleCommand::Help,
    }
}

pub fn parse_persona_command(remainder: &str) -> PersonaCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => PersonaCommand::Current,
        Some(action) if action == "panel" => PersonaCommand::Panel {
            persona_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "list" => PersonaCommand::List,
        Some(action) if action == "show" => PersonaCommand::Show {
            persona_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "use" => PersonaCommand::Use {
            persona_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "clear" => PersonaCommand::Clear,
        Some(action) if action == "help" => PersonaCommand::Help,
        Some(_) => PersonaCommand::Help,
    }
}

pub fn parse_prompt_fragment_command(remainder: &str) -> PromptFragmentCommand {
    let mut parts = remainder.split_whitespace();

    match parts.next().map(|part| part.to_ascii_lowercase()) {
        None => PromptFragmentCommand::Current,
        Some(action) if action == "panel" => PromptFragmentCommand::Panel {
            fragment_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "list" => PromptFragmentCommand::List,
        Some(action) if action == "show" => PromptFragmentCommand::Show {
            fragment_name: parts.next().map(ToString::to_string),
        },
        Some(action) if action == "use" => PromptFragmentCommand::Use {
            fragment_names: parts.map(ToString::to_string).collect(),
        },
        Some(action) if action == "clear" => PromptFragmentCommand::Clear,
        Some(action) if action == "help" => PromptFragmentCommand::Help,
        Some(_) => PromptFragmentCommand::Help,
    }
}
