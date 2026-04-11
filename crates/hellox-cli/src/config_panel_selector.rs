use hellox_config::HelloxConfig;
use hellox_tui::{render_selector, SelectorEntry};

pub(crate) fn config_selector_keys(config: &HelloxConfig) -> Vec<String> {
    resolved_values(config)
        .into_iter()
        .map(|entry| entry.key.to_string())
        .collect()
}

pub(super) fn render_config_selector(
    config: &HelloxConfig,
    focus_key: Option<&str>,
) -> Vec<String> {
    let focus_key = normalized_focus_key(focus_key);
    let entries = resolved_values(config)
        .into_iter()
        .map(|entry| {
            SelectorEntry::new(
                entry.key.to_string(),
                vec![
                    format!("value: {}", preview_text(&entry.value, 96)),
                    format!("kind: {}", entry.kind),
                    format!("clearable: {}", yes_no(entry.clearable)),
                    format!("set: `hellox config set {} <value>`", entry.key),
                    if entry.clearable {
                        format!("clear: `hellox config clear {}`", entry.key)
                    } else {
                        "clear: (not supported)".to_string()
                    },
                ],
            )
            .selected(focus_key == Some(entry.key))
        })
        .collect::<Vec<_>>();
    render_selector(&entries)
}

pub(super) fn render_config_lens(config: &HelloxConfig, focus_key: Option<&str>) -> Vec<String> {
    let focus_key = normalized_focus_key(focus_key);
    let Some(entry) = resolved_values(config)
        .into_iter()
        .find(|entry| focus_key.is_none_or(|key| key == entry.key))
        .or_else(|| resolved_values(config).into_iter().next())
    else {
        return vec!["(no resolved config values)".to_string()];
    };

    let lines = vec![
        format!("value: {}", preview_text(&entry.value, 128)),
        format!("kind: {}", entry.kind),
        format!("clearable: {}", yes_no(entry.clearable)),
        format!("description: {}", entry.description),
        format!("value_chars: {}", entry.value.chars().count()),
        format!("set: `/config set {} <value>`", entry.key),
        format!("panel: `/config panel {}`", entry.key),
    ];

    render_selector(&[SelectorEntry::new(entry.key.to_string(), lines).selected(true)])
}

struct ConfigValueEntry {
    key: &'static str,
    value: String,
    kind: &'static str,
    clearable: bool,
    description: &'static str,
}

fn resolved_values(config: &HelloxConfig) -> Vec<ConfigValueEntry> {
    let fragments = if config.prompt.fragments.is_empty() {
        "(none)".to_string()
    } else {
        config.prompt.fragments.join(", ")
    };

    vec![
        ConfigValueEntry {
            key: "gateway.listen",
            value: config.gateway.listen.clone(),
            kind: "string",
            clearable: false,
            description: "Gateway listen address",
        },
        ConfigValueEntry {
            key: "permissions.mode",
            value: config.permissions.mode.to_string(),
            kind: "string",
            clearable: false,
            description: "Default permission mode",
        },
        ConfigValueEntry {
            key: "session.model",
            value: config.session.model.clone(),
            kind: "string",
            clearable: false,
            description: "Default session model profile",
        },
        ConfigValueEntry {
            key: "session.persist",
            value: config.session.persist.to_string(),
            kind: "bool",
            clearable: false,
            description: "Persist session snapshots by default",
        },
        ConfigValueEntry {
            key: "output_style.default",
            value: config
                .output_style
                .default
                .clone()
                .unwrap_or_else(|| "(none)".to_string()),
            kind: "string",
            clearable: true,
            description: "Default output style name",
        },
        ConfigValueEntry {
            key: "prompt.persona",
            value: config
                .prompt
                .persona
                .clone()
                .unwrap_or_else(|| "(none)".to_string()),
            kind: "string",
            clearable: true,
            description: "Default persona name",
        },
        ConfigValueEntry {
            key: "prompt.fragments",
            value: fragments,
            kind: "string-list",
            clearable: true,
            description: "Default prompt fragments",
        },
    ]
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
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

fn normalized_focus_key(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
