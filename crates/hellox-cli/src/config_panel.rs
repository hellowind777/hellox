use std::path::Path;

use hellox_config::HelloxConfig;
use hellox_tui::{render_panel, KeyValueRow, PanelSection};

#[path = "config_panel_selector.rs"]
mod selector;

use selector::{render_config_lens, render_config_selector};

pub(crate) fn render_config_panel(config_path: &Path, config: &HelloxConfig) -> String {
    let metadata = vec![KeyValueRow::new("config_path", normalize_path(config_path))];
    let sections = vec![
        PanelSection::new("Resolved config selector", render_config_selector(config)),
        PanelSection::new("Focused config lens", render_config_lens(config)),
        PanelSection::new("Action palette", config_cli_palette()),
        PanelSection::new("REPL palette", config_repl_palette()),
    ];

    render_panel("Config panel", &metadata, &sections)
}

fn config_cli_palette() -> Vec<String> {
    vec![
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
        "- panel: `/config panel`".to_string(),
        "- path|keys: `/config path` or `/config keys`".to_string(),
        "- set: `/config set <key> <value>`".to_string(),
        "- clear: `/config clear <key>`".to_string(),
    ]
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
