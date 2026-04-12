use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use hellox_config::config_root;
use hellox_tui::render_cards;
use serde::{Deserialize, Serialize};

use super::trust_copy::{
    accepted_cards, dialog_title, invalid_choice_text, prompt_label, trust_cards, TrustChoice,
    TrustMode,
};
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
    let normalized = normalize_workspace_path(working_directory)?;
    let mode = resolve_trust_mode(working_directory)?;

    if is_workspace_trusted(&normalized, mode)? {
        return Ok(true);
    }

    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return Ok(true);
    }

    prompt_for_workspace_trust(language, working_directory, &normalized, mode)
}

fn is_workspace_trusted(normalized: &str, mode: TrustMode) -> Result<bool> {
    match mode {
        TrustMode::RememberWorkspace => {
            let store = load_trust_store()?;
            Ok(store.trusted_workspaces.contains_key(normalized))
        }
        TrustMode::SessionOnly => Ok(session_trust_cache()
            .lock()
            .expect("session trust cache poisoned")
            .contains(normalized)),
    }
}

fn prompt_for_workspace_trust(
    language: AppLanguage,
    working_directory: &Path,
    normalized: &str,
    mode: TrustMode,
) -> Result<bool> {
    let store_path = workspace_trust_path()
        .display()
        .to_string()
        .replace('\\', "/");

    println!();
    print_title(language, dialog_title(language));
    for line in render_cards(&trust_cards(language, normalized, mode, &store_path)) {
        println!("{line}");
    }

    loop {
        print!("{}", prompt_label(language, mode));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match TrustChoice::from_input(language, &input) {
            Some(TrustChoice::Trust) => {
                remember_workspace_trust(working_directory, normalized, mode)?;
                println!();
                for line in render_cards(&accepted_cards(language, normalized, mode)) {
                    println!("{line}");
                }
                println!();
                return Ok(true);
            }
            Some(TrustChoice::Exit) => {
                println!();
                return Ok(false);
            }
            None => println!("{}", invalid_choice_text(language, mode)),
        }
    }
}

fn remember_workspace_trust(
    working_directory: &Path,
    normalized: &str,
    mode: TrustMode,
) -> Result<()> {
    match mode {
        TrustMode::RememberWorkspace => save_persisted_workspace_trust(normalized),
        TrustMode::SessionOnly => {
            let _ = working_directory;
            session_trust_cache()
                .lock()
                .expect("session trust cache poisoned")
                .insert(normalized.to_string());
            Ok(())
        }
    }
}

fn save_persisted_workspace_trust(normalized: &str) -> Result<()> {
    let mut store = load_trust_store()?;
    store.trusted_workspaces.insert(
        normalized.to_string(),
        TrustedWorkspace {
            path: normalized.to_string(),
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

fn resolve_trust_mode(working_directory: &Path) -> Result<TrustMode> {
    let normalized = normalize_workspace_path(working_directory)?;
    let home = user_home_dir()
        .map(|path| normalize_workspace_path(&path))
        .transpose()?;

    if home.as_deref() == Some(normalized.as_str()) {
        return Ok(TrustMode::SessionOnly);
    }

    Ok(TrustMode::RememberWorkspace)
}

fn normalize_workspace_path(path: &Path) -> Result<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let resolved = absolute.canonicalize().unwrap_or(absolute);
    let normalized = resolved
        .display()
        .to_string()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .to_string();
    #[cfg(windows)]
    {
        return Ok(normalized.to_ascii_lowercase());
    }
    #[cfg(not(windows))]
    {
        Ok(normalized)
    }
}

fn session_trust_cache() -> &'static Mutex<HashSet<String>> {
    static SESSION_TRUST: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SESSION_TRUST.get_or_init(|| Mutex::new(HashSet::new()))
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn print_title(language: AppLanguage, title: &str) {
    println!();
    match language {
        AppLanguage::English => println!("{title}\n{}", "·".repeat(title.len().max(20))),
        AppLanguage::SimplifiedChinese => println!("{title}\n{}", "·".repeat(24)),
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
        normalize_workspace_path, remember_workspace_trust, resolve_trust_mode, save_trust_store,
        session_trust_cache, workspace_trust_path, WorkspaceTrustStore,
    };
    use crate::startup::trust_copy::TrustMode;

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
        let _guard = super::super::test_support::env_lock();
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

    #[test]
    fn home_directory_uses_session_only_trust_mode() {
        let _guard = super::super::test_support::env_lock();
        let root = temp_root();
        let project = root.join("project");
        fs::create_dir_all(&project).expect("create project");
        let current_home = env::var_os("HOME");
        let current_user_profile = env::var_os("USERPROFILE");
        env::set_var("HOME", &root);
        env::set_var("USERPROFILE", &root);

        let home_mode = resolve_trust_mode(&root).expect("resolve home mode");
        let project_mode = resolve_trust_mode(&project).expect("resolve project mode");

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

        assert_eq!(home_mode, TrustMode::SessionOnly);
        assert_eq!(project_mode, TrustMode::RememberWorkspace);
    }

    #[test]
    fn session_only_trust_does_not_write_persistent_store() {
        let _guard = super::super::test_support::env_lock();
        let root = temp_root();
        let current_home = env::var_os("HOME");
        let current_user_profile = env::var_os("USERPROFILE");
        env::set_var("HOME", &root);
        env::set_var("USERPROFILE", &root);

        let normalized = normalize_workspace_path(&root).expect("normalize path");
        let trust_path = workspace_trust_path();
        let original_store = fs::read_to_string(&trust_path).ok();
        remember_workspace_trust(&root, &normalized, TrustMode::SessionOnly)
            .expect("remember session trust");
        let is_cached = session_trust_cache()
            .lock()
            .expect("session trust cache poisoned")
            .contains(&normalized);
        let updated_store = fs::read_to_string(&trust_path).ok();

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

        assert!(is_cached);
        assert_eq!(updated_store, original_store);
    }
}
