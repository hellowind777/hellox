use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hellox_agent::OutputStylePrompt;
use hellox_config::{output_styles_root, HelloxConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OutputStyleScope {
    User,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputStyleDefinition {
    pub name: String,
    pub scope: OutputStyleScope,
    pub path: PathBuf,
    pub prompt: String,
}

pub fn resolve_configured_output_style(
    config: &HelloxConfig,
    workspace_root: &Path,
) -> Result<Option<OutputStylePrompt>> {
    match config.output_style.default.as_deref() {
        Some(name) => load_output_style(name, workspace_root).map(|style| {
            Some(OutputStylePrompt {
                name: style.name,
                prompt: style.prompt,
            })
        }),
        None => Ok(None),
    }
}

pub fn discover_output_styles(workspace_root: &Path) -> Result<Vec<OutputStyleDefinition>> {
    let mut styles = BTreeMap::new();
    load_output_styles_from_root(output_styles_root(), OutputStyleScope::User, &mut styles)?;
    load_output_styles_from_root(
        project_output_styles_root(workspace_root),
        OutputStyleScope::Project,
        &mut styles,
    )?;
    Ok(styles.into_values().collect())
}

pub fn load_output_style(name: &str, workspace_root: &Path) -> Result<OutputStyleDefinition> {
    discover_output_styles(workspace_root)?
        .into_iter()
        .find(|style| style.name == name)
        .ok_or_else(|| anyhow!("Output style `{name}` was not found"))
}

pub fn project_output_styles_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".hellox").join("output-styles")
}

pub fn format_output_style_list(
    styles: &[OutputStyleDefinition],
    default_style: Option<&str>,
) -> String {
    if styles.is_empty() {
        return "No output styles found.".to_string();
    }

    let mut lines = vec!["style\tdefault\tscope\tpath".to_string()];
    for style in styles {
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            style.name,
            if default_style == Some(style.name.as_str()) {
                "yes"
            } else {
                "no"
            },
            match style.scope {
                OutputStyleScope::User => "user",
                OutputStyleScope::Project => "project",
            },
            normalize_path(&style.path)
        ));
    }
    lines.join("\n")
}

pub fn format_output_style_detail(
    style: &OutputStyleDefinition,
    is_default: bool,
    is_active: bool,
) -> String {
    format!(
        "style: {}\ndefault: {}\nactive: {}\nscope: {}\npath: {}\n\n{}",
        style.name,
        is_default,
        is_active,
        match style.scope {
            OutputStyleScope::User => "user",
            OutputStyleScope::Project => "project",
        },
        normalize_path(&style.path),
        style.prompt
    )
}

fn load_output_styles_from_root(
    root: PathBuf,
    scope: OutputStyleScope,
    styles: &mut BTreeMap<String, OutputStyleDefinition>,
) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for entry in
        fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !is_supported_style_path(&path) {
            continue;
        }

        let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let prompt = fs::read_to_string(&path)
            .with_context(|| format!("failed to read output style {}", path.display()))?;
        styles.insert(
            name.to_string(),
            OutputStyleDefinition {
                name: name.to_string(),
                scope,
                path,
                prompt: prompt.trim().to_string(),
            },
        );
    }

    Ok(())
}

fn is_supported_style_path(path: &Path) -> bool {
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

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_config::HelloxConfig;

    use super::{
        discover_output_styles, format_output_style_list, load_output_style,
        project_output_styles_root, resolve_configured_output_style,
    };

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-output-styles-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn discovers_project_output_styles() {
        let root = temp_dir();
        let project_root = project_output_styles_root(&root);
        fs::create_dir_all(&project_root).expect("create style root");
        fs::write(
            project_root.join("concise.md"),
            "Respond tersely and avoid extra framing.\n",
        )
        .expect("write style");

        let styles = discover_output_styles(&root).expect("discover styles");
        assert!(styles.iter().any(|style| style.name == "concise"));
    }

    #[test]
    fn load_output_style_resolves_configured_default() {
        let root = temp_dir();
        let project_root = project_output_styles_root(&root);
        fs::create_dir_all(&project_root).expect("create style root");
        fs::write(
            project_root.join("reviewer.txt"),
            "Prioritize bugs, risks, and missing tests.\n",
        )
        .expect("write style");

        let mut config = HelloxConfig::default();
        config.output_style.default = Some("reviewer".to_string());

        let style = resolve_configured_output_style(&config, &root)
            .expect("resolve style")
            .expect("configured style");
        assert_eq!(style.name, "reviewer");
        assert!(style.prompt.contains("Prioritize bugs"));

        let loaded = load_output_style("reviewer", &root).expect("load style");
        assert_eq!(loaded.name, "reviewer");
    }

    #[test]
    fn format_output_style_list_marks_default() {
        let root = temp_dir();
        let project_root = project_output_styles_root(&root);
        fs::create_dir_all(&project_root).expect("create style root");
        fs::write(project_root.join("concise.md"), "Keep it short.\n").expect("write style");

        let styles = discover_output_styles(&root).expect("discover styles");
        let text = format_output_style_list(&styles, Some("concise"));
        assert!(text.contains("concise\tyes"));
    }
}
