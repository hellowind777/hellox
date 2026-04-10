use std::path::PathBuf;

use clap::Parser;

use crate::cli_types::{
    Cli, Commands, OutputStyleCommands, PersonaCommands, PromptFragmentCommands,
};

#[test]
fn parses_style_panel_commands() {
    let output_style = Cli::try_parse_from([
        "hellox",
        "output-style",
        "panel",
        "reviewer",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse output-style panel");
    let persona = Cli::try_parse_from([
        "hellox",
        "persona",
        "panel",
        "reviewer",
        "--config",
        "config/custom.toml",
    ])
    .expect("parse persona panel");
    let fragment = Cli::try_parse_from([
        "hellox",
        "prompt-fragment",
        "panel",
        "safety",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse prompt-fragment panel");

    match output_style.command {
        Some(Commands::OutputStyle {
            command:
                OutputStyleCommands::Panel {
                    style_name,
                    cwd,
                    config,
                },
        }) => {
            assert_eq!(style_name, Some(String::from("reviewer")));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
            assert_eq!(config, None);
        }
        other => panic!("unexpected output-style panel command: {other:?}"),
    }

    match persona.command {
        Some(Commands::Persona {
            command:
                PersonaCommands::Panel {
                    persona_name,
                    cwd,
                    config,
                },
        }) => {
            assert_eq!(persona_name, Some(String::from("reviewer")));
            assert_eq!(cwd, None);
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
        }
        other => panic!("unexpected persona panel command: {other:?}"),
    }

    match fragment.command {
        Some(Commands::PromptFragment {
            command:
                PromptFragmentCommands::Panel {
                    fragment_name,
                    cwd,
                    config,
                },
        }) => {
            assert_eq!(fragment_name, Some(String::from("safety")));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
            assert_eq!(config, None);
        }
        other => panic!("unexpected prompt-fragment panel command: {other:?}"),
    }
}

#[test]
fn parses_persona_commands() {
    let list = Cli::try_parse_from(["hellox", "persona", "list", "--cwd", "workspace/app"])
        .expect("parse persona list");
    let set_default = Cli::try_parse_from([
        "hellox",
        "persona",
        "set-default",
        "reviewer",
        "--config",
        "config/custom.toml",
    ])
    .expect("parse persona set-default");

    match list.command {
        Some(Commands::Persona {
            command: PersonaCommands::List { cwd, config },
        }) => {
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
            assert_eq!(config, None);
        }
        other => panic!("unexpected persona list command: {other:?}"),
    }

    match set_default.command {
        Some(Commands::Persona {
            command:
                PersonaCommands::SetDefault {
                    persona_name,
                    cwd,
                    config,
                },
        }) => {
            assert_eq!(persona_name, "reviewer");
            assert_eq!(cwd, None);
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
        }
        other => panic!("unexpected persona set-default command: {other:?}"),
    }
}

#[test]
fn parses_prompt_fragment_commands() {
    let show = Cli::try_parse_from([
        "hellox",
        "prompt-fragment",
        "show",
        "safety",
        "--cwd",
        "workspace/app",
    ])
    .expect("parse prompt-fragment show");
    let set_default = Cli::try_parse_from([
        "hellox",
        "prompt-fragment",
        "set-default",
        "safety",
        "checklist",
        "--config",
        "config/custom.toml",
    ])
    .expect("parse prompt-fragment set-default");

    match show.command {
        Some(Commands::PromptFragment {
            command:
                PromptFragmentCommands::Show {
                    fragment_name,
                    cwd,
                    config,
                },
        }) => {
            assert_eq!(fragment_name, Some(String::from("safety")));
            assert_eq!(cwd, Some(PathBuf::from("workspace/app")));
            assert_eq!(config, None);
        }
        other => panic!("unexpected prompt-fragment show command: {other:?}"),
    }

    match set_default.command {
        Some(Commands::PromptFragment {
            command:
                PromptFragmentCommands::SetDefault {
                    fragment_names,
                    cwd,
                    config,
                },
        }) => {
            assert_eq!(
                fragment_names,
                vec![String::from("safety"), String::from("checklist")]
            );
            assert_eq!(cwd, None);
            assert_eq!(config, Some(PathBuf::from("config/custom.toml")));
        }
        other => panic!("unexpected prompt-fragment set-default command: {other:?}"),
    }
}
