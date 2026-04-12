use std::env;
use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_agent::{compact_messages, CompactMode, StoredSession};
use hellox_config::{
    default_config_path, load_or_default, memory_root, save_config, sessions_root, shares_root,
    McpOAuthConfig, McpScope,
};
use hellox_tools_mcp::{
    add_server, auth_backend_for_config_path, build_stdio_server, build_stream_server,
    call_tool as mcp_call_tool, clear_bearer_token, clear_server_oauth_account,
    exchange_server_oauth_authorization_code, format_auth_status, format_prompt_get,
    format_prompt_list, format_registry_detail, format_registry_list, format_resource_list,
    format_resource_read, format_server_detail, format_server_list, format_tool_call,
    format_tool_list, get_prompt as mcp_get_prompt, get_registry_server_latest, get_server,
    install_registry_server, list_prompts as mcp_list_prompts, list_registry_servers,
    list_resources as mcp_list_resources, list_tools as mcp_list_tools, parse_key_value_pairs,
    parse_prompt_arguments, parse_tool_call_arguments, read_resource as mcp_read_resource,
    refresh_server_oauth_access_token, remove_server, set_bearer_token, set_server_enabled,
    set_server_oauth, start_server_oauth_authorization, StreamTransportKind,
};

use crate::cli_types::{McpCommands, McpScopeValue};
use crate::cli_types::{MemoryCommands, SessionCommands};
use crate::diagnostics::{
    cost_text, doctor_text, gather_workspace_stats, stats_text, status_text, usage_text,
};
use crate::mcp_panel::render_mcp_panel;
use crate::memory::{
    archive_memories, capture_memory_from_snapshot, cluster_memories, decay_archived_memories,
    format_memory_archive_report, format_memory_cluster_report, format_memory_decay_report,
    format_memory_list, format_memory_prune_report, format_memory_search_results,
    list_archived_memories, list_memories, load_archived_memory, load_memory,
    memory_result_targets, prune_memories, search_archived_memories_ranked, search_memories_ranked,
    write_memory_from_snapshot_summary, MemoryArchiveOptions, MemoryClusterOptions,
    MemoryDecayOptions, MemoryPruneOptions,
};
use crate::memory_panel::render_memory_panel;
use crate::repl::output_localizer::localize_user_visible_output;
use crate::search::{format_search_results, merge_search_hits, search_memories, search_sessions};
use crate::session_panel::render_session_panel;
use crate::sessions::{format_session_detail, format_session_list, list_sessions, load_session};
use crate::startup::{resolve_app_language, resolve_default_app_language, AppLanguage};
use crate::transcript::{default_share_path, export_stored_session_markdown};

pub fn handle_memory_command(command: MemoryCommands) -> Result<()> {
    let language = resolve_default_app_language();
    match command {
        MemoryCommands::Panel {
            archived,
            memory_id,
        } => {
            print_localized(
                language,
                render_memory_panel(&memory_root(), archived, memory_id.as_deref())?,
            );
        }
        MemoryCommands::List { archived } => {
            let memories = if archived {
                list_archived_memories(&memory_root())?
            } else {
                list_memories(&memory_root())?
            };
            print_localized(language, format_memory_list(&memories));
        }
        MemoryCommands::Show {
            memory_id,
            archived,
        } => {
            let markdown = if archived {
                load_archived_memory(&memory_root(), &memory_id)?
            } else {
                load_memory(&memory_root(), &memory_id)?
            };
            print_localized(language, markdown);
        }
        MemoryCommands::Search {
            query,
            limit,
            archived,
        } => {
            let hits = if archived {
                search_archived_memories_ranked(&memory_root(), &query, limit)?
            } else {
                search_memories_ranked(&memory_root(), &query, limit)?
            };
            print_localized(language, format_memory_search_results(&query, &hits));
        }
        MemoryCommands::Clusters {
            archived,
            limit,
            min_jaccard,
            max_tokens,
            semantic,
        } => {
            let report = cluster_memories(
                &memory_root(),
                &MemoryClusterOptions {
                    archived,
                    limit,
                    min_jaccard,
                    max_tokens,
                    semantic,
                },
            )?;
            print_localized(language, format_memory_cluster_report(&report));
        }
        MemoryCommands::Prune {
            scope,
            older_than_days,
            keep_latest,
            apply,
        } => {
            let report = prune_memories(
                &memory_root(),
                &MemoryPruneOptions {
                    scope,
                    older_than_days,
                    keep_latest,
                    apply,
                },
            )?;
            print_localized(language, format_memory_prune_report(&report));
        }
        MemoryCommands::Archive {
            scope,
            older_than_days,
            keep_latest,
            apply,
        } => {
            let report = archive_memories(
                &memory_root(),
                &MemoryArchiveOptions {
                    scope,
                    older_than_days,
                    keep_latest,
                    apply,
                },
            )?;
            print_localized(language, format_memory_archive_report(&report));
        }
        MemoryCommands::Decay {
            scope,
            older_than_days,
            keep_latest,
            max_summary_lines,
            max_summary_chars,
            apply,
        } => {
            let report = decay_archived_memories(
                &memory_root(),
                &MemoryDecayOptions {
                    scope,
                    older_than_days,
                    keep_latest,
                    max_summary_lines,
                    max_summary_chars,
                    apply,
                },
            )?;
            print_localized(language, format_memory_decay_report(&report));
        }
        MemoryCommands::Capture {
            session_id,
            instructions,
        } => {
            let snapshot = load_session(&sessions_root(), &session_id)?;
            let result =
                capture_memory_from_snapshot(&snapshot, &memory_root(), instructions.as_deref())?;
            println!(
                "{}",
                memory_capture_text(
                    language,
                    compact_mode_label(result.mode, language),
                    &memory_result_targets(&result),
                )
            );
        }
    }

    Ok(())
}

pub fn handle_session_command(command: SessionCommands) -> Result<()> {
    let language = resolve_default_app_language();
    match command {
        SessionCommands::Panel { session_id } => {
            print_localized(
                language,
                render_session_panel(&sessions_root(), session_id.as_deref())?,
            );
        }
        SessionCommands::List => {
            let sessions = list_sessions(&sessions_root())?;
            print_localized(language, format_session_list(&sessions));
        }
        SessionCommands::Show { session_id } => {
            let snapshot = load_session(&sessions_root(), &session_id)?;
            print_localized(language, format_session_detail(&snapshot));
        }
        SessionCommands::Compact {
            session_id,
            instructions,
        } => {
            let mut stored = StoredSession::load(&session_id)?;
            let mut messages = stored.restore_messages();
            let transcript = messages.clone();
            let result = compact_messages(&mut messages, instructions.as_deref());
            let memory_result = write_memory_from_snapshot_summary(
                &stored.snapshot,
                &memory_root(),
                &result,
                instructions.as_deref(),
                Some(transcript.as_slice()),
            )?;
            stored.save(&messages)?;
            println!(
                "{}",
                compacted_session_text(
                    language,
                    &session_id,
                    compact_mode_label(result.mode, language),
                    result.original_message_count,
                    result.retained_message_count,
                    &memory_result_targets(&memory_result),
                )
            );
        }
        SessionCommands::Share { session_id, output } => {
            let snapshot = load_session(&sessions_root(), &session_id)?;
            let destination = output.unwrap_or_else(|| {
                default_share_path(&shares_root(), Some(snapshot.session_id.as_str()))
            });
            export_stored_session_markdown(&snapshot, &destination)?;
            println!(
                "{}",
                shared_transcript_text(
                    language,
                    &destination.display().to_string().replace('\\', "/"),
                )
            );
        }
    }

    Ok(())
}

pub fn handle_search(query: String, limit: usize) -> Result<()> {
    let language = resolve_default_app_language();
    let session_hits = search_sessions(&sessions_root(), &query, limit)?;
    let memory_hits = search_memories(&memory_root(), &query, limit)?;
    let hits = merge_search_hits(limit, vec![session_hits, memory_hits]);
    print_localized(language, format_search_results(&query, &hits));
    Ok(())
}

pub fn handle_doctor_command() -> Result<()> {
    let config_path = default_config_path();
    let config = load_or_default(Some(config_path.clone()))?;
    let workspace_root = env::current_dir()?;
    println!(
        "{}",
        doctor_text(
            &workspace_root,
            &config_path,
            &config,
            resolve_app_language(&config)
        )?
    );
    Ok(())
}

pub fn handle_status_command() -> Result<()> {
    let config_path = default_config_path();
    let config = load_or_default(Some(config_path.clone()))?;
    let workspace_root = env::current_dir()?;
    let stats = gather_workspace_stats(&workspace_root)?;
    println!(
        "{}",
        status_text(
            &workspace_root,
            &config_path,
            &config,
            &stats,
            resolve_app_language(&config)
        )
    );
    Ok(())
}

pub fn handle_usage_command() -> Result<()> {
    let workspace_root = env::current_dir()?;
    let config = load_or_default(Some(default_config_path()))?;
    let stats = gather_workspace_stats(&workspace_root)?;
    println!("{}", usage_text(&stats, resolve_app_language(&config)));
    Ok(())
}

pub fn handle_stats_command() -> Result<()> {
    let workspace_root = env::current_dir()?;
    let config = load_or_default(Some(default_config_path()))?;
    let stats = gather_workspace_stats(&workspace_root)?;
    println!("{}", stats_text(&stats, resolve_app_language(&config)));
    Ok(())
}

pub fn handle_cost_command() -> Result<()> {
    let workspace_root = env::current_dir()?;
    let config_path = default_config_path();
    let config = load_or_default(Some(config_path))?;
    let stats = gather_workspace_stats(&workspace_root)?;
    println!(
        "{}",
        cost_text(&stats, &config, resolve_app_language(&config))
    );
    Ok(())
}

pub fn handle_mcp_command(command: McpCommands) -> Result<()> {
    let config_path = default_config_path();
    let mut config = load_or_default(Some(config_path.clone()))?;
    let language = resolve_app_language(&config);
    let auth_backend = auth_backend_for_config_path(&config_path);

    match command {
        McpCommands::Panel { server_name } => {
            print_localized(
                language,
                render_mcp_panel(&config_path, &config, server_name.as_deref())?,
            );
        }
        McpCommands::List => {
            print_localized(language, format_server_list(&config));
        }
        McpCommands::Show { server_name } => {
            print_localized(
                language,
                format_server_detail(&server_name, get_server(&config, &server_name)?),
            );
        }
        McpCommands::Tools { server_name } => {
            let server = get_server(&config, &server_name)?;
            let result = mcp_list_tools(&auth_backend, &server_name, server)?;
            print_localized(language, format_tool_list(&server_name, &result));
        }
        McpCommands::Call {
            server_name,
            tool_name,
            input,
        } => {
            let server = get_server(&config, &server_name)?;
            let arguments = parse_tool_call_arguments(input.as_deref())?;
            let result = mcp_call_tool(&auth_backend, &server_name, server, &tool_name, arguments)?;
            print_localized(
                language,
                format_tool_call(&server_name, &tool_name, &result),
            );
        }
        McpCommands::Resources { server_name } => {
            let server = get_server(&config, &server_name)?;
            let result = mcp_list_resources(&auth_backend, &server_name, server)?;
            print_localized(language, format_resource_list(&server_name, &result));
        }
        McpCommands::Prompts { server_name } => {
            let server = get_server(&config, &server_name)?;
            let result = mcp_list_prompts(&auth_backend, &server_name, server)?;
            print_localized(language, format_prompt_list(&server_name, &result));
        }
        McpCommands::ReadResource { server_name, uri } => {
            let server = get_server(&config, &server_name)?;
            let result = mcp_read_resource(&auth_backend, &server_name, server, &uri)?;
            print_localized(language, format_resource_read(&server_name, &uri, &result));
        }
        McpCommands::GetPrompt {
            server_name,
            prompt_name,
            input,
        } => {
            let server = get_server(&config, &server_name)?;
            let arguments = parse_prompt_arguments(input.as_deref())?;
            let result =
                mcp_get_prompt(&auth_backend, &server_name, server, &prompt_name, arguments)?;
            print_localized(
                language,
                format_prompt_get(&server_name, &prompt_name, &result),
            );
        }
        McpCommands::AuthShow { server_name } => {
            let server = get_server(&config, &server_name)?;
            print_localized(
                language,
                format_auth_status(&auth_backend, &server_name, server)?,
            );
        }
        McpCommands::AuthSetToken {
            server_name,
            bearer_token,
        } => {
            let server = get_server(&config, &server_name)?;
            set_bearer_token(&auth_backend, &server_name, server, bearer_token)?;
            print_localized(
                language,
                format!("Stored MCP bearer token for `{server_name}`."),
            );
        }
        McpCommands::AuthClear { server_name } => {
            get_server(&config, &server_name)?;
            let removed = clear_bearer_token(&auth_backend, &server_name)?;
            if removed {
                print_localized(
                    language,
                    format!("Cleared MCP bearer token for `{server_name}`."),
                );
            } else {
                print_localized(
                    language,
                    format!("No stored MCP bearer token found for `{server_name}`."),
                );
            }
        }
        McpCommands::AuthOauthSet {
            server_name,
            client_id,
            authorize_url,
            token_url,
            redirect_url,
            provider,
            scopes,
            login_hint,
            account_id,
        } => {
            get_server(&config, &server_name)?;
            set_server_oauth(
                &mut config,
                &server_name,
                McpOAuthConfig {
                    provider,
                    client_id,
                    authorize_url,
                    token_url,
                    redirect_url,
                    scopes,
                    login_hint,
                    account_id,
                },
            )?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(
                language,
                format!(
                    "Configured MCP OAuth for `{server_name}` in `{}`.",
                    normalize_path(&config_path)
                ),
            );
        }
        McpCommands::AuthOauthStart { server_name } => {
            let server = get_server(&config, &server_name)?;
            let request = start_server_oauth_authorization(&server_name, server)?;
            print_localized(
                language,
                format!(
                    "server: {server_name}\nauthorization_url: {}\ncode_verifier: {}\nstate: {}",
                    request.authorization_url, request.code_verifier, request.state
                ),
            );
        }
        McpCommands::AuthOauthExchange {
            server_name,
            code,
            code_verifier,
        } => {
            let server = get_server(&config, &server_name)?;
            let account = exchange_server_oauth_authorization_code(
                &auth_backend,
                &server_name,
                server,
                &code,
                &code_verifier,
            )?;
            print_localized(
                language,
                format!(
                    "Stored MCP OAuth account `{}` for `{server_name}` (provider: {}).",
                    account.account_id, account.provider
                ),
            );
        }
        McpCommands::AuthOauthRefresh { server_name } => {
            let server = get_server(&config, &server_name)?;
            let account = refresh_server_oauth_access_token(&auth_backend, &server_name, server)?;
            print_localized(
                language,
                format!(
                    "Refreshed MCP OAuth account `{}` for `{server_name}`.",
                    account.account_id
                ),
            );
        }
        McpCommands::AuthOauthClear { server_name } => {
            let server = get_server(&config, &server_name)?;
            if clear_server_oauth_account(&auth_backend, &server_name, server)? {
                print_localized(
                    language,
                    format!(
                        "Cleared linked MCP OAuth account for `{server_name}`. OAuth client config remains in `{}`.",
                        normalize_path(&config_path)
                    ),
                );
            } else {
                print_localized(
                    language,
                    format!("No linked MCP OAuth account found for `{server_name}`."),
                );
            }
        }
        McpCommands::RegistryList { cursor, limit } => {
            let result = list_registry_servers(cursor.as_deref(), Some(limit))?;
            print_localized(language, format_registry_list(&result));
        }
        McpCommands::RegistryShow { name } => {
            let entry = get_registry_server_latest(&name)?;
            print_localized(language, format_registry_detail(&entry));
        }
        McpCommands::RegistryInstall {
            name,
            server_name,
            scope,
        } => {
            let result = install_registry_server(
                &mut config,
                &name,
                server_name.as_deref(),
                mcp_scope(scope),
            )?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(
                language,
                format!(
                    "Installed MCP registry server `{}` as `{}` using `{}` -> {}.",
                    result.registry_name,
                    result.installed_server_name,
                    result.transport,
                    result.url
                ),
            );
        }
        McpCommands::AddStdio {
            server_name,
            command,
            args,
            env,
            cwd,
            scope,
            description,
        } => {
            let env = parse_key_value_pairs(&env, "env")?;
            let server = build_stdio_server(
                command,
                args,
                env,
                cwd.as_deref(),
                mcp_scope(scope),
                description,
            );
            add_server(&mut config, server_name.clone(), server)?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(
                language,
                format!(
                    "Added MCP server `{server_name}` to `{}`.",
                    normalize_path(&config_path)
                ),
            );
        }
        McpCommands::AddSse {
            server_name,
            url,
            headers,
            oauth_client_id,
            oauth_authorize_url,
            oauth_token_url,
            oauth_redirect_url,
            oauth_provider,
            oauth_scopes,
            oauth_login_hint,
            oauth_account_id,
            scope,
            description,
        } => {
            let headers = parse_key_value_pairs(&headers, "headers")?;
            let server = build_stream_server(
                StreamTransportKind::Sse,
                url,
                headers,
                mcp_scope(scope),
                description,
                mcp_oauth_from_parts(
                    oauth_provider,
                    oauth_client_id,
                    oauth_authorize_url,
                    oauth_token_url,
                    oauth_redirect_url,
                    oauth_scopes,
                    oauth_login_hint,
                    oauth_account_id,
                )?,
            )?;
            add_server(&mut config, server_name.clone(), server)?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(
                language,
                format!(
                    "Added MCP server `{server_name}` to `{}`.",
                    normalize_path(&config_path)
                ),
            );
        }
        McpCommands::AddWs {
            server_name,
            url,
            headers,
            oauth_client_id,
            oauth_authorize_url,
            oauth_token_url,
            oauth_redirect_url,
            oauth_provider,
            oauth_scopes,
            oauth_login_hint,
            oauth_account_id,
            scope,
            description,
        } => {
            let headers = parse_key_value_pairs(&headers, "headers")?;
            let server = build_stream_server(
                StreamTransportKind::Ws,
                url,
                headers,
                mcp_scope(scope),
                description,
                mcp_oauth_from_parts(
                    oauth_provider,
                    oauth_client_id,
                    oauth_authorize_url,
                    oauth_token_url,
                    oauth_redirect_url,
                    oauth_scopes,
                    oauth_login_hint,
                    oauth_account_id,
                )?,
            )?;
            add_server(&mut config, server_name.clone(), server)?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(
                language,
                format!(
                    "Added MCP server `{server_name}` to `{}`.",
                    normalize_path(&config_path)
                ),
            );
        }
        McpCommands::Enable { server_name } => {
            set_server_enabled(&mut config, &server_name, true)?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(language, format!("Enabled MCP server `{server_name}`."));
        }
        McpCommands::Disable { server_name } => {
            set_server_enabled(&mut config, &server_name, false)?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(language, format!("Disabled MCP server `{server_name}`."));
        }
        McpCommands::Remove { server_name } => {
            remove_server(&mut config, &server_name)?;
            save_config(Some(config_path.clone()), &config)?;
            print_localized(language, format!("Removed MCP server `{server_name}`."));
        }
    }

    Ok(())
}

fn compact_mode_label(mode: CompactMode, language: AppLanguage) -> &'static str {
    match (mode, language) {
        (CompactMode::Micro, AppLanguage::English) => "microcompact",
        (CompactMode::Full, AppLanguage::English) => "compact",
        (CompactMode::Micro, AppLanguage::SimplifiedChinese) => "微压缩",
        (CompactMode::Full, AppLanguage::SimplifiedChinese) => "压缩",
    }
}

fn memory_capture_text(language: AppLanguage, mode: &str, targets: &str) -> String {
    match language {
        AppLanguage::English => format!("Captured layered memory using {mode} mode. {targets}"),
        AppLanguage::SimplifiedChinese => format!("已使用 `{mode}` 模式捕获分层记忆。{targets}"),
    }
}

fn compacted_session_text(
    language: AppLanguage,
    session_id: &str,
    mode: &str,
    original_count: usize,
    retained_count: usize,
    targets: &str,
) -> String {
    match language {
        AppLanguage::English => format!(
            "Compacted session `{session_id}` in {mode} mode: {original_count} -> {retained_count} message(s). {targets}"
        ),
        AppLanguage::SimplifiedChinese => format!(
            "已使用 `{mode}` 模式压缩会话 `{session_id}`：{original_count} -> {retained_count} 条消息。{targets}"
        ),
    }
}

fn shared_transcript_text(language: AppLanguage, path: &str) -> String {
    match language {
        AppLanguage::English => format!("Shared transcript written to `{path}`."),
        AppLanguage::SimplifiedChinese => format!("已将转录导出到 `{path}`。"),
    }
}

fn print_localized(language: AppLanguage, text: impl Into<String>) {
    println!("{}", localize_user_visible_output(language, text.into()));
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn mcp_oauth_from_parts(
    provider: Option<String>,
    client_id: Option<String>,
    authorize_url: Option<String>,
    token_url: Option<String>,
    redirect_url: Option<String>,
    scopes: Vec<String>,
    login_hint: Option<String>,
    account_id: Option<String>,
) -> Result<Option<McpOAuthConfig>> {
    match (
        client_id,
        authorize_url,
        token_url,
        redirect_url,
        provider,
        login_hint,
        account_id,
        scopes,
    ) {
        (None, None, None, None, None, None, None, scopes) if scopes.is_empty() => Ok(None),
        (
            Some(client_id),
            Some(authorize_url),
            Some(token_url),
            Some(redirect_url),
            provider,
            login_hint,
            account_id,
            scopes,
        ) => Ok(Some(McpOAuthConfig {
            provider,
            client_id,
            authorize_url,
            token_url,
            redirect_url,
            scopes,
            login_hint,
            account_id,
        })),
        _ => Err(anyhow!(
            "MCP OAuth requires --oauth-client-id, --oauth-authorize-url, --oauth-token-url, and --oauth-redirect-url together."
        )),
    }
}

fn mcp_scope(value: McpScopeValue) -> McpScope {
    match value {
        McpScopeValue::User => McpScope::User,
        McpScopeValue::Project => McpScope::Project,
        McpScopeValue::Local => McpScope::Local,
        McpScopeValue::Dynamic => McpScope::Dynamic,
        McpScopeValue::Enterprise => McpScope::Enterprise,
        McpScopeValue::Managed => McpScope::Managed,
        McpScopeValue::Claudeai => McpScope::Claudeai,
    }
}
