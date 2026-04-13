use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use crossterm::cursor::MoveUp;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::ExecutableCommand;
use hellox_config::config_root_for;
use serde::{Deserialize, Serialize};

use super::trust_copy::{
    fallback_notice_text, invalid_choice_text, prompt_label, trust_dialog_lines, TrustChoice,
    TrustMode, TrustSelection,
};
use super::AppLanguage;

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_DIM: &str = "\x1b[2m";
const ANSI_WARNING: &str = "\x1b[33m";
const ANSI_SUGGESTION: &str = "\x1b[36m";

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

pub fn ensure_workspace_trusted(
    config_path: &Path,
    language: AppLanguage,
    working_directory: &Path,
) -> Result<bool> {
    let normalized = normalize_workspace_path(working_directory)?;
    let mode = resolve_trust_mode(working_directory)?;

    if is_workspace_trusted(config_path, &normalized, mode)? {
        return Ok(true);
    }

    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return Ok(true);
    }

    prompt_for_workspace_trust(config_path, language, working_directory, &normalized, mode)
}

fn is_workspace_trusted(config_path: &Path, normalized: &str, mode: TrustMode) -> Result<bool> {
    match mode {
        TrustMode::RememberWorkspace => {
            let store = load_trust_store(config_path)?;
            Ok(store.trusted_workspaces.contains_key(normalized))
        }
        TrustMode::SessionOnly => Ok(session_trust_cache()
            .lock()
            .expect("session trust cache poisoned")
            .contains(normalized)),
    }
}

fn prompt_for_workspace_trust(
    config_path: &Path,
    language: AppLanguage,
    working_directory: &Path,
    normalized: &str,
    mode: TrustMode,
) -> Result<bool> {
    let display_path = display_workspace_path(working_directory)?;
    let choice = match prompt_for_workspace_trust_interactive(language, &display_path, mode) {
        Ok(choice) => choice,
        Err(error) => {
            println!();
            println!("{}", fallback_notice_text(language));
            println!("{error}");
            prompt_for_workspace_trust_fallback(language, &display_path, mode)?
        }
    };

    match choice {
        TrustChoice::Trust => {
            remember_workspace_trust(config_path, working_directory, normalized, mode)?;
            Ok(true)
        }
        TrustChoice::Exit => Ok(false),
    }
}

fn prompt_for_workspace_trust_interactive(
    language: AppLanguage,
    display_path: &str,
    _mode: TrustMode,
) -> Result<TrustChoice> {
    let _raw_mode = RawModeGuard::activate()?;
    let mut stdout = io::stdout();
    let mut rendered_line_count = 0usize;
    let mut selection = TrustSelection::Trust;
    let mut exit_pending = false;

    loop {
        rendered_line_count = redraw_trust_dialog(
            &mut stdout,
            rendered_line_count,
            &trust_dialog_lines(language, display_path, selection, exit_pending),
        )?;

        match event::read().context("failed to read trust dialog input")? {
            Event::Key(key) if is_key_press(key) => {
                match resolve_key_action(language, key, exit_pending) {
                    TrustKeyAction::MovePrevious => {
                        selection = selection.previous();
                        exit_pending = false;
                    }
                    TrustKeyAction::MoveNext => {
                        selection = selection.next();
                        exit_pending = false;
                    }
                    TrustKeyAction::Confirm => {
                        clear_rendered_dialog(&mut stdout, rendered_line_count)?;
                        return Ok(selection.choice());
                    }
                    TrustKeyAction::ConfirmTrust => {
                        clear_rendered_dialog(&mut stdout, rendered_line_count)?;
                        return Ok(TrustChoice::Trust);
                    }
                    TrustKeyAction::ConfirmExit => {
                        clear_rendered_dialog(&mut stdout, rendered_line_count)?;
                        return Ok(TrustChoice::Exit);
                    }
                    TrustKeyAction::Cancel => {
                        clear_rendered_dialog(&mut stdout, rendered_line_count)?;
                        return Ok(TrustChoice::Exit);
                    }
                    TrustKeyAction::ArmExit => {
                        exit_pending = true;
                    }
                    TrustKeyAction::ExitImmediately => {
                        clear_rendered_dialog(&mut stdout, rendered_line_count)?;
                        return Ok(TrustChoice::Exit);
                    }
                    TrustKeyAction::Ignore => {}
                }
            }
            _ => {}
        }
    }
}

fn prompt_for_workspace_trust_fallback(
    language: AppLanguage,
    display_path: &str,
    _mode: TrustMode,
) -> Result<TrustChoice> {
    for line in trust_dialog_lines(language, display_path, TrustSelection::Trust, false) {
        println!("{line}");
    }

    loop {
        print!("{}", prompt_label(language));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match TrustChoice::from_input(language, &input) {
            Some(choice) => {
                println!();
                return Ok(choice);
            }
            None => println!("{}", invalid_choice_text(language)),
        }
    }
}

fn redraw_trust_dialog(
    stdout: &mut io::Stdout,
    previous_line_count: usize,
    lines: &[String],
) -> Result<usize> {
    if previous_line_count > 0 {
        stdout
            .execute(MoveUp(previous_line_count as u16))
            .context("failed to reposition trust dialog cursor")?;
        stdout
            .execute(Clear(ClearType::FromCursorDown))
            .context("failed to clear trust dialog")?;
    }

    for line in lines {
        writeln!(stdout, "{}", style_trust_dialog_line(lines, line))?;
    }
    stdout.flush()?;
    Ok(lines.len())
}

fn style_trust_dialog_line(lines: &[String], line: &str) -> String {
    let trimmed = line.trim_start();
    if line.starts_with('╭') {
        return colorize(ANSI_WARNING, line);
    }
    if matches!(trimmed, "Accessing workspace:" | "正在访问工作区：") {
        return colorize(&format!("{ANSI_BOLD}{ANSI_WARNING}"), line);
    }
    if is_trust_path_line(lines, line) {
        return colorize(ANSI_BOLD, line);
    }
    if trimmed.starts_with('❯') {
        return colorize(&format!("{ANSI_BOLD}{ANSI_SUGGESTION}"), line);
    }
    if trimmed.starts_with("Security guide") || trimmed.starts_with("安全指南") {
        return colorize(ANSI_DIM, line);
    }
    if is_trust_footer_line(trimmed) {
        return colorize(ANSI_DIM, line);
    }
    line.to_string()
}

fn colorize(prefix: &str, value: &str) -> String {
    format!("{prefix}{value}{ANSI_RESET}")
}

fn is_trust_footer_line(trimmed: &str) -> bool {
    trimmed.starts_with("Enter ")
        || trimmed.starts_with("Press Ctrl+C")
        || trimmed.starts_with("再按一次 Ctrl+C")
        || trimmed.starts_with("Enter 确认")
}

fn is_trust_path_line(lines: &[String], line: &str) -> bool {
    if line.trim().is_empty() {
        return false;
    }

    let Some(title_index) = lines.iter().position(|candidate| {
        matches!(
            candidate.trim_start(),
            "Accessing workspace:" | "正在访问工作区："
        )
    }) else {
        return false;
    };

    let Some(path_start) = lines
        .iter()
        .skip(title_index + 1)
        .position(|candidate| !candidate.trim().is_empty())
        .map(|offset| title_index + 1 + offset)
    else {
        return false;
    };

    let path_end = lines
        .iter()
        .skip(path_start)
        .position(|candidate| candidate.trim().is_empty())
        .map(|offset| path_start + offset)
        .unwrap_or(lines.len());

    lines[path_start..path_end]
        .iter()
        .any(|candidate| candidate == line)
}

fn clear_rendered_dialog(stdout: &mut io::Stdout, line_count: usize) -> Result<()> {
    if line_count == 0 {
        return Ok(());
    }

    stdout
        .execute(MoveUp(line_count as u16))
        .context("failed to reposition trust dialog cursor for clear")?;
    stdout
        .execute(Clear(ClearType::FromCursorDown))
        .context("failed to clear rendered trust dialog")?;
    stdout.flush()?;
    Ok(())
}

fn remember_workspace_trust(
    config_path: &Path,
    working_directory: &Path,
    normalized: &str,
    mode: TrustMode,
) -> Result<()> {
    match mode {
        TrustMode::RememberWorkspace => save_persisted_workspace_trust(config_path, normalized),
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

fn save_persisted_workspace_trust(config_path: &Path, normalized: &str) -> Result<()> {
    let mut store = load_trust_store(config_path)?;
    store.trusted_workspaces.insert(
        normalized.to_string(),
        TrustedWorkspace {
            path: normalized.to_string(),
            accepted_at: unix_timestamp(),
        },
    );
    save_trust_store(config_path, &store)
}

fn load_trust_store(config_path: &Path) -> Result<WorkspaceTrustStore> {
    let path = workspace_trust_path_for(config_path);
    if !path.exists() {
        return Ok(WorkspaceTrustStore::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read trust store {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse trust store {}", path.display()))
}

fn save_trust_store(config_path: &Path, store: &WorkspaceTrustStore) -> Result<()> {
    let path = workspace_trust_path_for(config_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create trust dir {}", parent.display()))?;
    }
    let raw =
        serde_json::to_string_pretty(store).context("failed to serialize workspace trust store")?;
    fs::write(&path, raw).with_context(|| format!("failed to write trust store {}", path.display()))
}

fn workspace_trust_path_for(config_path: &Path) -> PathBuf {
    config_root_for(config_path).join("workspace-trust.json")
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

fn display_workspace_path(path: &Path) -> Result<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let resolved = absolute.canonicalize().unwrap_or(absolute);
    let display = resolved.display().to_string();
    #[cfg(windows)]
    {
        return Ok(display.trim_start_matches(r"\\?\").to_string());
    }
    #[cfg(not(windows))]
    {
        Ok(display)
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

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrustKeyAction {
    MovePrevious,
    MoveNext,
    Confirm,
    ConfirmTrust,
    ConfirmExit,
    Cancel,
    ArmExit,
    ExitImmediately,
    Ignore,
}

struct RawModeGuard;

impl RawModeGuard {
    fn activate() -> Result<Self> {
        enable_raw_mode().context("failed to enable raw mode for trust dialog")?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn is_key_press(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn resolve_key_action(language: AppLanguage, key: KeyEvent, exit_pending: bool) -> TrustKeyAction {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        return if exit_pending {
            TrustKeyAction::ExitImmediately
        } else {
            TrustKeyAction::ArmExit
        };
    }

    match key.code {
        KeyCode::Up | KeyCode::Left => TrustKeyAction::MovePrevious,
        KeyCode::Down | KeyCode::Right => TrustKeyAction::MoveNext,
        KeyCode::Enter => TrustKeyAction::Confirm,
        KeyCode::Esc => TrustKeyAction::Cancel,
        KeyCode::Char('1') => TrustKeyAction::ConfirmTrust,
        KeyCode::Char('2') => TrustKeyAction::ConfirmExit,
        KeyCode::Char(ch) if matches_single_key_accept(language, ch) => {
            TrustKeyAction::ConfirmTrust
        }
        KeyCode::Char(ch) if matches_single_key_reject(language, ch) => TrustKeyAction::ConfirmExit,
        _ => TrustKeyAction::Ignore,
    }
}

fn matches_single_key_accept(language: AppLanguage, value: char) -> bool {
    match language {
        AppLanguage::English => matches!(value, 'y' | 'Y'),
        AppLanguage::SimplifiedChinese => matches!(value, 'y' | 'Y' | '是'),
    }
}

fn matches_single_key_reject(language: AppLanguage, value: char) -> bool {
    match language {
        AppLanguage::English => matches!(value, 'n' | 'N'),
        AppLanguage::SimplifiedChinese => matches!(value, 'n' | 'N' | '否'),
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        display_workspace_path, normalize_workspace_path, remember_workspace_trust,
        resolve_trust_mode, save_trust_store, session_trust_cache, workspace_trust_path_for,
        WorkspaceTrustStore,
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
    fn display_workspace_path_avoids_extended_windows_prefix() {
        let root = temp_root();
        let display = display_workspace_path(&root).expect("display path");
        assert!(!display.starts_with(r"\\?\"));
        #[cfg(windows)]
        assert!(display.contains('\\'));
    }

    #[test]
    fn save_trust_store_creates_json_file() {
        let _guard = super::super::test_support::env_lock();
        let root = temp_root();
        let config_path = root.join(".hellox").join("config.toml");
        let current_home = env::var_os("HOME");
        let current_user_profile = env::var_os("USERPROFILE");
        env::set_var("HOME", &root);
        env::set_var("USERPROFILE", &root);

        let result = save_trust_store(&config_path, &WorkspaceTrustStore::default());
        let trust_path = workspace_trust_path_for(&config_path);

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
        let config_path = root.join(".hellox").join("config.toml");
        let current_home = env::var_os("HOME");
        let current_user_profile = env::var_os("USERPROFILE");
        env::set_var("HOME", &root);
        env::set_var("USERPROFILE", &root);

        let normalized = normalize_workspace_path(&root).expect("normalize path");
        let trust_path = workspace_trust_path_for(&config_path);
        let original_store = fs::read_to_string(&trust_path).ok();
        remember_workspace_trust(&config_path, &root, &normalized, TrustMode::SessionOnly)
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

    #[test]
    fn trust_store_path_follows_config_path_not_global_home() {
        let _guard = super::super::test_support::env_lock();
        let global_home = temp_root();
        let isolated_root = temp_root();
        let config_path = isolated_root.join(".hellox").join("config.toml");
        let current_home = env::var_os("HOME");
        let current_user_profile = env::var_os("USERPROFILE");
        env::set_var("HOME", &global_home);
        env::set_var("USERPROFILE", &global_home);

        save_trust_store(&config_path, &WorkspaceTrustStore::default()).expect("save trust store");
        let scoped_path = workspace_trust_path_for(&config_path);
        let global_path = global_home.join(".hellox").join("workspace-trust.json");

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

        assert!(scoped_path.exists());
        assert!(!global_path.exists());
    }
}
