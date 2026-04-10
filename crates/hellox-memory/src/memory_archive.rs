use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::{
    list_memories, memory_archive_path_for_scope, memory_path_for_scope, normalize_path,
    relative_age_text, unix_timestamp, MemoryScope, MemoryScopeSelector,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryArchiveOptions {
    pub scope: MemoryScopeSelector,
    pub older_than_days: u64,
    pub keep_latest: usize,
    pub apply: bool,
}

impl Default for MemoryArchiveOptions {
    fn default() -> Self {
        Self {
            scope: MemoryScopeSelector::All,
            older_than_days: 30,
            keep_latest: 3,
            apply: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryArchiveCandidate {
    pub memory_id: String,
    pub scope: MemoryScope,
    pub age: String,
    pub updated_at: u64,
    pub reason: String,
    pub source_path: String,
    pub archive_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryArchiveReport {
    pub scope: MemoryScopeSelector,
    pub older_than_days: u64,
    pub keep_latest: usize,
    pub apply: bool,
    pub scanned: usize,
    pub kept: usize,
    pub candidates: Vec<MemoryArchiveCandidate>,
}

pub fn archive_memories(
    root: &Path,
    options: &MemoryArchiveOptions,
) -> Result<MemoryArchiveReport> {
    if options.keep_latest == 0 {
        return Err(anyhow!("memory archive keep_latest must be at least 1"));
    }

    let entries = list_memories(root)?;
    let mut report = MemoryArchiveReport {
        scope: options.scope,
        older_than_days: options.older_than_days,
        keep_latest: options.keep_latest,
        apply: options.apply,
        scanned: 0,
        kept: 0,
        candidates: Vec::new(),
    };

    for scope in [MemoryScope::Session, MemoryScope::Project] {
        if !options.scope.matches(scope) {
            continue;
        }

        let scoped_entries = entries
            .iter()
            .filter(|entry| entry.scope == scope)
            .cloned()
            .collect::<Vec<_>>();

        for (index, entry) in scoped_entries.into_iter().enumerate() {
            report.scanned += 1;
            if index < options.keep_latest || age_days(entry.updated_at) < options.older_than_days {
                report.kept += 1;
                continue;
            }

            let source_path = memory_path_for_scope(root, scope, &entry.memory_id);
            let archive_path = memory_archive_path_for_scope(root, scope, &entry.memory_id);
            if options.apply {
                let Some(parent) = archive_path.parent() else {
                    return Err(anyhow!(
                        "memory archive destination is missing a parent directory: {}",
                        archive_path.display()
                    ));
                };
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create memory archive dir {}", parent.display())
                })?;
                if archive_path.exists() {
                    return Err(anyhow!(
                        "memory archive destination already exists: {}",
                        archive_path.display()
                    ));
                }

                fs::rename(&source_path, &archive_path).with_context(|| {
                    format!(
                        "failed to archive memory file {} -> {}",
                        source_path.display(),
                        archive_path.display()
                    )
                })?;
            }

            report.candidates.push(MemoryArchiveCandidate {
                memory_id: entry.memory_id,
                scope,
                age: relative_age_text(entry.updated_at),
                updated_at: entry.updated_at,
                reason: format!(
                    "older than {}d and outside latest {}",
                    options.older_than_days, options.keep_latest
                ),
                source_path: entry.path,
                archive_path: normalize_path(&archive_path.display().to_string()),
            });
        }
    }

    Ok(report)
}

pub fn format_memory_archive_report(report: &MemoryArchiveReport) -> String {
    if report.scanned == 0 {
        return format!(
            "No memory files found for scope `{}`.",
            report.scope.as_str()
        );
    }

    let action = if report.apply {
        "Archived"
    } else {
        "Would archive"
    };
    let action_label = if report.apply {
        "archived"
    } else {
        "would-archive"
    };
    let mut lines = vec![
        format!(
            "{action} {} stale memory file(s) for scope `{}` (older than {}d, keep latest {}).",
            report.candidates.len(),
            report.scope.as_str(),
            report.older_than_days,
            report.keep_latest
        ),
        format!("scanned\t{}\nkept\t{}", report.scanned, report.kept),
    ];

    if report.candidates.is_empty() {
        return lines.join("\n");
    }

    lines.push(
        "memory_id\tscope\tage\tupdated_at\taction\treason\tsource_path\tarchive_path".to_string(),
    );
    for candidate in &report.candidates {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            candidate.memory_id,
            candidate.scope.as_str(),
            candidate.age,
            candidate.updated_at,
            action_label,
            candidate.reason,
            candidate.source_path,
            candidate.archive_path
        ));
    }
    lines.join("\n")
}

fn age_days(updated_at: u64) -> u64 {
    unix_timestamp().saturating_sub(updated_at) / 86_400
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        archive_memories, format_memory_archive_report, MemoryArchiveOptions, MemoryArchiveReport,
        MemoryScopeSelector,
    };
    use crate::{list_memories, load_memory};

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-memory-archive-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn apply_archive(root: &PathBuf, options: &MemoryArchiveOptions) -> MemoryArchiveReport {
        archive_memories(root, options).expect("archive memories")
    }

    #[test]
    fn archive_report_marks_candidates_outside_retention_window() {
        let root = temp_root();
        let session_root = root.join("sessions");
        let project_root = root.join("projects");
        fs::create_dir_all(&session_root).expect("create session root");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(session_root.join("session-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(session_root.join("session-z-archive.md"), "# hellox memory")
            .expect("write archive");
        fs::write(project_root.join("project-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(project_root.join("project-z-archive.md"), "# hellox memory")
            .expect("write archive");

        let report = apply_archive(
            &root,
            &MemoryArchiveOptions {
                older_than_days: 0,
                keep_latest: 1,
                ..MemoryArchiveOptions::default()
            },
        );

        assert_eq!(report.scanned, 4);
        assert_eq!(report.kept, 2);
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].memory_id, "session-z-archive");
        assert_eq!(report.candidates[1].memory_id, "project-z-archive");

        let rendered = format_memory_archive_report(&report);
        assert!(rendered.contains("Would archive 2 stale memory file(s)"));
        assert!(rendered.contains("would-archive"));
    }

    #[test]
    fn archive_apply_moves_only_matching_scope() {
        let root = temp_root();
        let session_root = root.join("sessions");
        let project_root = root.join("projects");
        fs::create_dir_all(&session_root).expect("create session root");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(session_root.join("session-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(session_root.join("session-z-archive.md"), "# hellox memory")
            .expect("write archive");
        fs::write(project_root.join("project-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(project_root.join("project-z-keep.md"), "# hellox memory").expect("write keep");

        let report = apply_archive(
            &root,
            &MemoryArchiveOptions {
                scope: MemoryScopeSelector::Session,
                older_than_days: 0,
                keep_latest: 1,
                apply: true,
            },
        );

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].memory_id, "session-z-archive");
        assert_eq!(list_memories(&root).expect("list memories").len(), 3);
        assert!(load_memory(&root, "session-z-archive").is_ok());
        assert!(load_memory(&root, "project-z-keep").is_ok());
    }
}
