use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hellox_config::{HelloxConfig, PluginEntryConfig, PluginSourceConfig};
use serde::{Deserialize, Serialize};

const MANIFEST_DIR: &str = ".hellox-plugin";
const MANIFEST_FILE: &str = "plugin.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub plugin_id: String,
    pub enabled: bool,
    pub install_path: Option<PathBuf>,
    pub source: PluginSourceConfig,
    pub manifest: Option<PluginManifest>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PluginInstallResult {
    pub plugin_id: String,
    pub install_path: PathBuf,
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone)]
pub struct PluginRemovalResult {
    pub plugin_id: String,
    pub removed_path: Option<PathBuf>,
}

pub fn format_plugin_list(plugins: &[LoadedPlugin]) -> String {
    if plugins.is_empty() {
        return "No plugins installed.".to_string();
    }

    let mut lines = vec!["plugin_id\tenabled\tversion\tsource\tcapabilities\twarnings".to_string()];
    for plugin in plugins {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            plugin.plugin_id,
            plugin.enabled,
            plugin
                .manifest
                .as_ref()
                .map(|manifest| manifest.version.as_str())
                .unwrap_or("-"),
            source_label(&plugin.source),
            capability_summary(plugin.manifest.as_ref()),
            if plugin.warnings.is_empty() {
                "-".to_string()
            } else {
                plugin.warnings.join(" | ")
            }
        ));
    }
    lines.join("\n")
}

pub fn format_plugin_detail(plugin: &LoadedPlugin) -> String {
    let mut lines = vec![
        format!("id: {}", plugin.plugin_id),
        format!("enabled: {}", plugin.enabled),
        format!("source: {}", source_label(&plugin.source)),
        format!(
            "install_path: {}",
            plugin
                .install_path
                .as_ref()
                .map(|path| normalize_path(path))
                .unwrap_or_else(|| "(none)".to_string())
        ),
    ];

    if let Some(manifest) = &plugin.manifest {
        lines.push(format!("name: {}", manifest.name));
        lines.push(format!("version: {}", manifest.version));
        if let Some(description) = &manifest.description {
            lines.push(format!("description: {description}"));
        }
        lines.push(format!(
            "commands: {}",
            if manifest.commands.is_empty() {
                "(none)".to_string()
            } else {
                manifest.commands.join(", ")
            }
        ));
        lines.push(format!(
            "skills: {}",
            if manifest.skills.is_empty() {
                "(none)".to_string()
            } else {
                manifest.skills.join(", ")
            }
        ));
        lines.push(format!(
            "hooks: {}",
            if manifest.hooks.is_empty() {
                "(none)".to_string()
            } else {
                manifest.hooks.join(", ")
            }
        ));
        lines.push(format!(
            "mcp_servers: {}",
            if manifest.mcp_servers.is_empty() {
                "(none)".to_string()
            } else {
                manifest.mcp_servers.join(", ")
            }
        ));
    }

    if !plugin.warnings.is_empty() {
        lines.push(format!("warnings: {}", plugin.warnings.join(" | ")));
    }

    lines.join("\n")
}

pub fn load_installed_plugins(config: &HelloxConfig) -> Vec<LoadedPlugin> {
    config
        .plugins
        .installed
        .iter()
        .map(|(plugin_id, entry)| load_plugin_from_entry(plugin_id, entry))
        .collect()
}

pub fn inspect_plugin(config: &HelloxConfig, plugin_id: &str) -> Result<LoadedPlugin> {
    let entry = config
        .plugins
        .installed
        .get(plugin_id)
        .ok_or_else(|| anyhow!("Plugin `{plugin_id}` was not found"))?;
    Ok(load_plugin_from_entry(plugin_id, entry))
}

pub fn install_plugin(
    config: &mut HelloxConfig,
    source_root: &Path,
    plugins_root: &Path,
    enabled: bool,
) -> Result<PluginInstallResult> {
    let manifest = load_manifest(source_root)?;
    if config.plugins.installed.contains_key(&manifest.id) {
        return Err(anyhow!("Plugin `{}` is already installed", manifest.id));
    }

    fs::create_dir_all(plugins_root)
        .with_context(|| format!("failed to create plugins root {}", plugins_root.display()))?;

    let install_path = plugins_root.join(&manifest.id);
    if install_path.exists() {
        return Err(anyhow!(
            "Install path `{}` already exists",
            normalize_path(&install_path)
        ));
    }

    copy_dir_all(source_root, &install_path)?;
    config.plugins.installed.insert(
        manifest.id.clone(),
        PluginEntryConfig {
            enabled,
            install_path: Some(normalize_path(&install_path)),
            source: PluginSourceConfig::LocalPath {
                path: normalize_path(source_root),
            },
            version: Some(manifest.version.clone()),
            description: manifest.description.clone(),
        },
    );

    Ok(PluginInstallResult {
        plugin_id: manifest.id.clone(),
        install_path,
        manifest,
    })
}

pub fn set_plugin_enabled(config: &mut HelloxConfig, plugin_id: &str, enabled: bool) -> Result<()> {
    let entry = config
        .plugins
        .installed
        .get_mut(plugin_id)
        .ok_or_else(|| anyhow!("Plugin `{plugin_id}` was not found"))?;
    entry.enabled = enabled;
    Ok(())
}

pub fn remove_plugin(
    config: &mut HelloxConfig,
    plugin_id: &str,
    plugins_root: &Path,
) -> Result<PluginRemovalResult> {
    let entry = config
        .plugins
        .installed
        .remove(plugin_id)
        .ok_or_else(|| anyhow!("Plugin `{plugin_id}` was not found"))?;

    let removed_path = entry.install_path.map(PathBuf::from);
    if let Some(path) = &removed_path {
        if path.exists() {
            ensure_path_within_root(path, plugins_root)?;
            fs::remove_dir_all(path)
                .with_context(|| format!("failed to remove plugin at {}", path.display()))?;
        }
    }

    Ok(PluginRemovalResult {
        plugin_id: plugin_id.to_string(),
        removed_path,
    })
}

pub fn load_manifest(root: &Path) -> Result<PluginManifest> {
    let manifest_path = manifest_path(root);
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read plugin manifest {}", manifest_path.display()))?;
    let manifest = serde_json::from_str::<PluginManifest>(&raw).with_context(|| {
        format!(
            "failed to parse plugin manifest {}",
            manifest_path.display()
        )
    })?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn load_plugin_from_entry(plugin_id: &str, entry: &PluginEntryConfig) -> LoadedPlugin {
    let install_path = entry.install_path.as_ref().map(PathBuf::from);
    let mut warnings = Vec::new();
    let manifest = match &install_path {
        Some(path) => match load_manifest(path) {
            Ok(manifest) => Some(manifest),
            Err(error) => {
                warnings.push(error.to_string());
                None
            }
        },
        None => {
            warnings.push("plugin has no install path configured".to_string());
            None
        }
    };

    LoadedPlugin {
        plugin_id: plugin_id.to_string(),
        enabled: entry.enabled,
        install_path,
        source: entry.source.clone(),
        manifest,
        warnings,
    }
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_DIR).join(MANIFEST_FILE)
}

fn validate_manifest(manifest: &PluginManifest) -> Result<()> {
    if manifest.id.trim().is_empty() {
        return Err(anyhow!("plugin manifest id must not be empty"));
    }
    if !manifest
        .id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(anyhow!(
            "plugin manifest id may only contain ASCII letters, digits, `-`, and `_`"
        ));
    }
    if manifest.name.trim().is_empty() {
        return Err(anyhow!("plugin manifest name must not be empty"));
    }
    if manifest.version.trim().is_empty() {
        return Err(anyhow!("plugin manifest version must not be empty"));
    }
    Ok(())
}

fn ensure_path_within_root(path: &Path, root: &Path) -> Result<()> {
    let canonical_path = fs::canonicalize(path)
        .with_context(|| format!("failed to resolve plugin path {}", path.display()))?;
    let canonical_root = fs::canonicalize(root)
        .with_context(|| format!("failed to resolve plugins root {}", root.display()))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(anyhow!(
            "refusing to delete plugin outside plugins root: {}",
            canonical_path.display()
        ));
    }
    Ok(())
}

fn copy_dir_all(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create directory {}", destination.display()))?;

    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn capability_summary(manifest: Option<&PluginManifest>) -> String {
    match manifest {
        Some(manifest) => format!(
            "cmd:{} skill:{} hook:{} mcp:{}",
            manifest.commands.len(),
            manifest.skills.len(),
            manifest.hooks.len(),
            manifest.mcp_servers.len()
        ),
        None => "cmd:0 skill:0 hook:0 mcp:0".to_string(),
    }
}

fn source_label(source: &PluginSourceConfig) -> String {
    match source {
        PluginSourceConfig::LocalPath { path } => format!("local:{path}"),
        PluginSourceConfig::Marketplace {
            marketplace,
            package,
            version,
        } => match version {
            Some(version) => format!("marketplace:{marketplace}/{package}@{version}"),
            None => format!("marketplace:{marketplace}/{package}"),
        },
        PluginSourceConfig::Builtin { name } => format!("builtin:{name}"),
    }
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_config::HelloxConfig;

    use super::{
        format_plugin_detail, format_plugin_list, inspect_plugin, install_plugin,
        load_installed_plugins, remove_plugin, set_plugin_enabled,
    };

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("hellox-plugin-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    fn write_manifest(root: &Path, plugin_id: &str) {
        let manifest_root = root.join(".hellox-plugin");
        fs::create_dir_all(&manifest_root).expect("create manifest root");
        fs::write(
            manifest_root.join("plugin.json"),
            format!(
                r#"{{
  "id": "{plugin_id}",
  "name": "Filesystem Plugin",
  "version": "0.1.0",
  "description": "Plugin used by tests",
  "commands": ["plugin.inspect"],
  "skills": ["filesystem"],
  "hooks": ["pre_tool"],
  "mcp_servers": ["filesystem"]
}}"#
            ),
        )
        .expect("write manifest");
    }

    #[test]
    fn install_list_toggle_and_remove_plugins() {
        let source = temp_dir();
        write_manifest(&source, "filesystem");
        fs::write(source.join("README.md"), "# plugin").expect("write readme");

        let plugins_root = temp_dir().join("installed");
        let mut config = HelloxConfig::default();

        let install = install_plugin(&mut config, &source, &plugins_root, true).expect("install");
        assert_eq!(install.plugin_id, "filesystem");

        let listed = load_installed_plugins(&config);
        assert_eq!(listed.len(), 1);
        let rendered = format_plugin_list(&listed);
        assert!(rendered.contains("filesystem"));

        set_plugin_enabled(&mut config, "filesystem", false).expect("disable");
        let detail = format_plugin_detail(&inspect_plugin(&config, "filesystem").expect("plugin"));
        assert!(detail.contains("enabled: false"));
        assert!(detail.contains("commands: plugin.inspect"));

        let removal = remove_plugin(&mut config, "filesystem", &plugins_root).expect("remove");
        assert_eq!(removal.plugin_id, "filesystem");
        assert!(!config.plugins.installed.contains_key("filesystem"));
    }
}
