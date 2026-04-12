use std::collections::BTreeMap;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use hellox_config::config_root;
use serde::{Deserialize, Serialize};

use super::AppLanguage;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WorkspaceTrustStore {
    #[serde(default)]
    trusted_workspaces: BTreeMap<String, TrustedWorkspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedWorkspace {
    path: String,
    accepted_at: u64,
}

pub fn ensure_workspace_trusted(language: AppLanguage, working_directory: &Path) -> Result<bool> {
    if is_workspace_trusted(working_directory)? {
        return Ok(true);
    }

    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return Ok(true);
    }

    prompt_for_workspace_trust(language, working_directory)
}

fn is_workspace_trusted(working_directory: &Path) -> Result<bool> {
    let normalized = normalize_workspace_path(working_directory)?;
    let store = load_trust_store()?;
    Ok(store.trusted_workspaces.contains_key(&normalized))
}

fn prompt_for_workspace_trust(language: AppLanguage, working_directory: &Path) -> Result<bool> {
    let normalized = normalize_workspace_path(working_directory)?;
    println!();
    for line in trust_prompt_lines(language, &normalized) {
        println!("{line}");
    }

    loop {
        print!("{}", prompt_label(language));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if language.accepts_input(&input) {
            trust_workspace(working_directory)?;
            println!();
            return Ok(true);
        }
        if language.rejects_input(&input) {
            println!();
            return Ok(false);
        }
        println!("{}", invalid_choice_text(language));
    }
}

fn trust_workspace(working_directory: &Path) -> Result<()> {
    let normalized = normalize_workspace_path(working_directory)?;
    let mut store = load_trust_store()?;
    store.trusted_workspaces.insert(
        normalized.clone(),
        TrustedWorkspace {
            path: normalized,
            accepted_at: unix_timestamp(),
        },
    );
    save_trust_store(&store)
}

fn load_trust_store() -> Result<WorkspaceTrustStore> {
    let path = workspace_trust_path();
    if !path.exists() {
        return Ok(WorkspaceTrustStore::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read trust store {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse trust store {}", path.display()))
}

fn save_trust_store(store: &WorkspaceTrustStore) -> Result<()> {
    let path = workspace_trust_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create trust dir {}", parent.display()))?;
    }
    let raw =
        serde_json::to_string_pretty(store).context("failed to serialize workspace trust store")?;
    fs::write(&path, raw).with_context(|| format!("failed to write trust store {}", path.display()))
}

fn workspace_trust_path() -> PathBuf {
    config_root().join("workspace-trust.json")
}

fn normalize_workspace_path(path: &Path) -> Result<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let resolved = absolute.canonicalize().unwrap_or(absolute);
    let normalized = resolved.display().to_string().replace('\\', "/");
    #[cfg(windows)]
    {
        return Ok(normalized.to_ascii_lowercase());
    }
    #[cfg(not(windows))]
    {
        Ok(normalized)
    }
}

fn trust_prompt_lines(language: AppLanguage, working_directory: &str) -> Vec<String> {
    match language {
        AppLanguage::English => vec![
            "Accessing workspace:".to_string(),
            format!("  {working_directory}"),
            "Quick safety check: Is this a project you created or one you trust?".to_string(),
            "hellox will be able to read, edit, and execute files here.".to_string(),
            "  1. Yes, I trust this folder".to_string(),
            "  2. No, exit".to_string(),
        ],
        AppLanguage::SimplifiedChinese => vec![
            "正在访问工作区：".to_string(),
            format!("  {working_directory}"),
            "安全确认：这是你创建的项目，或你信任的代码目录吗？".to_string(),
            "继续后，hellox 将可以在这里读取、编辑并执行文件。".to_string(),
            "  1. 是的，我信任这个目录".to_string(),
            "  2. 不，退出".to_string(),
        ],
    }
}

fn prompt_label(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Select [1/2]: ",
        AppLanguage::SimplifiedChinese => "请选择 [1/2]：",
    }
}

fn invalid_choice_text(language: AppLanguage) -> &'static str {
    match language {
        AppLanguage::English => "Please enter 1 to trust this folder, or 2 to exit.",
        AppLanguage::SimplifiedChinese => "请输入 1 表示信任当前目录，或输入 2 退出。",
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        normalize_workspace_path, save_trust_store, workspace_trust_path, WorkspaceTrustStore,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-workspace-trust-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn normalize_workspace_path_uses_forward_slashes() {
        let root = temp_root();
        let normalized = normalize_workspace_path(&root).expect("normalize path");
        assert!(normalized.contains('/'));
        assert!(!normalized.contains('\\'));
    }

    #[test]
    fn save_trust_store_creates_json_file() {
        let root = temp_root();
        let current_home = env::var_os("HOME");
        let current_user_profile = env::var_os("USERPROFILE");
        env::set_var("HOME", &root);
        env::set_var("USERPROFILE", &root);

        let result = save_trust_store(&WorkspaceTrustStore::default());
        let trust_path = workspace_trust_path();

        if let Some(value) = current_home {
            env::set_var("HOME", value);
        } else {
            env::remove_var("HOME");
        }
        if let Some(value) = current_user_profile {
            env::set_var("USERPROFILE", value);
        } else {
            env::remove_var("USERPROFILE");
        }

        assert!(result.is_ok());
        assert!(trust_path.exists());
    }
}
