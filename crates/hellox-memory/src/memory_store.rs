use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{anyhow, Context, Result};

use crate::{
    memory_archive_path_for_scope, memory_archive_scope_root, memory_path_for_scope,
    memory_scope_root, normalize_path, relative_age_text, MemoryEntry, MemoryScope,
};

pub fn list_memories(root: &Path) -> Result<Vec<MemoryEntry>> {
    let mut entries = Vec::new();
    let session_root = memory_scope_root(root, MemoryScope::Session);
    let project_root = memory_scope_root(root, MemoryScope::Project);
    collect_memory_entries(&session_root, MemoryScope::Session, &mut entries)?;
    collect_memory_entries(&project_root, MemoryScope::Project, &mut entries)?;
    entries.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    Ok(entries)
}

pub fn list_archived_memories(root: &Path) -> Result<Vec<MemoryEntry>> {
    let mut entries = Vec::new();
    let session_root = memory_archive_scope_root(root, MemoryScope::Session);
    let project_root = memory_archive_scope_root(root, MemoryScope::Project);
    collect_memory_entries(&session_root, MemoryScope::Session, &mut entries)?;
    collect_memory_entries(&project_root, MemoryScope::Project, &mut entries)?;
    entries.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    Ok(entries)
}

pub fn format_memory_list(entries: &[MemoryEntry]) -> String {
    if entries.is_empty() {
        return "No memory files found.".to_string();
    }

    let mut lines = Vec::with_capacity(entries.len() + 1);
    lines.push("memory_id\tscope\tage\tupdated_at\tpath".to_string());
    for entry in entries {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}",
            entry.memory_id,
            entry.scope.as_str(),
            relative_age_text(entry.updated_at),
            entry.updated_at,
            entry.path
        ));
    }
    lines.join("\n")
}

pub fn load_memory(root: &Path, memory_id: &str) -> Result<String> {
    let memory_id = memory_id.trim();
    if memory_id.is_empty() {
        return Err(anyhow!("memory id cannot be empty"));
    }

    let scopes = if memory_id.starts_with("project-") {
        [MemoryScope::Project, MemoryScope::Session]
    } else {
        [MemoryScope::Session, MemoryScope::Project]
    };

    for scope in scopes {
        let path = memory_path_for_scope(root, scope, memory_id);
        if path.exists() {
            return fs::read_to_string(&path)
                .with_context(|| format!("failed to read memory file {}", path.display()));
        }

        let archived_path = memory_archive_path_for_scope(root, scope, memory_id);
        if archived_path.exists() {
            return fs::read_to_string(&archived_path).with_context(|| {
                format!(
                    "failed to read archived memory file {}",
                    archived_path.display()
                )
            });
        }
    }

    Err(anyhow!("memory `{memory_id}` was not found"))
}

pub fn load_archived_memory(root: &Path, memory_id: &str) -> Result<String> {
    let memory_id = memory_id.trim();
    if memory_id.is_empty() {
        return Err(anyhow!("memory id cannot be empty"));
    }

    let scopes = if memory_id.starts_with("project-") {
        [MemoryScope::Project, MemoryScope::Session]
    } else {
        [MemoryScope::Session, MemoryScope::Project]
    };

    for scope in scopes {
        let path = memory_archive_path_for_scope(root, scope, memory_id);
        if path.exists() {
            return fs::read_to_string(&path).with_context(|| {
                format!("failed to read archived memory file {}", path.display())
            });
        }
    }

    Err(anyhow!("archived memory `{memory_id}` was not found"))
}

fn collect_memory_entries(
    scope_root: &Path,
    scope: MemoryScope,
    entries: &mut Vec<MemoryEntry>,
) -> Result<()> {
    if !scope_root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&scope_root)
        .with_context(|| format!("failed to list memory dir {}", scope_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }

        let updated_at = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_secs())
            .unwrap_or_default();
        let memory_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        entries.push(MemoryEntry {
            memory_id,
            scope,
            updated_at,
            path: normalize_path(&path.display().to_string()),
        });
    }

    Ok(())
}
