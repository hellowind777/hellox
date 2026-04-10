use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use hellox_config::config_root;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookDefinition {
    pub name: String,
    pub scope: String,
    pub path: PathBuf,
    pub preview: String,
}

pub fn discover_hooks(cwd: &Path) -> Result<Vec<HookDefinition>> {
    let mut hooks = Vec::new();
    collect_hooks(&config_root().join("hooks"), "user", &mut hooks)?;
    collect_hooks(&cwd.join(".hellox").join("hooks"), "project", &mut hooks)?;
    hooks.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.scope.cmp(&right.scope))
    });
    Ok(hooks)
}

pub fn find_hook<'a>(hooks: &'a [HookDefinition], name: &str) -> Result<&'a HookDefinition> {
    hooks
        .iter()
        .find(|hook| hook.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| anyhow!("Hook `{name}` was not found"))
}

pub fn format_hook_list(hooks: &[HookDefinition]) -> String {
    if hooks.is_empty() {
        return "No hooks discovered.".to_string();
    }

    let mut lines = vec!["name\tscope\tpath\tpreview".to_string()];
    for hook in hooks {
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            hook.name,
            hook.scope,
            normalize_path(&hook.path),
            hook.preview
        ));
    }
    lines.join("\n")
}

pub fn format_hook_detail(hook: &HookDefinition) -> String {
    format!(
        "name: {}\nscope: {}\npath: {}\npreview: {}",
        hook.name,
        hook.scope,
        normalize_path(&hook.path),
        hook.preview
    )
}

fn collect_hooks(root: &Path, scope: &str, hooks: &mut Vec<HookDefinition>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for path in collect_files(root)? {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read hook file {}", path.display()))?;
        let preview = raw
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("(empty)")
            .to_string();
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .ok_or_else(|| anyhow!("hook file `{}` is missing a name", path.display()))?;
        hooks.push(HookDefinition {
            name,
            scope: scope.to_string(),
            path,
            preview,
        });
    }

    Ok(())
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read hooks root {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{discover_hooks, find_hook, format_hook_detail, format_hook_list};

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("hellox-hooks-{suffix}"));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }

    #[test]
    fn discovers_and_formats_project_hooks() {
        let root = temp_dir();
        let hooks_root = root.join(".hellox").join("hooks");
        fs::create_dir_all(&hooks_root).expect("create hooks root");
        fs::write(
            hooks_root.join("pre_tool.ps1"),
            "Write-Host 'before tool'\n",
        )
        .expect("write hook");

        let hooks = discover_hooks(&root).expect("discover hooks");
        assert_eq!(hooks.len(), 1);

        let rendered = format_hook_list(&hooks);
        assert!(rendered.contains("pre_tool"));
        let detail = format_hook_detail(find_hook(&hooks, "pre_tool").expect("find hook"));
        assert!(detail.contains("before tool"));
    }
}
