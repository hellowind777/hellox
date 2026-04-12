use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRecord {
    pub name: String,
    pub repo_root: PathBuf,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitWorktreeAction {
    Keep,
    Remove,
    ForceRemove,
}

pub fn enter_worktree(
    working_directory: &Path,
    name: &str,
    base_ref: Option<&str>,
    reuse_existing: bool,
) -> Result<WorktreeRecord> {
    let name = normalize_worktree_name(name)?;
    let repo_root = repo_root_for(working_directory)?;
    let path = repo_root.join(".hellox").join("worktrees").join(&name);

    if path.exists() {
        if !reuse_existing {
            bail!("worktree `{name}` already exists at {}", path.display());
        }
        return inspect_existing_worktree(&name, &repo_root, &path);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create worktree dir {}", parent.display()))?;
    }

    let target_ref = base_ref.unwrap_or("HEAD");
    git_in(
        &repo_root,
        &[
            "worktree",
            "add",
            "--detach",
            &normalize_path(&path),
            target_ref,
        ],
    )?;

    inspect_existing_worktree(&name, &repo_root, &path)
}

pub fn exit_worktree(
    working_directory: &Path,
    name: Option<&str>,
    raw_path: Option<&str>,
    action: ExitWorktreeAction,
) -> Result<WorktreeRecord> {
    let repo_root = repo_root_for(working_directory)?;
    let path = resolve_worktree_path(working_directory, &repo_root, name, raw_path)?;
    let worktree = inspect_existing_worktree(&worktree_name_from_path(&path)?, &repo_root, &path)?;

    match action {
        ExitWorktreeAction::Keep => Ok(worktree),
        ExitWorktreeAction::Remove | ExitWorktreeAction::ForceRemove => {
            ensure_managed_worktree_path(&repo_root, &path)?;
            let mut args = vec!["worktree", "remove"];
            if action == ExitWorktreeAction::ForceRemove {
                args.push("--force");
            }
            let normalized = normalize_path(&path);
            args.push(&normalized);
            git_in(&repo_root, &args)?;
            git_in(&repo_root, &["worktree", "prune"])?;
            Ok(worktree)
        }
    }
}

pub fn default_worktree_name(label: Option<&str>) -> String {
    let base = label
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(sanitize_worktree_segment)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "agent".to_string());
    format!("{base}-{}", unix_timestamp())
}

fn inspect_existing_worktree(name: &str, repo_root: &Path, path: &Path) -> Result<WorktreeRecord> {
    let is_worktree = git_output_in(path, &["rev-parse", "--is-inside-work-tree"])?;
    if is_worktree != "true" {
        bail!("path is not a valid git worktree: {}", path.display());
    }

    let head = git_output_in(path, &["rev-parse", "--short", "HEAD"])?;
    let branch = git_output_in(path, &["branch", "--show-current"])
        .ok()
        .filter(|value| !value.trim().is_empty());
    Ok(WorktreeRecord {
        name: name.to_string(),
        repo_root: repo_root.to_path_buf(),
        path: path.to_path_buf(),
        branch,
        head,
    })
}

fn resolve_worktree_path(
    working_directory: &Path,
    repo_root: &Path,
    name: Option<&str>,
    raw_path: Option<&str>,
) -> Result<PathBuf> {
    match (name, raw_path) {
        (Some(name), None) => Ok(repo_root
            .join(".hellox")
            .join("worktrees")
            .join(normalize_worktree_name(name)?)),
        (None, Some(raw)) => {
            let candidate = PathBuf::from(raw);
            Ok(if candidate.is_absolute() {
                candidate
            } else {
                working_directory.join(candidate)
            })
        }
        (Some(_), Some(_)) => bail!("provide either `name` or `path`, not both"),
        (None, None) => bail!("either `name` or `path` is required"),
    }
}

fn ensure_managed_worktree_path(repo_root: &Path, path: &Path) -> Result<()> {
    let managed_root = repo_root.join(".hellox").join("worktrees");
    if !path.starts_with(&managed_root) {
        bail!(
            "managed worktree operations are limited to {}",
            managed_root.display()
        );
    }
    Ok(())
}

fn worktree_name_from_path(path: &Path) -> Result<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| {
            anyhow!(
                "worktree path is missing a terminal name: {}",
                path.display()
            )
        })
}

fn repo_root_for(working_directory: &Path) -> Result<PathBuf> {
    let root = git_output_in(working_directory, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(root))
}

fn git_in(directory: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", directory.display()))?;
    if !output.status.success() {
        bail!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        );
    }
    Ok(())
}

fn git_output_in(directory: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", directory.display()))?;
    if !output.status.success() {
        bail!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn normalize_worktree_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("worktree name cannot be empty");
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("worktree name may only contain letters, numbers, '-', '_' or '.'");
    }
    Ok(trimmed.to_string())
}

fn sanitize_worktree_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests;
