use anyhow::{anyhow, Result};
use std::path::Path;

use hellox_config::HelloxConfig;
use hellox_tui::{render_panel, KeyValueRow, PanelSection};

#[path = "config_panel_selector.rs"]
mod selector;

pub(crate) use selector::config_selector_keys;
use selector::{render_config_lens, render_config_selector};

pub(crate) fn render_config_panel(
    config_path: &Path,
    config: &HelloxConfig,
    focus_key: Option<&str>,
) -> Result<String> {
    if let Some(focus_key) = focus_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|focus_key| {
            !config_selector_keys(config)
                .iter()
                .any(|key| key == focus_key)
        })
    {
        return Err(anyhow!(
            "config panel key `{focus_key}` was not found; use `hellox config keys` to list supported keys"
        ));
    }

    let metadata = vec![KeyValueRow::new("config_path", normalize_path(config_path))];
    let sections = vec![
        PanelSection::new(
            "Resolved config selector",
            render_config_selector(config, focus_key),
        ),
        PanelSection::new("Focused config lens", render_config_lens(config, focus_key)),
        PanelSection::new("Action palette", config_cli_palette()),
        PanelSection::new("REPL palette", config_repl_palette()),
    ];

    Ok(render_panel("Config panel", &metadata, &sections))
}

fn config_cli_palette() -> Vec<String> {
    vec![
        "- focus one: `hellox config panel <key>`".to_string(),
        "- show: `hellox config show`".to_string(),
        "- path: `hellox config path`".to_string(),
        "- keys: `hellox config keys`".to_string(),
        "- set: `hellox config set <key> <value>`".to_string(),
        "- clear: `hellox config clear <key>`".to_string(),
    ]
}

fn config_repl_palette() -> Vec<String> {
    vec![
        "- show: `/config`".to_string(),
        "- panel: `/config panel [key]`".to_string(),
        "- numeric focus: render `/config panel`, then enter `1..n`".to_string(),
        "- path|keys: `/config path` or `/config keys`".to_string(),
        "- set: `/config set <key> <value>`".to_string(),
        "- clear: `/config clear <key>`".to_string(),
    ]
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
