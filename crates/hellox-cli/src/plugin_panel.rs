use std::path::Path;

use anyhow::Result;
use hellox_config::{HelloxConfig, PluginSourceConfig};
use hellox_plugin::{inspect_plugin, load_installed_plugins, LoadedPlugin};
use hellox_tui::{render_panel, render_table, KeyValueRow, PanelSection, Table};

pub(crate) fn render_plugin_panel(
    config_path: &Path,
    config: &HelloxConfig,
    plugin_id: Option<&str>,
) -> Result<String> {
    let plugin_id = plugin_id.map(str::trim).filter(|value| !value.is_empty());
    match plugin_id {
        Some(plugin_id) => render_plugin_detail_panel(config_path, config, plugin_id),
        None => Ok(render_plugin_list_panel(config_path, config)),
    }
}

pub(crate) fn plugin_panel_ids(config: &HelloxConfig) -> Vec<String> {
    load_installed_plugins(config)
        .into_iter()
        .map(|plugin| plugin.plugin_id)
        .collect()
}

fn render_plugin_list_panel(config_path: &Path, config: &HelloxConfig) -> String {
    let plugins = load_installed_plugins(config);
    let enabled = plugins.iter().filter(|plugin| plugin.enabled).count();
    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("plugins", plugins.len().to_string()),
        KeyValueRow::new("enabled", enabled.to_string()),
        KeyValueRow::new(
            "marketplaces",
            config.plugins.marketplaces.len().to_string(),
        ),
    ];
    let sections = vec![
        PanelSection::new(
            "Installed plugins",
            render_table(&build_plugin_table(&plugins)),
        ),
        PanelSection::new(
            "Marketplaces",
            render_table(&build_marketplace_table(config)),
        ),
        PanelSection::new("Action palette", plugin_list_cli_palette()),
        PanelSection::new("REPL palette", plugin_list_repl_palette()),
    ];

    render_panel("Plugin panel", &metadata, &sections)
}

fn render_plugin_detail_panel(
    config_path: &Path,
    config: &HelloxConfig,
    plugin_id: &str,
) -> Result<String> {
    let plugin = inspect_plugin(config, plugin_id)?;
    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("plugin_id", plugin.plugin_id.clone()),
        KeyValueRow::new("enabled", yes_no(plugin.enabled)),
        KeyValueRow::new("source", source_label(&plugin.source)),
        KeyValueRow::new(
            "install_path",
            plugin
                .install_path
                .as_ref()
                .map(|path| normalize_path(path))
                .unwrap_or_else(|| "(none)".to_string()),
        ),
        KeyValueRow::new(
            "version",
            plugin
                .manifest
                .as_ref()
                .map(|manifest| manifest.version.clone())
                .unwrap_or_else(|| "(unknown)".to_string()),
        ),
    ];

    let sections = vec![
        PanelSection::new("Manifest", manifest_lines(&plugin)),
        PanelSection::new("Warnings", warning_lines(&plugin)),
        PanelSection::new("Action palette", plugin_detail_cli_palette(plugin_id)),
        PanelSection::new("REPL palette", plugin_detail_repl_palette(plugin_id)),
    ];

    Ok(render_panel(
        &format!("Plugin detail panel: {plugin_id}"),
        &metadata,
        &sections,
    ))
}

fn build_plugin_table(plugins: &[LoadedPlugin]) -> Table {
    let rows = plugins
        .iter()
        .enumerate()
        .map(|(index, plugin)| {
            vec![
                (index + 1).to_string(),
                plugin.plugin_id.clone(),
                yes_no(plugin.enabled),
                plugin
                    .manifest
                    .as_ref()
                    .map(|manifest| manifest.version.clone())
                    .unwrap_or_else(|| "-".to_string()),
                source_label(&plugin.source),
                capability_summary(plugin),
                if plugin.warnings.is_empty() {
                    "-".to_string()
                } else {
                    preview_text(&plugin.warnings.join(" | "), 36)
                },
                format!("hellox plugin panel {}", plugin.plugin_id),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "#".to_string(),
            "plugin".to_string(),
            "enabled".to_string(),
            "version".to_string(),
            "source".to_string(),
            "capabilities".to_string(),
            "warnings".to_string(),
            "open".to_string(),
        ],
        rows,
    )
}

fn build_marketplace_table(config: &HelloxConfig) -> Table {
    let rows = config
        .plugins
        .marketplaces
        .iter()
        .enumerate()
        .map(|(index, (name, marketplace))| {
            vec![
                (index + 1).to_string(),
                name.clone(),
                yes_no(marketplace.enabled),
                marketplace.url.clone(),
                preview_text(marketplace.description.as_deref().unwrap_or("(none)"), 40),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "#".to_string(),
            "marketplace".to_string(),
            "enabled".to_string(),
            "url".to_string(),
            "description".to_string(),
        ],
        rows,
    )
}

fn manifest_lines(plugin: &LoadedPlugin) -> Vec<String> {
    let Some(manifest) = &plugin.manifest else {
        return vec!["manifest: (unavailable)".to_string()];
    };
    let mut lines = vec![
        format!("name: {}", manifest.name),
        format!("version: {}", manifest.version),
        format!(
            "description: {}",
            manifest.description.as_deref().unwrap_or("(none)")
        ),
        format!(
            "commands: {}",
            if manifest.commands.is_empty() {
                "(none)".to_string()
            } else {
                manifest.commands.join(", ")
            }
        ),
        format!(
            "skills: {}",
            if manifest.skills.is_empty() {
                "(none)".to_string()
            } else {
                manifest.skills.join(", ")
            }
        ),
        format!(
            "hooks: {}",
            if manifest.hooks.is_empty() {
                "(none)".to_string()
            } else {
                manifest.hooks.join(", ")
            }
        ),
        format!(
            "mcp_servers: {}",
            if manifest.mcp_servers.is_empty() {
                "(none)".to_string()
            } else {
                manifest.mcp_servers.join(", ")
            }
        ),
    ];

    if let PluginSourceConfig::Marketplace {
        marketplace,
        package,
        version,
    } = &plugin.source
    {
        lines.push(format!("marketplace: {marketplace}"));
        lines.push(format!("package: {package}"));
        lines.push(format!(
            "marketplace_version: {}",
            version.as_deref().unwrap_or("(none)")
        ));
    }
    lines
}

fn warning_lines(plugin: &LoadedPlugin) -> Vec<String> {
    if plugin.warnings.is_empty() {
        Vec::new()
    } else {
        plugin
            .warnings
            .iter()
            .map(|warning| format!("- {warning}"))
            .collect()
    }
}

fn plugin_list_cli_palette() -> Vec<String> {
    vec![
        "- open panel: `hellox plugin panel <plugin-id>`".to_string(),
        "- install: `hellox plugin install <path>`".to_string(),
        "- marketplace add: `hellox plugin marketplace add <name> --url <url>`".to_string(),
        "- enable: `hellox plugin enable <plugin-id>`".to_string(),
        "- remove: `hellox plugin remove <plugin-id>`".to_string(),
    ]
}

fn plugin_list_repl_palette() -> Vec<String> {
    vec![
        "- open panel: `/plugin panel [plugin-id]`".to_string(),
        "- numeric open: render `/plugin panel`, then enter `1..n`".to_string(),
        "- show detail: `/plugin show <plugin-id>`".to_string(),
        "- install: `/plugin install <path>`".to_string(),
        "- marketplaces: `/plugin marketplace`".to_string(),
    ]
}

fn plugin_detail_cli_palette(plugin_id: &str) -> Vec<String> {
    vec![
        "- back to list: `hellox plugin panel`".to_string(),
        format!("- show raw detail: `hellox plugin show {plugin_id}`"),
        format!("- enable: `hellox plugin enable {plugin_id}`"),
        format!("- disable: `hellox plugin disable {plugin_id}`"),
        format!("- remove: `hellox plugin remove {plugin_id}`"),
    ]
}

fn plugin_detail_repl_palette(plugin_id: &str) -> Vec<String> {
    vec![
        "- back to list: `/plugin panel`".to_string(),
        format!("- show raw detail: `/plugin show {plugin_id}`"),
        format!("- enable: `/plugin enable {plugin_id}`"),
        format!("- disable: `/plugin disable {plugin_id}`"),
        format!("- remove: `/plugin remove {plugin_id}`"),
    ]
}

fn capability_summary(plugin: &LoadedPlugin) -> String {
    plugin
        .manifest
        .as_ref()
        .map(|manifest| {
            format!(
                "c{}/s{}/h{}/m{}",
                manifest.commands.len(),
                manifest.skills.len(),
                manifest.hooks.len(),
                manifest.mcp_servers.len()
            )
        })
        .unwrap_or_else(|| "-".to_string())
}

fn source_label(source: &PluginSourceConfig) -> String {
    match source {
        PluginSourceConfig::LocalPath { path } => format!("local:{path}"),
        PluginSourceConfig::Marketplace {
            marketplace,
            package,
            version,
        } => format!(
            "marketplace:{marketplace}/{package}@{}",
            version.as_deref().unwrap_or("latest")
        ),
        PluginSourceConfig::Builtin { name } => format!("builtin:{name}"),
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "YES".to_string()
    } else {
        "NO".to_string()
    }
}

fn preview_text(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let head = compact
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        format!("{head}...")
    }
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
