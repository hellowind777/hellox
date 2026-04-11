use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{save_config, HelloxConfig, PermissionMode};

use super::commands::{OutputStyleCommand, PersonaCommand, PromptFragmentCommand, ReplCommand};
use super::format::help_text;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-style-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session_in(root: PathBuf) -> AgentSession {
    AgentSession::create(
        GatewayClient::new("http://127.0.0.1:7821"),
        default_tool_registry(),
        root.join(".hellox").join("config.toml"),
        root,
        "powershell",
        AgentOptions::default(),
        PermissionMode::BypassPermissions,
        None,
        None,
        false,
        None,
    )
}

fn metadata_in(root: &PathBuf) -> ReplMetadata {
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: root.join(".hellox").join("config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join(".hellox").join("plugins"),
        sessions_root: root.join(".hellox").join("sessions"),
        shares_root: root.join("shares"),
    }
}

fn write_persona(root: &PathBuf, persona_name: &str, prompt: &str) {
    let personas_root = root.join(".hellox").join("personas");
    fs::create_dir_all(&personas_root).expect("create personas root");
    fs::write(personas_root.join(format!("{persona_name}.md")), prompt).expect("write persona");
}

fn write_prompt_fragment(root: &PathBuf, fragment_name: &str, prompt: &str) {
    let fragments_root = root.join(".hellox").join("prompt-fragments");
    fs::create_dir_all(&fragments_root).expect("create fragments root");
    fs::write(fragments_root.join(format!("{fragment_name}.md")), prompt)
        .expect("write prompt fragment");
}

fn write_output_style(root: &PathBuf, style_name: &str, prompt: &str) {
    let styles_root = root.join(".hellox").join("output-styles");
    fs::create_dir_all(&styles_root).expect("create styles root");
    fs::write(styles_root.join(format!("{style_name}.md")), prompt).expect("write output style");
}

#[test]
fn parse_persona_and_fragment_commands() {
    assert_eq!(
        super::commands::parse_command("/output-style panel reviewer"),
        Some(ReplCommand::OutputStyle(OutputStyleCommand::Panel {
            style_name: Some(String::from("reviewer"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/persona"),
        Some(ReplCommand::Persona(PersonaCommand::Current))
    );
    assert_eq!(
        super::commands::parse_command("/persona panel reviewer"),
        Some(ReplCommand::Persona(PersonaCommand::Panel {
            persona_name: Some(String::from("reviewer"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/persona show reviewer"),
        Some(ReplCommand::Persona(PersonaCommand::Show {
            persona_name: Some(String::from("reviewer"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/fragment panel safety"),
        Some(ReplCommand::PromptFragment(PromptFragmentCommand::Panel {
            fragment_name: Some(String::from("safety"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/fragment use safety checklist"),
        Some(ReplCommand::PromptFragment(PromptFragmentCommand::Use {
            fragment_names: vec![String::from("safety"), String::from("checklist")]
        }))
    );
    assert_eq!(
        super::commands::parse_command("/prompt-fragment clear"),
        Some(ReplCommand::PromptFragment(PromptFragmentCommand::Clear))
    );
}

#[test]
fn help_text_lists_prompt_layer_commands() {
    let text = help_text();
    assert!(text.contains("/output-style panel [name]"));
    assert!(text.contains("/persona"));
    assert!(text.contains("/persona panel [name]"));
    assert!(text.contains("/fragment"));
    assert!(text.contains("/fragment panel [name]"));
}

#[test]
fn handle_output_style_panel_renders_dashboard_and_detail() {
    let root = temp_dir();
    write_output_style(&root, "reviewer", "Use terse review language.");
    let mut config = HelloxConfig::default();
    config.output_style.default = Some(String::from("reviewer"));
    save_config(Some(root.join(".hellox").join("config.toml")), &config).expect("save config");
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());

    let list = super::style_actions::handle_output_style_command(
        OutputStyleCommand::Panel { style_name: None },
        &mut session,
        &metadata,
    )
    .expect("render output-style list panel");
    assert!(list.contains("Output style panel"));
    assert!(list.contains("hellox output-style panel reviewer"));
    assert!(list.contains("/output-style panel [name]"));

    let detail = super::style_actions::handle_output_style_command(
        OutputStyleCommand::Panel {
            style_name: Some(String::from("reviewer")),
        },
        &mut session,
        &metadata,
    )
    .expect("render output-style detail panel");
    assert!(detail.contains("Output style panel: reviewer"));
    assert!(detail.contains("Prompt preview"));
    assert!(detail.contains("/output-style use reviewer"));
}

#[test]
fn output_style_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    write_output_style(&root, "reviewer", "Use terse review language.");
    save_config(
        Some(root.join(".hellox").join("config.toml")),
        &HelloxConfig::default(),
    )
    .expect("save config");
    let metadata = metadata_in(&root);
    let mut session = session_in(root);
    let driver = super::CliReplDriver::new();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("/output-style panel", &mut session, &metadata)
                    .await
                    .expect("open output-style panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::OutputStylePanelList { style_names }) => {
                    assert_eq!(style_names, vec!["reviewer".to_string()]);
                }
                other => panic!("expected output-style selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("select output style"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}

#[test]
fn handle_persona_commands_update_and_show_session_persona() {
    let root = temp_dir();
    write_persona(
        &root,
        "reviewer",
        "Prioritize risks, regressions, and missing tests.",
    );
    let mut config = HelloxConfig::default();
    config.prompt.persona = Some("reviewer".to_string());
    save_config(Some(root.join(".hellox").join("config.toml")), &config).expect("save config");
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());

    let summary = super::style_actions::handle_persona_command(
        PersonaCommand::Current,
        &mut session,
        &metadata,
    )
    .expect("summarize personas");
    assert!(summary.contains("default_persona: reviewer"));
    assert!(summary.contains("reviewer"));

    let detail = super::style_actions::handle_persona_command(
        PersonaCommand::Show { persona_name: None },
        &mut session,
        &metadata,
    )
    .expect("show default persona");
    assert!(detail.contains("name: reviewer"));
    assert!(detail.contains("kind: persona"));
    assert!(detail.contains("default: yes"));

    let action =
        handle_repl_input("/persona use reviewer", &mut session, &metadata).expect("use persona");
    assert_eq!(action, ReplAction::Continue);
    assert_eq!(session.persona_name(), Some("reviewer"));

    let cleared =
        handle_repl_input("/persona clear", &mut session, &metadata).expect("clear persona");
    assert_eq!(cleared, ReplAction::Continue);
    assert_eq!(session.persona_name(), None);

    let panel = super::style_actions::handle_persona_command(
        PersonaCommand::Panel {
            persona_name: Some(String::from("reviewer")),
        },
        &mut session,
        &metadata,
    )
    .expect("render persona panel");
    assert!(panel.contains("Persona panel: reviewer"));
    assert!(panel.contains("/persona use reviewer"));
}

#[test]
fn handle_fragment_commands_update_and_show_session_fragments() {
    let root = temp_dir();
    write_prompt_fragment(&root, "safety", "Call out security and safety risks.");
    write_prompt_fragment(&root, "checklist", "End with a concise checklist.");
    let mut config = HelloxConfig::default();
    config.prompt.fragments = vec![String::from("safety")];
    save_config(Some(root.join(".hellox").join("config.toml")), &config).expect("save config");
    let metadata = metadata_in(&root);
    let mut session = session_in(root.clone());

    let summary = super::style_actions::handle_prompt_fragment_command(
        PromptFragmentCommand::Current,
        &mut session,
        &metadata,
    )
    .expect("summarize fragments");
    assert!(summary.contains("default_prompt_fragments: safety"));
    assert!(summary.contains("safety"));

    let detail = super::style_actions::handle_prompt_fragment_command(
        PromptFragmentCommand::Show {
            fragment_name: None,
        },
        &mut session,
        &metadata,
    )
    .expect("show default fragment");
    assert!(detail.contains("name: safety"));
    assert!(detail.contains("kind: fragment"));
    assert!(detail.contains("default: yes"));

    let action = handle_repl_input("/fragment use safety checklist", &mut session, &metadata)
        .expect("use fragments");
    assert_eq!(action, ReplAction::Continue);
    assert_eq!(
        session.prompt_fragment_names(),
        &[String::from("safety"), String::from("checklist")]
    );

    let cleared = handle_repl_input("/prompt-fragment clear", &mut session, &metadata)
        .expect("clear fragments");
    assert_eq!(cleared, ReplAction::Continue);
    assert!(session.prompt_fragment_names().is_empty());

    let panel = super::style_actions::handle_prompt_fragment_command(
        PromptFragmentCommand::Panel {
            fragment_name: Some(String::from("safety")),
        },
        &mut session,
        &metadata,
    )
    .expect("render fragment panel");
    assert!(panel.contains("Prompt fragment panel: safety"));
    assert!(panel.contains("/fragment use safety"));
}
