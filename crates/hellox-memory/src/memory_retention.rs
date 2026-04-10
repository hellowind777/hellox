use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::{list_memories, memory_path_for_scope, relative_age_text, unix_timestamp, MemoryScope};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MemoryScopeSelector {
    All,
    Session,
    Project,
}

impl MemoryScopeSelector {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Session => "session",
            Self::Project => "project",
        }
    }

    pub(crate) fn matches(&self, scope: MemoryScope) -> bool {
        matches!(self, Self::All)
            || matches!((self, scope), (Self::Session, MemoryScope::Session))
            || matches!((self, scope), (Self::Project, MemoryScope::Project))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryPruneOptions {
    pub scope: MemoryScopeSelector,
    pub older_than_days: u64,
    pub keep_latest: usize,
    pub apply: bool,
}

impl Default for MemoryPruneOptions {
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
pub struct MemoryPruneCandidate {
    pub memory_id: String,
    pub scope: MemoryScope,
    pub age: String,
    pub updated_at: u64,
    pub reason: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryPruneReport {
    pub scope: MemoryScopeSelector,
    pub older_than_days: u64,
    pub keep_latest: usize,
    pub apply: bool,
    pub scanned: usize,
    pub kept: usize,
    pub candidates: Vec<MemoryPruneCandidate>,
}

pub fn prune_memories(root: &Path, options: &MemoryPruneOptions) -> Result<MemoryPruneReport> {
    if options.keep_latest == 0 {
        return Err(anyhow!("memory prune keep_latest must be at least 1"));
    }

    let entries = list_memories(root)?;
    let mut report = MemoryPruneReport {
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

            let path = memory_path_for_scope(root, scope, &entry.memory_id);
            if options.apply {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to prune memory file {}", path.display()))?;
            }

            report.candidates.push(MemoryPruneCandidate {
                memory_id: entry.memory_id,
                scope,
                age: relative_age_text(entry.updated_at),
                updated_at: entry.updated_at,
                reason: format!(
                    "older than {}d and outside latest {}",
                    options.older_than_days, options.keep_latest
                ),
                path: entry.path,
            });
        }
    }

    Ok(report)
}

pub fn format_memory_prune_report(report: &MemoryPruneReport) -> String {
    if report.scanned == 0 {
        return format!(
            "No memory files found for scope `{}`.",
            report.scope.as_str()
        );
    }

    let action = if report.apply {
        "Pruned"
    } else {
        "Would prune"
    };
    let action_label = if report.apply {
        "pruned"
    } else {
        "would-prune"
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

    lines.push("memory_id\tscope\tage\tupdated_at\taction\treason\tpath".to_string());
    for candidate in &report.candidates {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            candidate.memory_id,
            candidate.scope.as_str(),
            candidate.age,
            candidate.updated_at,
            action_label,
            candidate.reason,
            candidate.path
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
        format_memory_prune_report, prune_memories, MemoryPruneOptions, MemoryScopeSelector,
    };
    use crate::{list_memories, load_memory};

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-memory-prune-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn prune_report_marks_candidates_outside_retention_window() {
        let root = temp_root();
        let session_root = root.join("sessions");
        let project_root = root.join("projects");
        fs::create_dir_all(&session_root).expect("create session root");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(session_root.join("session-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(session_root.join("session-z-prune.md"), "# hellox memory").expect("write prune");
        fs::write(project_root.join("project-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(project_root.join("project-z-prune.md"), "# hellox memory").expect("write prune");

        let report = prune_memories(
            &root,
            &MemoryPruneOptions {
                older_than_days: 0,
                keep_latest: 1,
                ..MemoryPruneOptions::default()
            },
        )
        .expect("prune memories");

        assert_eq!(report.scanned, 4);
        assert_eq!(report.kept, 2);
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].memory_id, "session-z-prune");
        assert_eq!(report.candidates[1].memory_id, "project-z-prune");

        let rendered = format_memory_prune_report(&report);
        assert!(rendered.contains("Would prune 2 stale memory file(s)"));
        assert!(rendered.contains("would-prune"));
    }

    #[test]
    fn prune_apply_removes_only_matching_scope() {
        let root = temp_root();
        let session_root = root.join("sessions");
        let project_root = root.join("projects");
        fs::create_dir_all(&session_root).expect("create session root");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(session_root.join("session-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(session_root.join("session-z-prune.md"), "# hellox memory").expect("write prune");
        fs::write(project_root.join("project-a-keep.md"), "# hellox memory").expect("write keep");
        fs::write(project_root.join("project-z-keep.md"), "# hellox memory").expect("write keep");

        let report = prune_memories(
            &root,
            &MemoryPruneOptions {
                scope: MemoryScopeSelector::Session,
                older_than_days: 0,
                keep_latest: 1,
                apply: true,
            },
        )
        .expect("apply prune");

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].memory_id, "session-z-prune");
        assert_eq!(list_memories(&root).expect("list memories").len(), 3);
        assert!(load_memory(&root, "session-z-prune").is_err());
        assert!(load_memory(&root, "project-z-keep").is_ok());
    }
}
