use std::path::Path;

use anyhow::{anyhow, Result};
use hellox_style::PromptAssetDefinition;
use hellox_tui::{render_panel, render_table, KeyValueRow, PanelSection, Table};

use crate::output_styles::{discover_output_styles, OutputStyleDefinition, OutputStyleScope};
use crate::personas::discover_personas;
use crate::prompt_fragments::discover_prompt_fragments;
use crate::style_command_support::normalize_path;

struct PanelAsset {
    name: String,
    kind: &'static str,
    scope: &'static str,
    path: String,
    prompt: String,
}

pub(crate) fn render_output_style_panel(
    config_path: &Path,
    workspace_root: &Path,
    default_style: Option<&str>,
    active_style: Option<&str>,
    style_name: Option<&str>,
) -> Result<String> {
    let assets = discover_output_styles(workspace_root)?
        .iter()
        .map(output_style_asset)
        .collect::<Vec<_>>();
    render_asset_panel(
        "Output style panel",
        config_path,
        workspace_root,
        &assets,
        &names_from_option(default_style),
        &names_from_option(active_style),
        style_name,
        "output-style",
        "output-style",
    )
}

pub(crate) fn output_style_panel_names(workspace_root: &Path) -> Result<Vec<String>> {
    Ok(discover_output_styles(workspace_root)?
        .iter()
        .map(|style| style.name.clone())
        .collect())
}

pub(crate) fn render_persona_panel(
    config_path: &Path,
    workspace_root: &Path,
    default_persona: Option<&str>,
    active_persona: Option<&str>,
    persona_name: Option<&str>,
) -> Result<String> {
    let assets = discover_personas(workspace_root)?
        .iter()
        .map(prompt_asset)
        .collect::<Vec<_>>();
    render_asset_panel(
        "Persona panel",
        config_path,
        workspace_root,
        &assets,
        &names_from_option(default_persona),
        &names_from_option(active_persona),
        persona_name,
        "persona",
        "persona",
    )
}

pub(crate) fn persona_panel_names(workspace_root: &Path) -> Result<Vec<String>> {
    Ok(discover_personas(workspace_root)?
        .iter()
        .map(|persona| persona.name.clone())
        .collect())
}

pub(crate) fn render_prompt_fragment_panel(
    config_path: &Path,
    workspace_root: &Path,
    default_fragments: &[String],
    active_fragments: &[String],
    fragment_name: Option<&str>,
) -> Result<String> {
    let assets = discover_prompt_fragments(workspace_root)?
        .iter()
        .map(prompt_asset)
        .collect::<Vec<_>>();
    render_asset_panel(
        "Prompt fragment panel",
        config_path,
        workspace_root,
        &assets,
        default_fragments,
        active_fragments,
        fragment_name,
        "prompt-fragment",
        "fragment",
    )
}

pub(crate) fn prompt_fragment_panel_names(workspace_root: &Path) -> Result<Vec<String>> {
    Ok(discover_prompt_fragments(workspace_root)?
        .iter()
        .map(|fragment| fragment.name.clone())
        .collect())
}

fn render_asset_panel(
    title: &str,
    config_path: &Path,
    workspace_root: &Path,
    assets: &[PanelAsset],
    default_names: &[String],
    active_names: &[String],
    selected_name: Option<&str>,
    cli_root: &str,
    repl_root: &str,
) -> Result<String> {
    let selected_name = selected_name
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match selected_name {
        Some(selected_name) => render_asset_detail_panel(
            title,
            config_path,
            workspace_root,
            assets,
            default_names,
            active_names,
            selected_name,
            cli_root,
            repl_root,
        ),
        None => Ok(render_asset_list_panel(
            title,
            config_path,
            workspace_root,
            assets,
            default_names,
            active_names,
            cli_root,
            repl_root,
        )),
    }
}

fn render_asset_list_panel(
    title: &str,
    config_path: &Path,
    workspace_root: &Path,
    assets: &[PanelAsset],
    default_names: &[String],
    active_names: &[String],
    cli_root: &str,
    repl_root: &str,
) -> String {
    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("workspace_root", normalize_path(workspace_root)),
        KeyValueRow::new("definitions", assets.len().to_string()),
        KeyValueRow::new("defaults", render_names(default_names)),
        KeyValueRow::new("active", render_names(active_names)),
    ];
    let sections = vec![
        PanelSection::new(
            "Definitions",
            render_table(&build_asset_table(
                assets,
                default_names,
                active_names,
                cli_root,
            )),
        ),
        PanelSection::new("Action palette", asset_list_cli_palette(cli_root)),
        PanelSection::new("REPL palette", asset_list_repl_palette(repl_root)),
    ];

    render_panel(title, &metadata, &sections)
}

fn render_asset_detail_panel(
    title: &str,
    config_path: &Path,
    workspace_root: &Path,
    assets: &[PanelAsset],
    default_names: &[String],
    active_names: &[String],
    selected_name: &str,
    cli_root: &str,
    repl_root: &str,
) -> Result<String> {
    let asset = assets
        .iter()
        .find(|asset| asset.name == selected_name)
        .ok_or_else(|| anyhow!("{title} entry `{selected_name}` was not found"))?;

    let metadata = vec![
        KeyValueRow::new("config_path", normalize_path(config_path)),
        KeyValueRow::new("workspace_root", normalize_path(workspace_root)),
        KeyValueRow::new("name", asset.name.clone()),
        KeyValueRow::new("kind", asset.kind),
        KeyValueRow::new(
            "default",
            yes_no(default_names.iter().any(|name| name == &asset.name)),
        ),
        KeyValueRow::new(
            "active",
            yes_no(active_names.iter().any(|name| name == &asset.name)),
        ),
        KeyValueRow::new("scope", asset.scope),
        KeyValueRow::new("path", asset.path.clone()),
    ];
    let sections = vec![
        PanelSection::new("Prompt preview", prompt_lines(&asset.prompt)),
        PanelSection::new(
            "Action palette",
            asset_detail_cli_palette(cli_root, &asset.name),
        ),
        PanelSection::new(
            "REPL palette",
            asset_detail_repl_palette(repl_root, &asset.name),
        ),
    ];

    Ok(render_panel(
        &format!("{title}: {}", asset.name),
        &metadata,
        &sections,
    ))
}

fn build_asset_table(
    assets: &[PanelAsset],
    default_names: &[String],
    active_names: &[String],
    cli_root: &str,
) -> Table {
    let rows = assets
        .iter()
        .enumerate()
        .map(|(index, asset)| {
            vec![
                (index + 1).to_string(),
                asset.name.clone(),
                yes_no(default_names.iter().any(|name| name == &asset.name)),
                yes_no(active_names.iter().any(|name| name == &asset.name)),
                asset.scope.to_string(),
                preview_text(&asset.path, 44),
                format!("hellox {cli_root} panel {}", asset.name),
            ]
        })
        .collect::<Vec<_>>();

    Table::new(
        vec![
            "#".to_string(),
            "name".to_string(),
            "default".to_string(),
            "active".to_string(),
            "scope".to_string(),
            "path".to_string(),
            "open".to_string(),
        ],
        rows,
    )
}

fn output_style_asset(style: &OutputStyleDefinition) -> PanelAsset {
    PanelAsset {
        name: style.name.clone(),
        kind: "output_style",
        scope: match style.scope {
            OutputStyleScope::User => "user",
            OutputStyleScope::Project => "project",
        },
        path: normalize_path(&style.path),
        prompt: style.prompt.clone(),
    }
}

fn prompt_asset(asset: &PromptAssetDefinition) -> PanelAsset {
    PanelAsset {
        name: asset.name.clone(),
        kind: asset.kind.as_str(),
        scope: match asset.scope {
            hellox_style::PromptAssetScope::User => "user",
            hellox_style::PromptAssetScope::Project => "project",
        },
        path: normalize_path(&asset.path),
        prompt: asset.prompt.clone(),
    }
}

fn asset_list_cli_palette(cli_root: &str) -> Vec<String> {
    vec![
        format!("- open panel: `hellox {cli_root} panel <name>`"),
        format!("- show raw: `hellox {cli_root} show <name>`"),
        format!("- set default: `hellox {cli_root} set-default <name>`"),
        format!("- clear default: `hellox {cli_root} clear-default`"),
    ]
}

fn asset_list_repl_palette(repl_root: &str) -> Vec<String> {
    vec![
        format!("- open panel: `/{repl_root} panel [name]`"),
        format!("- numeric open: render `/{repl_root} panel`, then enter `1..n`"),
        format!("- show raw: `/{repl_root} show <name>`"),
        format!("- use in session: `/{repl_root} use <name>`"),
        format!("- clear active: `/{repl_root} clear`"),
    ]
}

fn asset_detail_cli_palette(cli_root: &str, name: &str) -> Vec<String> {
    vec![
        format!("- back to list: `hellox {cli_root} panel`"),
        format!("- show raw: `hellox {cli_root} show {name}`"),
        format!("- set default: `hellox {cli_root} set-default {name}`"),
        format!("- clear default: `hellox {cli_root} clear-default`"),
    ]
}

fn asset_detail_repl_palette(repl_root: &str, name: &str) -> Vec<String> {
    vec![
        format!("- back to list: `/{repl_root} panel`"),
        format!("- show raw: `/{repl_root} show {name}`"),
        format!("- use in session: `/{repl_root} use {name}`"),
        format!("- clear active: `/{repl_root} clear`"),
    ]
}

fn prompt_lines(prompt: &str) -> Vec<String> {
    if prompt.trim().is_empty() {
        vec!["(empty)".to_string()]
    } else {
        prompt.lines().map(ToString::to_string).collect()
    }
}

fn names_from_option(value: Option<&str>) -> Vec<String> {
    value.map(ToString::to_string).into_iter().collect()
}

fn render_names(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    }
}

fn preview_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let head = value
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        format!("{head}...")
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "YES".to_string()
    } else {
        "NO".to_string()
    }
}
