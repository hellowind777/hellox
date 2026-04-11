use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_config::HelloxConfig;
use hellox_remote::{list_remote_environments, RemoteEnvironmentSummary, TeleportPlan};
use hellox_tui::{render_panel, render_selector, KeyValueRow, PanelSection, SelectorEntry};

use crate::style_command_support::normalize_path;

pub(crate) fn render_remote_env_panel(
    config_path: &Path,
    config: &HelloxConfig,
    environment_name: Option<&str>,
) -> Result<String> {
    let environment_name = environment_name
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match environment_name {
        Some(environment_name) => {
            render_remote_env_detail_panel(config_path, config, environment_name)
        }
        None => Ok(render_remote_env_list_panel(config_path, config)),
    }
}

pub(crate) fn remote_env_panel_names(config: &HelloxConfig) -> Vec<String> {
    let mut names = list_remote_environments(config)
        .into_iter()
        .map(|environment| environment.name)
        .collect::<Vec<_>>();
    names.sort();
    names
}

pub(crate) fn render_teleport_plan_panel(plan: &TeleportPlan) -> String {
    let metadata = vec![
        KeyValueRow::new("viewer", "direct_connect_plan"),
        KeyValueRow::new("environment", plan.environment_name.clone()),
        KeyValueRow::new("session_id", plan.session_id.clone()),
        KeyValueRow::new("model", plan.model.clone()),
        KeyValueRow::new("source", plan.source.clone()),
        KeyValueRow::new("enabled", yes_no(plan.enabled)),
    ];
    let sections = vec![
        PanelSection::new("Teleport plan lens", render_teleport_plan_lens(plan)),
        PanelSection::new(
            "Action palette",
            teleport_cli_palette(&plan.environment_name, &plan.session_id),
        ),
        PanelSection::new(
            "REPL palette",
            teleport_repl_palette(&plan.environment_name, &plan.session_id),
        ),
    ];

    render_panel(
        &format!("Teleport plan panel: {}", plan.environment_name),
        &metadata,
        &sections,
    )
}

fn render_remote_env_list_panel(config_path: &Path, config: &HelloxConfig) -> String {
    let environments = sorted_remote_environments(config);
    let enabled_count = environments
        .iter()
        .filter(|environment| environment.enabled)
        .count();
    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("environments", environments.len().to_string()),
        KeyValueRow::new("enabled", enabled_count.to_string()),
    ];
    let sections = vec![
        PanelSection::new(
            "Environment selector",
            render_remote_env_selector(&environments),
        ),
        PanelSection::new("Action palette", remote_env_list_cli_palette()),
        PanelSection::new("REPL palette", remote_env_list_repl_palette()),
    ];

    render_panel("Remote environment panel", &metadata, &sections)
}

fn render_remote_env_detail_panel(
    config_path: &Path,
    config: &HelloxConfig,
    environment_name: &str,
) -> Result<String> {
    let environment = sorted_remote_environments(config)
        .into_iter()
        .find(|environment| environment.name == environment_name)
        .ok_or_else(|| anyhow!("Remote environment `{environment_name}` was not found"))?;
    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("environment", environment.name.clone()),
        KeyValueRow::new("enabled", yes_no(environment.enabled)),
        KeyValueRow::new("server_url", environment.server_url.clone()),
        KeyValueRow::new("auth_source", auth_source(&environment)),
    ];
    let sections = vec![
        PanelSection::new("Environment lens", render_remote_env_lens(&environment)),
        PanelSection::new(
            "Action palette",
            remote_env_detail_cli_palette(&environment),
        ),
        PanelSection::new("REPL palette", remote_env_detail_repl_palette(&environment)),
    ];

    Ok(render_panel(
        &format!("Remote environment panel: {environment_name}"),
        &metadata,
        &sections,
    ))
}

fn sorted_remote_environments(config: &HelloxConfig) -> Vec<RemoteEnvironmentSummary> {
    let mut environments = list_remote_environments(config);
    environments.sort_by(|left, right| left.name.cmp(&right.name));
    environments
}

fn render_remote_env_selector(environments: &[RemoteEnvironmentSummary]) -> Vec<String> {
    let entries = environments
        .iter()
        .map(|environment| {
            SelectorEntry::new(
                environment.name.clone(),
                vec![
                    format!("server_url: {}", environment.server_url),
                    format!("auth_source: {}", auth_source(environment)),
                    format!(
                        "description: {}",
                        environment.description.as_deref().unwrap_or("(none)")
                    ),
                    format!("focus: `hellox remote-env panel {}`", environment.name),
                ],
            )
            .with_badge(status_label(environment.enabled))
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

fn render_remote_env_lens(environment: &RemoteEnvironmentSummary) -> Vec<String> {
    render_selector(&[SelectorEntry::new(
        environment.name.clone(),
        vec![
            format!("server_url: {}", environment.server_url),
            format!(
                "token_env: {}",
                environment.token_env.as_deref().unwrap_or("(none)")
            ),
            format!(
                "account_id: {}",
                environment.account_id.as_deref().unwrap_or("(none)")
            ),
            format!(
                "device_id: {}",
                environment.device_id.as_deref().unwrap_or("(none)")
            ),
            format!(
                "description: {}",
                environment.description.as_deref().unwrap_or("(none)")
            ),
        ],
    )
    .with_badge(status_label(environment.enabled))
    .selected(true)])
}

fn render_teleport_plan_lens(plan: &TeleportPlan) -> Vec<String> {
    render_selector(&[SelectorEntry::new(
        plan.session_id.clone(),
        vec![
            format!("server_url: {}", plan.server_url),
            format!("connect_url: {}", plan.connect_url),
            format!("auth_source: {}", plan.auth_source),
            format!(
                "token_env: {}",
                plan.token_env.as_deref().unwrap_or("(none)")
            ),
            format!(
                "account_id: {}",
                plan.account_id.as_deref().unwrap_or("(none)")
            ),
            format!(
                "device_id: {}",
                plan.device_id.as_deref().unwrap_or("(none)")
            ),
            format!("working_directory: {}", plan.working_directory),
        ],
    )
    .with_badge(status_label(plan.enabled))
    .selected(true)])
}

fn remote_env_list_cli_palette() -> Vec<String> {
    vec![
        "- open panel: `hellox remote-env panel <name>`".to_string(),
        "- show raw: `hellox remote-env show <name>`".to_string(),
        "- add profile: `hellox remote-env add <name> --url <url>`".to_string(),
        "- assistant viewer: `hellox assistant list --environment <name>`".to_string(),
    ]
}

fn remote_env_list_repl_palette() -> Vec<String> {
    vec![
        "- open panel: `/remote-env panel [name]`".to_string(),
        "- show raw: `/remote-env show <name>`".to_string(),
        "- numeric focus: render `/remote-env panel`, then enter `1..n`".to_string(),
        "- assistant viewer: `/assistant list <name>`".to_string(),
    ]
}

fn remote_env_detail_cli_palette(environment: &RemoteEnvironmentSummary) -> Vec<String> {
    let toggle = if environment.enabled {
        "disable"
    } else {
        "enable"
    };
    vec![
        "- back to list: `hellox remote-env panel`".to_string(),
        format!("- show raw: `hellox remote-env show {}`", environment.name),
        format!(
            "- teleport panel: `hellox teleport panel {} --session-id <session-id>`",
            environment.name
        ),
        format!(
            "- assistant viewer: `hellox assistant list --environment {}`",
            environment.name
        ),
        format!(
            "- toggle state: `hellox remote-env {toggle} {}`",
            environment.name
        ),
    ]
}

fn remote_env_detail_repl_palette(environment: &RemoteEnvironmentSummary) -> Vec<String> {
    let toggle = if environment.enabled {
        "disable"
    } else {
        "enable"
    };
    vec![
        "- back to list: `/remote-env panel`".to_string(),
        format!("- show raw: `/remote-env show {}`", environment.name),
        format!(
            "- teleport panel: `/teleport panel {} <session-id>`",
            environment.name
        ),
        format!("- assistant viewer: `/assistant list {}`", environment.name),
        format!(
            "- toggle state: `/remote-env {toggle} {}`",
            environment.name
        ),
    ]
}

fn teleport_cli_palette(environment_name: &str, session_id: &str) -> Vec<String> {
    vec![
        format!(
            "- launch direct-connect: `hellox teleport connect {environment_name} --session-id {session_id}`"
        ),
        format!("- environment panel: `hellox remote-env panel {environment_name}`"),
        format!(
            "- assistant viewer: `hellox assistant list --environment {environment_name}`"
        ),
    ]
}

fn teleport_repl_palette(environment_name: &str, session_id: &str) -> Vec<String> {
    vec![
        format!("- launch direct-connect: `/teleport connect {environment_name} {session_id}`"),
        format!("- environment panel: `/remote-env panel {environment_name}`"),
        format!("- assistant viewer: `/assistant list {environment_name}`"),
    ]
}

fn auth_source(environment: &RemoteEnvironmentSummary) -> String {
    if let Some(token_env) = environment.token_env.as_deref() {
        format!("env:{token_env}")
    } else if let Some(account_id) = environment.account_id.as_deref() {
        format!("account:{account_id}")
    } else {
        "none".to_string()
    }
}

fn status_label(enabled: bool) -> String {
    if enabled {
        "ENABLED".to_string()
    } else {
        "DISABLED".to_string()
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "YES".to_string()
    } else {
        "NO".to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use hellox_config::HelloxConfig;
    use hellox_remote::{
        add_remote_environment, build_remote_environment, build_teleport_plan, TeleportOverrides,
    };

    use super::*;

    #[test]
    fn remote_env_panel_renders_selector_and_detail() {
        let mut config = HelloxConfig::default();
        add_remote_environment(
            &mut config,
            "dev".to_string(),
            build_remote_environment(
                "https://remote.example.test".to_string(),
                Some("REMOTE_TOKEN".to_string()),
                Some("account-1".to_string()),
                Some("device-1".to_string()),
                Some("Shared dev cluster".to_string()),
            ),
        )
        .expect("add remote env");

        let list = render_remote_env_panel(Path::new("D:/repo/.hellox/config.toml"), &config, None)
            .expect("render remote env list");
        assert!(list.contains("Remote environment panel"));
        assert!(list.contains("== Environment selector =="));
        assert!(list.contains("hellox remote-env panel dev"));
        assert!(list.contains("/remote-env panel [name]"));

        let detail = render_remote_env_panel(
            Path::new("D:/repo/.hellox/config.toml"),
            &config,
            Some("dev"),
        )
        .expect("render remote env detail");
        assert!(detail.contains("Remote environment panel: dev"));
        assert!(detail.contains("auth_source"));
        assert!(detail.contains("hellox teleport panel dev --session-id <session-id>"));
        assert!(detail.contains("/assistant list dev"));
    }

    #[test]
    fn teleport_plan_panel_renders_follow_up_actions() {
        let mut config = HelloxConfig::default();
        add_remote_environment(
            &mut config,
            "dev".to_string(),
            build_remote_environment(
                "https://remote.example.test".to_string(),
                Some("REMOTE_TOKEN".to_string()),
                None,
                None,
                None,
            ),
        )
        .expect("add remote env");

        let plan = build_teleport_plan(
            &config,
            "dev",
            None,
            TeleportOverrides {
                session_id: Some("session-123".to_string()),
                model: Some("opus".to_string()),
                working_directory: Some("D:/workspace".to_string()),
            },
        )
        .expect("build teleport plan");

        let panel = render_teleport_plan_panel(&plan);
        assert!(panel.contains("Teleport plan panel: dev"));
        assert!(panel.contains("cc://remote.example.test?session_id=session-123"));
        assert!(panel.contains("hellox teleport connect dev --session-id session-123"));
        assert!(panel.contains("/remote-env panel dev"));
    }
}
