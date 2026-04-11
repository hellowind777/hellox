use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{load_or_default, HelloxConfig, PermissionMode};

use super::commands::{MarketplaceCommand, PluginCommand, ReplCommand};
use super::format::help_text;
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-plugin-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn session(root: PathBuf) -> AgentSession {
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

fn metadata(root: &PathBuf) -> ReplMetadata {
    ReplMetadata {
        config: HelloxConfig::default(),
        config_path: root.join(".hellox").join("config.toml"),
        memory_root: root.join("memory"),
        plugins_root: root.join(".hellox").join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

fn write_plugin_source(root: &PathBuf, plugin_id: &str) -> PathBuf {
    let source = root.join("plugin-source");
    fs::create_dir_all(source.join(".hellox-plugin")).expect("create plugin source");
    fs::write(
        source.join(".hellox-plugin").join("plugin.json"),
        format!(
            r#"{{
  "id": "{plugin_id}",
  "name": "Filesystem Plugin",
  "version": "0.1.0",
  "description": "Plugin used by REPL tests",
  "commands": ["plugin.inspect"],
  "skills": ["filesystem"],
  "hooks": ["pre_tool"]
}}"#
        ),
    )
    .expect("write plugin manifest");
    source
}

#[test]
fn parse_plugin_commands() {
    assert_eq!(
        super::commands::parse_command("/plugin"),
        Some(ReplCommand::Plugin(PluginCommand::List))
    );
    assert_eq!(
        super::commands::parse_command("/plugin install C:/plugins/filesystem --disabled"),
        Some(ReplCommand::Plugin(PluginCommand::Install {
            source: Some(String::from("C:/plugins/filesystem")),
            disabled: true,
        }))
    );
    assert_eq!(
        super::commands::parse_command("/plugin panel filesystem"),
        Some(ReplCommand::Plugin(PluginCommand::Panel {
            plugin_id: Some(String::from("filesystem"))
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/plugin marketplace add official https://plugins.example.test/index.json"
        ),
        Some(ReplCommand::Plugin(PluginCommand::Marketplace(
            MarketplaceCommand::Add {
                marketplace_name: Some(String::from("official")),
                url: Some(String::from("https://plugins.example.test/index.json")),
            },
        )))
    );
}

#[test]
fn help_text_lists_plugin_commands() {
    let text = help_text();
    assert!(text.contains("/plugin"));
    assert!(text.contains("/plugin panel [id]"));
    assert!(text.contains("/plugin show <id>"));
    assert!(text.contains("/plugin marketplace"));
}

#[test]
fn handle_plugin_panel_renders_dashboard_and_detail() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let source = write_plugin_source(&root, "filesystem");

    handle_repl_input(
        &format!(
            "/plugin install {}",
            source.display().to_string().replace('\\', "/")
        ),
        &mut session,
        &metadata,
    )
    .expect("plugin install");

    let list = super::plugin_actions::handle_plugin_command(
        PluginCommand::Panel { plugin_id: None },
        &metadata,
    )
    .expect("render plugin list panel");
    assert!(list.contains("Plugin panel"));
    assert!(list.contains("hellox plugin panel filesystem"));
    assert!(list.contains("/plugin panel [plugin-id]"));

    let detail = super::plugin_actions::handle_plugin_command(
        PluginCommand::Panel {
            plugin_id: Some(String::from("filesystem")),
        },
        &metadata,
    )
    .expect("render plugin detail panel");
    assert!(detail.contains("Plugin detail panel: filesystem"));
    assert!(detail.contains("plugin_id    : filesystem"));
    assert!(detail.contains("/plugin disable filesystem"));
}

#[test]
fn plugin_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let source = write_plugin_source(&root, "filesystem");
    let driver = super::CliReplDriver::new();

    handle_repl_input(
        &format!(
            "/plugin install {}",
            source.display().to_string().replace('\\', "/")
        ),
        &mut session,
        &metadata,
    )
    .expect("plugin install");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("/plugin panel", &mut session, &metadata)
                    .await
                    .expect("open plugin panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::PluginPanelList { plugin_ids }) => {
                    assert_eq!(plugin_ids, vec!["filesystem".to_string()]);
                }
                other => panic!("expected plugin selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("select plugin"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}

#[test]
fn handle_plugin_install_persists_config_and_copies_files() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let source = write_plugin_source(&root, "filesystem");

    let action = handle_repl_input(
        &format!(
            "/plugin install {}",
            source.display().to_string().replace('\\', "/")
        ),
        &mut session,
        &metadata,
    )
    .expect("plugin install");
    assert_eq!(action, ReplAction::Continue);

    let config = load_or_default(Some(metadata.config_path.clone())).expect("load config");
    let plugin = config
        .plugins
        .installed
        .get("filesystem")
        .expect("installed plugin");
    assert!(plugin.enabled);
    assert!(metadata.plugins_root.join("filesystem").exists());
}

#[test]
fn handle_plugin_enable_disable_remove_and_marketplace_management() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let source = write_plugin_source(&root, "filesystem");

    handle_repl_input(
        &format!(
            "/plugin install {}",
            source.display().to_string().replace('\\', "/")
        ),
        &mut session,
        &metadata,
    )
    .expect("plugin install");

    assert_eq!(
        handle_repl_input("/plugin disable filesystem", &mut session, &metadata)
            .expect("plugin disable"),
        ReplAction::Continue
    );
    let disabled = load_or_default(Some(metadata.config_path.clone())).expect("load disabled");
    assert!(
        !disabled
            .plugins
            .installed
            .get("filesystem")
            .expect("filesystem plugin")
            .enabled
    );

    assert_eq!(
        handle_repl_input(
            "/plugin marketplace add official https://plugins.example.test/index.json",
            &mut session,
            &metadata,
        )
        .expect("add marketplace"),
        ReplAction::Continue
    );
    let with_marketplace =
        load_or_default(Some(metadata.config_path.clone())).expect("load marketplace");
    assert!(with_marketplace
        .plugins
        .marketplaces
        .contains_key("official"));

    assert_eq!(
        handle_repl_input("/plugin remove filesystem", &mut session, &metadata)
            .expect("plugin remove"),
        ReplAction::Continue
    );
    let removed = load_or_default(Some(metadata.config_path.clone())).expect("load removed");
    assert!(!removed.plugins.installed.contains_key("filesystem"));
}
