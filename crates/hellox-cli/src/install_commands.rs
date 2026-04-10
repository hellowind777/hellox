use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use hellox_config::config_root;

use crate::cli_install_types::{InstallCommands, UpgradeCommands};

pub(crate) fn handle_install_command(command: Option<InstallCommands>) -> Result<()> {
    println!(
        "{}",
        install_command_text(command.unwrap_or(InstallCommands::Status))?
    );
    Ok(())
}

pub(crate) fn handle_upgrade_command(command: Option<UpgradeCommands>) -> Result<()> {
    println!(
        "{}",
        upgrade_command_text(command.unwrap_or(UpgradeCommands::Status))?
    );
    Ok(())
}

pub(crate) fn install_command_text(command: InstallCommands) -> Result<String> {
    match command {
        InstallCommands::Status => install_status_text(),
        InstallCommands::Plan { source, target } => install_plan_text(source, target),
        InstallCommands::Apply {
            source,
            target,
            force,
        } => install_apply_text(source, target, force),
    }
}

pub(crate) fn upgrade_command_text(command: UpgradeCommands) -> Result<String> {
    match command {
        UpgradeCommands::Status => upgrade_status_text(),
        UpgradeCommands::Plan { source, target } => upgrade_plan_text(source, target),
        UpgradeCommands::Apply {
            source,
            target,
            backup,
            force,
        } => upgrade_apply_text(source, target, backup, force),
    }
}

fn install_status_text() -> Result<String> {
    lifecycle_status_text(
        "install",
        "Install copies the current or specified local binary into the stable `~/.hellox/bin` target.",
    )
}

fn upgrade_status_text() -> Result<String> {
    lifecycle_status_text(
        "upgrade",
        "Upgrade replaces the stable local install from an explicit local artifact without requiring a remote release feed.",
    )
}

fn lifecycle_status_text(action: &str, summary: &str) -> Result<String> {
    let current = current_executable()?;
    let target = default_install_target();
    let install_dir = default_install_dir();
    let mut lines = vec![
        format!("action: {action}"),
        format!("hellox_version: {}", env!("CARGO_PKG_VERSION")),
        format!("current_executable: {}", path_text(&current)),
        format!("current_channel: {}", detect_install_channel(&current)),
        format!("default_install_dir: {}", path_text(&install_dir)),
        format!("default_install_target: {}", path_text(&target)),
        format!("default_install_target_exists: {}", target.exists()),
        format!("config_root: {}", path_text(&config_root())),
        format!("path_ready: {}", path_dir_is_on_path(&install_dir)),
        format!("summary: {summary}"),
    ];
    if !path_dir_is_on_path(&install_dir) {
        lines.push(format!(
            "path_hint: add `{}` to PATH to invoke `hellox` globally",
            path_text(&install_dir)
        ));
    }
    Ok(lines.join("\n"))
}

fn install_plan_text(source: Option<PathBuf>, target: Option<PathBuf>) -> Result<String> {
    render_plan_text(
        "install",
        resolve_install_source(source)?,
        target.unwrap_or_else(default_install_target),
        false,
    )
}

fn upgrade_plan_text(source: PathBuf, target: Option<PathBuf>) -> Result<String> {
    render_plan_text(
        "upgrade",
        source,
        target.unwrap_or_else(default_install_target),
        true,
    )
}

fn render_plan_text(
    action: &str,
    source: PathBuf,
    target: PathBuf,
    include_backup: bool,
) -> Result<String> {
    ensure_source_exists(&source)?;
    let mut lines = vec![
        format!("action: {action}"),
        format!("source: {}", path_text(&source)),
        format!("target: {}", path_text(&target)),
        format!("target_exists: {}", target.exists()),
        format!(
            "same_location: {}",
            paths_point_to_same_location(&source, &target)?
        ),
    ];
    if include_backup {
        lines.push(format!("backup_path: {}", path_text(&backup_path(&target))));
    }
    lines.push(format!(
        "apply_hint: {}",
        apply_hint_command(action, &source, &target)
    ));
    Ok(lines.join("\n"))
}

fn install_apply_text(
    source: Option<PathBuf>,
    target: Option<PathBuf>,
    force: bool,
) -> Result<String> {
    let result = copy_binary(
        &resolve_install_source(source)?,
        &target.unwrap_or_else(default_install_target),
        false,
        force,
    )?;
    Ok(render_apply_text("Installed", &result))
}

fn upgrade_apply_text(
    source: PathBuf,
    target: Option<PathBuf>,
    backup: bool,
    force: bool,
) -> Result<String> {
    let result = copy_binary(
        &source,
        &target.unwrap_or_else(default_install_target),
        backup,
        force,
    )?;
    Ok(render_apply_text("Upgraded", &result))
}

fn render_apply_text(verb: &str, result: &CopyResult) -> String {
    let mut lines = vec![
        format!("result: {verb} hellox local binary"),
        format!("source: {}", path_text(&result.source)),
        format!("target: {}", path_text(&result.target)),
    ];
    if result.changed {
        lines.push(format!("copied_bytes: {}", result.bytes_copied));
    } else {
        lines.push("copied_bytes: 0".to_string());
        lines.push("note: source and target already point to the same file".to_string());
    }
    if let Some(backup) = &result.backup {
        lines.push(format!("backup: {}", path_text(backup)));
    }
    if let Some(parent) = result.target.parent() {
        lines.push(format!("path_ready: {}", path_dir_is_on_path(parent)));
        if !path_dir_is_on_path(parent) {
            lines.push(format!(
                "path_hint: add `{}` to PATH to invoke `hellox` globally",
                path_text(parent)
            ));
        }
    }
    lines.join("\n")
}

fn copy_binary(source: &Path, target: &Path, backup: bool, force: bool) -> Result<CopyResult> {
    ensure_source_exists(source)?;
    if paths_point_to_same_location(source, target)? {
        return Ok(CopyResult {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            backup: None,
            bytes_copied: 0,
            changed: false,
        });
    }

    if target.exists() && !force {
        bail!(
            "target already exists: {} (use `--force` to replace it)",
            path_text(target)
        );
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create install dir {}", path_text(parent)))?;
    }

    let backup_target = if backup && target.exists() {
        let backup = backup_path(target);
        fs::copy(target, &backup).with_context(|| {
            format!(
                "failed to create upgrade backup from {} to {}",
                path_text(target),
                path_text(&backup)
            )
        })?;
        Some(backup)
    } else {
        None
    };

    let bytes_copied = fs::copy(source, target).with_context(|| {
        format!(
            "failed to copy local binary from {} to {}",
            path_text(source),
            path_text(target)
        )
    })?;

    Ok(CopyResult {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        backup: backup_target,
        bytes_copied,
        changed: true,
    })
}

fn resolve_install_source(source: Option<PathBuf>) -> Result<PathBuf> {
    match source {
        Some(path) => Ok(path),
        None => current_executable(),
    }
}

fn current_executable() -> Result<PathBuf> {
    env::current_exe().context("failed to resolve current executable path")
}

fn default_install_dir() -> PathBuf {
    config_root().join("bin")
}

fn default_install_target() -> PathBuf {
    default_install_dir().join(default_binary_name())
}

fn default_binary_name() -> &'static str {
    if cfg!(windows) {
        "hellox.exe"
    } else {
        "hellox"
    }
}

fn backup_path(target: &Path) -> PathBuf {
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("hellox");
    target.with_file_name(format!("{file_name}.bak"))
}

fn detect_install_channel(path: &Path) -> &'static str {
    if path.starts_with(default_install_dir()) {
        "hellox_local_bin"
    } else if cargo_bin_dir()
        .map(|cargo_bin| path.starts_with(cargo_bin))
        .unwrap_or(false)
    {
        "cargo_bin"
    } else if path
        .components()
        .any(|component| component.as_os_str() == "target")
    {
        "workspace_target"
    } else {
        "standalone"
    }
}

fn cargo_bin_dir() -> Option<PathBuf> {
    env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .map(|path| path.join("bin"))
        .or_else(|| {
            env::var_os("HOME")
                .or_else(|| env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .map(|path| path.join(".cargo").join("bin"))
        })
}

fn path_dir_is_on_path(dir: &Path) -> bool {
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|entry| entry == dir)
}

fn ensure_source_exists(source: &Path) -> Result<()> {
    if !source.exists() {
        return Err(anyhow!(
            "source binary does not exist: {}",
            path_text(source)
        ));
    }
    if !source.is_file() {
        return Err(anyhow!(
            "source binary is not a file: {}",
            path_text(source)
        ));
    }
    Ok(())
}

fn paths_point_to_same_location(source: &Path, target: &Path) -> Result<bool> {
    let source = normalize_for_compare(source)?;
    let target = normalize_for_compare(target)?;
    Ok(source == target)
}

fn normalize_for_compare(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        fs::canonicalize(path)
            .with_context(|| format!("failed to resolve path {}", path_text(path)))
    } else if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .context("failed to resolve current dir")?
            .join(path))
    }
}

fn apply_hint_command(action: &str, source: &Path, target: &Path) -> String {
    match action {
        "install" => format!(
            "hellox install apply --source \"{}\" --target \"{}\" --force",
            path_text(source),
            path_text(target)
        ),
        _ => format!(
            "hellox upgrade apply --source \"{}\" --target \"{}\" --backup --force",
            path_text(source),
            path_text(target)
        ),
    }
}

fn path_text(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

struct CopyResult {
    source: PathBuf,
    target: PathBuf,
    backup: Option<PathBuf>,
    bytes_copied: u64,
    changed: bool,
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        default_install_target, install_command_text, upgrade_command_text, InstallCommands,
        UpgradeCommands,
    };

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-cli-install-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn install_status_reports_default_target() {
        let text = install_command_text(InstallCommands::Status).expect("install status");
        assert!(text.contains("action: install"));
        assert!(text.contains("default_install_target:"));
        assert!(text.contains("config_root:"));
    }

    #[test]
    fn install_apply_copies_binary_to_target() {
        let root = temp_dir();
        let source = root.join("build").join("hellox.exe");
        let target = root.join("bin").join("hellox.exe");
        fs::create_dir_all(source.parent().expect("source dir")).expect("create source dir");
        fs::write(&source, "binary-v1").expect("write source");

        let text = install_command_text(InstallCommands::Apply {
            source: Some(source.clone()),
            target: Some(target.clone()),
            force: false,
        })
        .expect("install apply");

        assert!(text.contains("Installed hellox local binary"));
        assert_eq!(
            fs::read_to_string(target).expect("read target"),
            "binary-v1"
        );
    }

    #[test]
    fn upgrade_plan_mentions_backup_path() {
        let root = temp_dir();
        let source = root.join("release").join("hellox.exe");
        fs::create_dir_all(source.parent().expect("release dir")).expect("create release dir");
        fs::write(&source, "binary-v2").expect("write source");

        let text = upgrade_command_text(UpgradeCommands::Plan {
            source: source.clone(),
            target: Some(default_install_target()),
        })
        .expect("upgrade plan");

        assert!(text.contains("action: upgrade"));
        assert!(text.contains("backup_path:"));
        assert!(text.contains("apply_hint: hellox upgrade apply"));
    }

    #[test]
    fn upgrade_apply_creates_backup_when_requested() {
        let root = temp_dir();
        let source = root.join("release").join("hellox.exe");
        let target = root.join("bin").join("hellox.exe");
        fs::create_dir_all(source.parent().expect("release dir")).expect("create release dir");
        fs::create_dir_all(target.parent().expect("bin dir")).expect("create bin dir");
        fs::write(&source, "binary-v2").expect("write source");
        fs::write(&target, "binary-v1").expect("write target");

        let text = upgrade_command_text(UpgradeCommands::Apply {
            source: source.clone(),
            target: Some(target.clone()),
            backup: true,
            force: true,
        })
        .expect("upgrade apply");

        assert!(text.contains("Upgraded hellox local binary"));
        assert_eq!(
            fs::read_to_string(&target).expect("read target"),
            "binary-v2"
        );
        assert_eq!(
            fs::read_to_string(root.join("bin").join("hellox.exe.bak")).expect("read backup"),
            "binary-v1"
        );
    }
}
