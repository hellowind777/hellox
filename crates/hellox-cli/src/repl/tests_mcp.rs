use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use hellox_agent::{default_tool_registry, AgentOptions, AgentSession, GatewayClient};
use hellox_config::{load_or_default, HelloxConfig, PermissionMode};

use super::commands::{McpCommand, ReplCommand};
use super::format::{config_text, help_text};
use super::{handle_repl_input, ReplAction, ReplMetadata};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-repl-mcp-{suffix}"));
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
        plugins_root: root.join("plugins"),
        sessions_root: root.join("sessions"),
        shares_root: root.join("shares"),
    }
}

#[test]
fn parse_mcp_commands() {
    assert_eq!(
        super::commands::parse_command("/mcp"),
        Some(ReplCommand::Mcp(McpCommand::List))
    );
    assert_eq!(
        super::commands::parse_command("/mcp show filesystem"),
        Some(ReplCommand::Mcp(McpCommand::Show {
            server_name: Some(String::from("filesystem"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp panel filesystem"),
        Some(ReplCommand::Mcp(McpCommand::Panel {
            server_name: Some(String::from("filesystem"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp tools filesystem"),
        Some(ReplCommand::Mcp(McpCommand::Tools {
            server_name: Some(String::from("filesystem"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp add stdio filesystem npx server-filesystem"),
        Some(ReplCommand::Mcp(McpCommand::AddStdio {
            server_name: Some(String::from("filesystem")),
            command: Some(String::from("npx")),
            args: vec![String::from("server-filesystem")],
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp call filesystem read_file {\"path\":\"README.md\"}"),
        Some(ReplCommand::Mcp(McpCommand::Call {
            server_name: Some(String::from("filesystem")),
            tool_name: Some(String::from("read_file")),
            input: Some(String::from("{\"path\":\"README.md\"}")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp add sse docs https://example.test/sse"),
        Some(ReplCommand::Mcp(McpCommand::AddSse {
            server_name: Some(String::from("docs")),
            url: Some(String::from("https://example.test/sse"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp prompts docs"),
        Some(ReplCommand::Mcp(McpCommand::Prompts {
            server_name: Some(String::from("docs"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp get-prompt docs reviewer {\"audience\":\"dev\"}"),
        Some(ReplCommand::Mcp(McpCommand::GetPrompt {
            server_name: Some(String::from("docs")),
            prompt_name: Some(String::from("reviewer")),
            input: Some(String::from("{\"audience\":\"dev\"}")),
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp auth set-token docs token-123"),
        Some(ReplCommand::Mcp(McpCommand::AuthSetToken {
            server_name: Some(String::from("docs")),
            bearer_token: Some(String::from("token-123")),
        }))
    );
    assert_eq!(
        super::commands::parse_command(
            "/mcp auth oauth-set docs client-123 https://auth.example.test/authorize https://auth.example.test/token http://127.0.0.1:8910/callback openid profile"
        ),
        Some(ReplCommand::Mcp(McpCommand::AuthOauthSet {
            server_name: Some(String::from("docs")),
            client_id: Some(String::from("client-123")),
            authorize_url: Some(String::from("https://auth.example.test/authorize")),
            token_url: Some(String::from("https://auth.example.test/token")),
            redirect_url: Some(String::from("http://127.0.0.1:8910/callback")),
            scopes: vec![String::from("openid"), String::from("profile")],
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp registry show ac.tandem/docs-mcp"),
        Some(ReplCommand::Mcp(McpCommand::RegistryShow {
            name: Some(String::from("ac.tandem/docs-mcp"))
        }))
    );
    assert_eq!(
        super::commands::parse_command("/mcp disable filesystem"),
        Some(ReplCommand::Mcp(McpCommand::Disable {
            server_name: Some(String::from("filesystem"))
        }))
    );
}

#[test]
fn help_text_lists_mcp_commands() {
    let text = help_text();
    assert!(text.contains("/mcp"));
    assert!(text.contains("/mcp panel [name]"));
    assert!(text.contains("/mcp show <name>"));
    assert!(text.contains("/mcp tools <name>"));
    assert!(text.contains("/mcp prompts <name>"));
    assert!(text.contains("/mcp auth oauth-set <name>"));
    assert!(text.contains("/mcp registry show <name>"));
    assert!(text.contains("/mcp enable <name>"));
}

#[test]
fn handle_mcp_panel_renders_dashboard_and_detail() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    handle_repl_input(
        "/mcp add sse docs https://example.test/sse",
        &mut session,
        &metadata,
    )
    .expect("mcp add sse");

    let list =
        super::mcp_actions::handle_mcp_command(McpCommand::Panel { server_name: None }, &metadata)
            .expect("render mcp list panel");
    assert!(list.contains("MCP panel"));
    assert!(list.contains("hellox mcp panel docs"));
    assert!(list.contains("/mcp panel [server-name]"));

    let detail = super::mcp_actions::handle_mcp_command(
        McpCommand::Panel {
            server_name: Some(String::from("docs")),
        },
        &metadata,
    )
    .expect("render mcp detail panel");
    assert!(detail.contains("MCP server panel: docs"));
    assert!(detail.contains("transport   : sse"));
    assert!(detail.contains("/mcp auth show docs"));
}

#[test]
fn mcp_panel_selector_allows_numeric_selection() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);
    let driver = super::CliReplDriver::new();

    handle_repl_input(
        "/mcp add sse docs https://example.test/sse",
        &mut session,
        &metadata,
    )
    .expect("mcp add sse");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            assert_eq!(
                driver
                    .handle_repl_input_async("/mcp panel", &mut session, &metadata)
                    .await
                    .expect("open mcp panel"),
                ReplAction::Continue
            );

            match driver.selector_context() {
                Some(super::SelectorContext::McpPanelList { server_names }) => {
                    assert_eq!(server_names, vec!["docs".to_string()]);
                }
                other => panic!("expected mcp selector context, got {other:?}"),
            }

            assert_eq!(
                driver
                    .handle_repl_input_async("1", &mut session, &metadata)
                    .await
                    .expect("select server"),
                ReplAction::Continue
            );
            assert!(driver.selector_context().is_none());
        });
}

#[test]
fn handle_mcp_stdio_add_persists_config_and_config_view_reloads_it() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    let action = handle_repl_input(
        "/mcp add stdio filesystem npx @modelcontextprotocol/server-filesystem",
        &mut session,
        &metadata,
    )
    .expect("mcp add stdio");
    assert_eq!(action, ReplAction::Continue);

    let config = load_or_default(Some(metadata.config_path.clone())).expect("load config");
    let server = config
        .mcp
        .servers
        .get("filesystem")
        .expect("filesystem server");
    assert!(server.enabled);
    assert_eq!(server.transport.kind(), "stdio");

    let text = config_text(&metadata);
    assert!(text.contains("filesystem"));
}

#[test]
fn handle_mcp_enable_disable_and_remove_update_config() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    handle_repl_input(
        "/mcp add sse docs https://example.test/sse",
        &mut session,
        &metadata,
    )
    .expect("mcp add sse");

    assert_eq!(
        handle_repl_input("/mcp disable docs", &mut session, &metadata).expect("mcp disable"),
        ReplAction::Continue
    );
    let disabled = load_or_default(Some(metadata.config_path.clone())).expect("reload config");
    assert!(
        !disabled
            .mcp
            .servers
            .get("docs")
            .expect("docs server")
            .enabled
    );

    assert_eq!(
        handle_repl_input("/mcp enable docs", &mut session, &metadata).expect("mcp enable"),
        ReplAction::Continue
    );
    let enabled = load_or_default(Some(metadata.config_path.clone())).expect("reload config");
    assert!(
        enabled
            .mcp
            .servers
            .get("docs")
            .expect("docs server")
            .enabled
    );

    assert_eq!(
        handle_repl_input("/mcp remove docs", &mut session, &metadata).expect("mcp remove"),
        ReplAction::Continue
    );
    let removed = load_or_default(Some(metadata.config_path.clone())).expect("reload config");
    assert!(!removed.mcp.servers.contains_key("docs"));
}

#[test]
fn handle_mcp_oauth_set_persists_server_oauth_config() {
    let root = temp_dir();
    let mut session = session(root.clone());
    let metadata = metadata(&root);

    handle_repl_input(
        "/mcp add sse docs https://example.test/mcp",
        &mut session,
        &metadata,
    )
    .expect("mcp add sse");

    assert_eq!(
        handle_repl_input(
            "/mcp auth oauth-set docs client-123 https://auth.example.test/authorize https://auth.example.test/token http://127.0.0.1:8910/callback openid profile",
            &mut session,
            &metadata,
        )
        .expect("mcp auth oauth-set"),
        ReplAction::Continue
    );

    let config = load_or_default(Some(metadata.config_path.clone())).expect("reload config");
    let oauth = config
        .mcp
        .servers
        .get("docs")
        .and_then(|server| server.oauth.as_ref())
        .expect("docs oauth");
    assert_eq!(oauth.client_id, "client-123");
    assert_eq!(
        oauth.scopes,
        vec![String::from("openid"), String::from("profile")]
    );
}
