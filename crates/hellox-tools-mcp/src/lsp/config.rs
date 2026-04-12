use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use hellox_config::{HelloxConfig, LspServerConfig};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedLspServer {
    pub(crate) name: String,
    pub(crate) config: LspServerConfig,
    pub(crate) file_path: PathBuf,
    pub(crate) workspace_root: PathBuf,
    pub(crate) language_id: String,
}

pub(crate) fn resolve_server(
    config: &HelloxConfig,
    working_directory: &Path,
    raw_file_path: &str,
) -> Result<ResolvedLspServer> {
    let file_path = if Path::new(raw_file_path).is_absolute() {
        PathBuf::from(raw_file_path)
    } else {
        working_directory.join(raw_file_path)
    };
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("LSP file path `{raw_file_path}` does not have an extension"))?;

    let (name, server) = config
        .lsp
        .servers
        .iter()
        .find(|(_, server)| server.enabled && server.matches_extension(extension))
        .ok_or_else(|| {
            let configured = config
                .lsp
                .servers
                .iter()
                .filter(|(_, server)| server.enabled)
                .map(|(name, server)| {
                    let extensions = if server.file_extensions.is_empty() {
                        "(no extensions)".to_string()
                    } else {
                        server.file_extensions.join(", ")
                    };
                    format!("{name} [{extensions}]")
                })
                .collect::<Vec<_>>();
            if configured.is_empty() {
                anyhow!(
                    "no enabled LSP servers are configured; add one under `[lsp.servers]` in config.toml"
                )
            } else {
                anyhow!(
                    "no enabled LSP server matches `.{extension}`; configured servers: {}",
                    configured.join("; ")
                )
            }
        })?;

    let workspace_root = detect_workspace_root(
        file_path.parent().unwrap_or(working_directory),
        &server.root_markers,
    )
    .unwrap_or_else(|| working_directory.to_path_buf());
    let language_id = server
        .language_id
        .clone()
        .unwrap_or_else(|| default_language_id(extension));

    Ok(ResolvedLspServer {
        name: name.clone(),
        config: server.clone(),
        file_path,
        workspace_root,
        language_id,
    })
}

fn detect_workspace_root(start: &Path, root_markers: &[String]) -> Option<PathBuf> {
    let markers = if root_markers.is_empty() {
        vec![".git".to_string()]
    } else {
        root_markers.to_vec()
    };

    let mut current = Some(start);
    while let Some(path) = current {
        if markers.iter().any(|marker| path.join(marker).exists()) {
            return Some(path.to_path_buf());
        }
        current = path.parent();
    }
    None
}

fn default_language_id(extension: &str) -> String {
    match extension.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "jsx" => "javascriptreact",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "json" => "json",
        "md" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cc" | "cpp" | "cxx" => "cpp",
        "cs" => "csharp",
        "php" => "php",
        "rb" => "ruby",
        "swift" => "swift",
        "kt" => "kotlin",
        "dart" => "dart",
        "lua" => "lua",
        "sh" => "shellscript",
        "ps1" => "powershell",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use hellox_config::{HelloxConfig, LspConfig, LspServerConfig};

    use super::resolve_server;

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("hellox-lsp-config-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn resolve_server_matches_extension_and_workspace_root() {
        let root = temp_dir();
        fs::create_dir_all(root.join(".git")).expect("create git root");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").expect("write file");

        let mut config = HelloxConfig::default();
        config.lsp = LspConfig {
            servers: BTreeMap::from([(
                "rust-analyzer".to_string(),
                LspServerConfig {
                    enabled: true,
                    description: None,
                    command: "rust-analyzer".to_string(),
                    args: Vec::new(),
                    env: BTreeMap::new(),
                    cwd: None,
                    language_id: None,
                    file_extensions: vec!["rs".to_string()],
                    root_markers: vec![".git".to_string()],
                },
            )]),
        };

        let resolved =
            resolve_server(&config, &root, "src/main.rs").expect("resolve rust-analyzer");
        assert_eq!(resolved.name, "rust-analyzer");
        assert_eq!(resolved.language_id, "rust");
        assert_eq!(resolved.workspace_root, root);
    }
}
