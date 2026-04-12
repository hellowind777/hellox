use std::path::PathBuf;

use anyhow::{anyhow, Result};

use super::super::ToolExecutionContext;

pub(super) fn resolve_child_working_directory(
    context: &ToolExecutionContext,
    cwd_override: Option<&str>,
    isolation: Option<&str>,
    worktree_name: Option<&str>,
    worktree_base_ref: Option<&str>,
    reuse_existing_worktree: bool,
    agent_name: Option<&str>,
) -> Result<PathBuf> {
    if let Some(isolation) = isolation {
        match isolation {
            "worktree" => {
                if cwd_override.is_some() {
                    return Err(anyhow!("cannot combine `cwd` with `isolation=worktree`"));
                }
                let name = worktree_name
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| {
                        hellox_tools_agent::worktree::default_worktree_name(agent_name)
                    });
                let worktree = hellox_tools_agent::worktree::enter_worktree(
                    &context.working_directory,
                    &name,
                    worktree_base_ref,
                    reuse_existing_worktree,
                )?;
                return Ok(worktree.path);
            }
            other => {
                return Err(anyhow!("unsupported agent isolation mode `{other}`"));
            }
        }
    }

    match cwd_override {
        Some(raw) => {
            let path = context.resolve_path(raw);
            if !path.is_dir() {
                return Err(anyhow!(
                    "agent working directory does not exist or is not a directory: {}",
                    path.display()
                ));
            }
            Ok(path)
        }
        None => Ok(context.working_directory.clone()),
    }
}

#[cfg(test)]
mod tests;
