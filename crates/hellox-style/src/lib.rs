use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hellox_config::{output_styles_root, HelloxConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NamedPrompt {
    pub name: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptLayers {
    pub output_style: Option<NamedPrompt>,
    pub persona: Option<NamedPrompt>,
    pub fragments: Vec<NamedPrompt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PromptAssetKind {
    OutputStyle,
    Persona,
    Fragment,
}

impl PromptAssetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OutputStyle => "output_style",
            Self::Persona => "persona",
            Self::Fragment => "fragment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PromptAssetScope {
    User,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptAssetDefinition {
    pub name: String,
    pub kind: PromptAssetKind,
    pub scope: PromptAssetScope,
    pub path: PathBuf,
    pub prompt: String,
}

pub fn compose_prompt_layers(base_prompt: &str, layers: &PromptLayers) -> String {
    let mut prompt = base_prompt.trim_end().to_string();

    if let Some(persona) = &layers.persona {
        prompt.push_str("\n\n# Persona\n");
        prompt.push_str(&format!(
            "- Active persona: {}\n- Apply the following persona guidance unless the user overrides it.\n\n{}",
            persona.name,
            persona.prompt.trim()
        ));
    }

    if !layers.fragments.is_empty() {
        prompt.push_str("\n\n# Prompt fragments\n");
        prompt.push_str(&format!(
            "- Active fragments: {}\n- Treat these fragments as additive guidance unless the user overrides them.\n",
            render_names(&layers.fragments)
        ));
        for fragment in &layers.fragments {
            prompt.push_str(&format!(
                "\n\n## Fragment: {}\n{}",
                fragment.name,
                fragment.prompt.trim()
            ));
        }
    }

    if let Some(style) = &layers.output_style {
        prompt.push_str("\n\n# Output style\n");
        prompt.push_str(&format!(
            "- Active style: {}\n- Apply the following output guidance unless the user overrides it.\n\n{}",
            style.name,
            style.prompt.trim()
        ));
    }

    prompt
}

pub fn resolve_prompt_layers(config: &HelloxConfig, workspace_root: &Path) -> Result<PromptLayers> {
    Ok(PromptLayers {
        output_style: resolve_named_prompt(
            config.output_style.default.as_deref(),
            workspace_root,
            PromptAssetKind::OutputStyle,
        )?,
        persona: resolve_named_prompt(
            config.prompt.persona.as_deref(),
            workspace_root,
            PromptAssetKind::Persona,
        )?,
        fragments: config
            .prompt
            .fragments
            .iter()
            .map(|name| {
                load_prompt_fragment(name, workspace_root).map(named_prompt_from_definition)
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

pub fn resolve_configured_output_style(
    config: &HelloxConfig,
    workspace_root: &Path,
) -> Result<Option<NamedPrompt>> {
    resolve_named_prompt(
        config.output_style.default.as_deref(),
        workspace_root,
        PromptAssetKind::OutputStyle,
    )
}

pub fn resolve_configured_persona(
    config: &HelloxConfig,
    workspace_root: &Path,
) -> Result<Option<NamedPrompt>> {
    resolve_named_prompt(
        config.prompt.persona.as_deref(),
        workspace_root,
        PromptAssetKind::Persona,
    )
}

pub fn resolve_configured_fragments(
    config: &HelloxConfig,
    workspace_root: &Path,
) -> Result<Vec<NamedPrompt>> {
    config
        .prompt
        .fragments
        .iter()
        .map(|name| load_prompt_fragment(name, workspace_root).map(named_prompt_from_definition))
        .collect()
}

pub fn discover_output_styles(workspace_root: &Path) -> Result<Vec<PromptAssetDefinition>> {
    discover_assets(
        PromptAssetKind::OutputStyle,
        output_styles_root(),
        project_output_styles_root(workspace_root),
    )
}

pub fn discover_personas(workspace_root: &Path) -> Result<Vec<PromptAssetDefinition>> {
    discover_assets(
        PromptAssetKind::Persona,
        user_personas_root(),
        project_personas_root(workspace_root),
    )
}

pub fn discover_prompt_fragments(workspace_root: &Path) -> Result<Vec<PromptAssetDefinition>> {
    discover_assets(
        PromptAssetKind::Fragment,
        user_prompt_fragments_root(),
        project_prompt_fragments_root(workspace_root),
    )
}

pub fn load_output_style(name: &str, workspace_root: &Path) -> Result<PromptAssetDefinition> {
    load_asset(
        PromptAssetKind::OutputStyle,
        name,
        output_styles_root(),
        project_output_styles_root(workspace_root),
    )
}

pub fn load_persona(name: &str, workspace_root: &Path) -> Result<PromptAssetDefinition> {
    load_asset(
        PromptAssetKind::Persona,
        name,
        user_personas_root(),
        project_personas_root(workspace_root),
    )
}

pub fn load_prompt_fragment(name: &str, workspace_root: &Path) -> Result<PromptAssetDefinition> {
    load_asset(
        PromptAssetKind::Fragment,
        name,
        user_prompt_fragments_root(),
        project_prompt_fragments_root(workspace_root),
    )
}

pub fn format_definition_list(
    definitions: &[PromptAssetDefinition],
    default_names: &[String],
    active_names: &[String],
) -> String {
    if definitions.is_empty() {
        return "No definitions found.".to_string();
    }

    let mut lines = vec!["name\tdefault\tactive\tscope\tpath".to_string()];
    for definition in definitions {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}",
            definition.name,
            yes_no(default_names.iter().any(|name| name == &definition.name)),
            yes_no(active_names.iter().any(|name| name == &definition.name)),
            match definition.scope {
                PromptAssetScope::User => "user",
                PromptAssetScope::Project => "project",
            },
            normalize_path(&definition.path)
        ));
    }
    lines.join("\n")
}

pub fn format_definition_detail(
    definition: &PromptAssetDefinition,
    default_names: &[String],
    active_names: &[String],
) -> String {
    format!(
        "name: {}\nkind: {}\ndefault: {}\nactive: {}\nscope: {}\npath: {}\n\n{}",
        definition.name,
        definition.kind.as_str(),
        yes_no(default_names.iter().any(|name| name == &definition.name)),
        yes_no(active_names.iter().any(|name| name == &definition.name)),
        match definition.scope {
            PromptAssetScope::User => "user",
            PromptAssetScope::Project => "project",
        },
        normalize_path(&definition.path),
        definition.prompt
    )
}

pub fn project_output_styles_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".hellox").join("output-styles")
}

pub fn project_personas_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".hellox").join("personas")
}

pub fn project_prompt_fragments_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".hellox").join("prompt-fragments")
}

pub fn user_personas_root() -> PathBuf {
    hellox_config::config_root().join("personas")
}

pub fn user_prompt_fragments_root() -> PathBuf {
    hellox_config::config_root().join("prompt-fragments")
}

fn resolve_named_prompt(
    name: Option<&str>,
    workspace_root: &Path,
    kind: PromptAssetKind,
) -> Result<Option<NamedPrompt>> {
    match name {
        Some(name) => load_asset_for_kind(kind, name, workspace_root)
            .map(named_prompt_from_definition)
            .map(Some),
        None => Ok(None),
    }
}

fn load_asset_for_kind(
    kind: PromptAssetKind,
    name: &str,
    workspace_root: &Path,
) -> Result<PromptAssetDefinition> {
    match kind {
        PromptAssetKind::OutputStyle => load_output_style(name, workspace_root),
        PromptAssetKind::Persona => load_persona(name, workspace_root),
        PromptAssetKind::Fragment => load_prompt_fragment(name, workspace_root),
    }
}

fn discover_assets(
    kind: PromptAssetKind,
    user_root: PathBuf,
    project_root: PathBuf,
) -> Result<Vec<PromptAssetDefinition>> {
    let mut definitions = BTreeMap::new();
    load_assets_from_root(kind, user_root, PromptAssetScope::User, &mut definitions)?;
    load_assets_from_root(
        kind,
        project_root,
        PromptAssetScope::Project,
        &mut definitions,
    )?;
    Ok(definitions.into_values().collect())
}

fn load_asset(
    kind: PromptAssetKind,
    name: &str,
    user_root: PathBuf,
    project_root: PathBuf,
) -> Result<PromptAssetDefinition> {
    discover_assets(kind, user_root, project_root)?
        .into_iter()
        .find(|definition| definition.name == name)
        .ok_or_else(|| anyhow!("{} `{name}` was not found", kind.as_str()))
}

fn load_assets_from_root(
    kind: PromptAssetKind,
    root: PathBuf,
    scope: PromptAssetScope,
    definitions: &mut BTreeMap<String, PromptAssetDefinition>,
) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for entry in
        fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !is_supported_prompt_path(&path) {
            continue;
        }

        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };

        let prompt = fs::read_to_string(&path)
            .with_context(|| format!("failed to read prompt definition {}", path.display()))?;
        definitions.insert(
            name.to_string(),
            PromptAssetDefinition {
                name: name.to_string(),
                kind,
                scope,
                path,
                prompt: prompt.trim().to_string(),
            },
        );
    }

    Ok(())
}

fn named_prompt_from_definition(definition: PromptAssetDefinition) -> NamedPrompt {
    NamedPrompt {
        name: definition.name,
        prompt: definition.prompt,
    }
}

fn is_supported_prompt_path(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("md" | "txt")
    )
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn render_names(prompts: &[NamedPrompt]) -> String {
    prompts
        .iter()
        .map(|prompt| prompt.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_config::HelloxConfig;

    use super::*;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-style-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn project_assets_override_user_assets() {
        let root = temp_dir();
        let user_root = root.join("user");
        let project_root = root.join("project");
        fs::create_dir_all(&user_root).expect("create user root");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(user_root.join("reviewer.md"), "user reviewer").expect("write user reviewer");
        fs::write(project_root.join("reviewer.md"), "project reviewer")
            .expect("write project reviewer");

        let styles = discover_assets(
            PromptAssetKind::OutputStyle,
            user_root.clone(),
            project_root.clone(),
        )
        .expect("discover assets");
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].prompt, "project reviewer");
        assert_eq!(styles[0].scope, PromptAssetScope::Project);
    }

    #[test]
    fn resolve_prompt_layers_loads_defaults() {
        let root = temp_dir();
        let styles_root = project_output_styles_root(&root);
        let personas_root = project_personas_root(&root);
        let fragments_root = project_prompt_fragments_root(&root);
        fs::create_dir_all(&styles_root).expect("create styles root");
        fs::create_dir_all(&personas_root).expect("create personas root");
        fs::create_dir_all(&fragments_root).expect("create fragments root");
        fs::write(styles_root.join("concise.md"), "Keep it short.").expect("write style");
        fs::write(personas_root.join("reviewer.md"), "Act like a reviewer.")
            .expect("write persona");
        fs::write(fragments_root.join("safety.md"), "Call out safety risks.")
            .expect("write fragment");

        let mut config = HelloxConfig::default();
        config.output_style.default = Some("concise".to_string());
        config.prompt.persona = Some("reviewer".to_string());
        config.prompt.fragments = vec!["safety".to_string()];

        let layers = resolve_prompt_layers(&config, &root).expect("resolve layers");
        assert_eq!(
            layers.output_style.as_ref().map(|item| item.name.as_str()),
            Some("concise")
        );
        assert_eq!(
            layers.persona.as_ref().map(|item| item.name.as_str()),
            Some("reviewer")
        );
        assert_eq!(layers.fragments.len(), 1);
        assert_eq!(layers.fragments[0].name, "safety");
    }

    #[test]
    fn compose_prompt_layers_appends_sections() {
        let prompt = compose_prompt_layers(
            "Base prompt",
            &PromptLayers {
                output_style: Some(NamedPrompt {
                    name: "concise".to_string(),
                    prompt: "Keep it short.".to_string(),
                }),
                persona: Some(NamedPrompt {
                    name: "reviewer".to_string(),
                    prompt: "Look for risks.".to_string(),
                }),
                fragments: vec![NamedPrompt {
                    name: "safety".to_string(),
                    prompt: "Highlight safety issues.".to_string(),
                }],
            },
        );
        assert!(prompt.contains("# Persona"));
        assert!(prompt.contains("# Prompt fragments"));
        assert!(prompt.contains("# Output style"));
    }
}
