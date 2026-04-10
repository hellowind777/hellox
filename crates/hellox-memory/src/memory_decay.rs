use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::{
    list_archived_memories, memory_archive_path_for_scope, relative_age_text, unix_timestamp,
    MemoryScope, MemoryScopeSelector,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDecayOptions {
    pub scope: MemoryScopeSelector,
    pub older_than_days: u64,
    pub keep_latest: usize,
    pub max_summary_lines: usize,
    pub max_summary_chars: usize,
    pub apply: bool,
}

impl Default for MemoryDecayOptions {
    fn default() -> Self {
        Self {
            scope: MemoryScopeSelector::All,
            older_than_days: 180,
            keep_latest: 20,
            max_summary_lines: 24,
            max_summary_chars: 1600,
            apply: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDecayCandidate {
    pub memory_id: String,
    pub scope: MemoryScope,
    pub age: String,
    pub updated_at: u64,
    pub reason: String,
    pub path: String,
    pub summary_before: String,
    pub summary_after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDecayReport {
    pub scope: MemoryScopeSelector,
    pub older_than_days: u64,
    pub keep_latest: usize,
    pub max_summary_lines: usize,
    pub max_summary_chars: usize,
    pub apply: bool,
    pub scanned: usize,
    pub kept: usize,
    pub candidates: Vec<MemoryDecayCandidate>,
}

pub fn decay_archived_memories(
    root: &Path,
    options: &MemoryDecayOptions,
) -> Result<MemoryDecayReport> {
    if options.keep_latest == 0 {
        return Err(anyhow!("memory decay keep_latest must be at least 1"));
    }
    if options.max_summary_lines == 0 {
        return Err(anyhow!("memory decay max_summary_lines must be at least 1"));
    }
    if options.max_summary_chars < 16 {
        return Err(anyhow!(
            "memory decay max_summary_chars must be at least 16"
        ));
    }

    let entries = list_archived_memories(root)?;
    let mut report = MemoryDecayReport {
        scope: options.scope,
        older_than_days: options.older_than_days,
        keep_latest: options.keep_latest,
        max_summary_lines: options.max_summary_lines,
        max_summary_chars: options.max_summary_chars,
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

            let path = memory_archive_path_for_scope(root, scope, &entry.memory_id);
            let original = fs::read_to_string(&path).with_context(|| {
                format!("failed to read archived memory file {}", path.display())
            })?;

            let (decayed, changed, stats) = decay_markdown_summary(
                &original,
                options.max_summary_lines,
                options.max_summary_chars,
            );
            if !changed {
                report.kept += 1;
                continue;
            }

            if options.apply {
                // Preserve file mtime so decay doesn't reshuffle retention ordering.
                let original_modified = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .ok();

                fs::write(&path, decayed.as_bytes()).with_context(|| {
                    format!(
                        "failed to write decayed archived memory file {}",
                        path.display()
                    )
                })?;

                if let Some(original_modified) = original_modified {
                    let file = fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&path)
                        .with_context(|| {
                            format!("failed to open archived memory file {}", path.display())
                        })?;
                    let times = fs::FileTimes::new().set_modified(original_modified);
                    file.set_times(times).with_context(|| {
                        format!("failed to restore file times for {}", path.display())
                    })?;
                }
            }

            report.candidates.push(MemoryDecayCandidate {
                memory_id: entry.memory_id,
                scope,
                age: relative_age_text(entry.updated_at),
                updated_at: entry.updated_at,
                reason: format!(
                    "older than {}d and outside latest {}",
                    options.older_than_days, options.keep_latest
                ),
                path: entry.path,
                summary_before: format!("{}/{}c", stats.before_lines, stats.before_chars),
                summary_after: format!("{}/{}c", stats.after_lines, stats.after_chars),
            });
        }
    }

    Ok(report)
}

pub fn format_memory_decay_report(report: &MemoryDecayReport) -> String {
    if report.scanned == 0 {
        return format!(
            "No archived memory files found for scope `{}`.",
            report.scope.as_str()
        );
    }

    let action = if report.apply {
        "Decayed"
    } else {
        "Would decay"
    };
    let action_label = if report.apply {
        "decayed"
    } else {
        "would-decay"
    };
    let mut lines = vec![
        format!(
            "{action} {} stale archived memory file(s) for scope `{}` (older than {}d, keep latest {}, max summary {} lines / {} chars).",
            report.candidates.len(),
            report.scope.as_str(),
            report.older_than_days,
            report.keep_latest,
            report.max_summary_lines,
            report.max_summary_chars
        ),
        format!("scanned\t{}\nkept\t{}", report.scanned, report.kept),
    ];

    if report.candidates.is_empty() {
        return lines.join("\n");
    }

    lines.push(
        "memory_id\tscope\tage\tupdated_at\taction\treason\tsummary_before\tsummary_after\tpath"
            .to_string(),
    );
    for candidate in &report.candidates {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            candidate.memory_id,
            candidate.scope.as_str(),
            candidate.age,
            candidate.updated_at,
            action_label,
            candidate.reason,
            candidate.summary_before,
            candidate.summary_after,
            candidate.path
        ));
    }
    lines.join("\n")
}

#[derive(Debug, Clone, Copy)]
struct SummaryDecayStats {
    before_lines: usize,
    before_chars: usize,
    after_lines: usize,
    after_chars: usize,
}

fn decay_markdown_summary(
    markdown: &str,
    max_lines: usize,
    max_chars: usize,
) -> (String, bool, SummaryDecayStats) {
    let lines = markdown.lines().collect::<Vec<_>>();
    let Some(summary_heading_index) = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("## Summary"))
    else {
        return (
            markdown.to_string(),
            false,
            SummaryDecayStats {
                before_lines: 0,
                before_chars: 0,
                after_lines: 0,
                after_chars: 0,
            },
        );
    };

    let mut content_start = summary_heading_index + 1;
    while content_start < lines.len() && lines[content_start].trim().is_empty() {
        content_start += 1;
    }

    let mut content_end = lines.len();
    for (index, line) in lines.iter().enumerate().skip(content_start) {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") && !trimmed.eq_ignore_ascii_case("## Summary") {
            content_end = index;
            break;
        }
    }

    let summary_lines = &lines[content_start..content_end];
    let summary_text = summary_lines.join("\n");
    let before_lines = summary_lines.len();
    let before_chars = summary_text.chars().count();

    let mut decayed_text = summary_lines
        .iter()
        .take(max_lines)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    if before_lines > max_lines {
        if !decayed_text.is_empty() {
            decayed_text.push('\n');
        }
        decayed_text.push_str("...");
    }
    decayed_text = truncate_chars(&decayed_text, max_chars);

    let after_lines = decayed_text.lines().count();
    let after_chars = decayed_text.chars().count();

    let changed = decayed_text != summary_text;
    if !changed {
        return (
            markdown.to_string(),
            false,
            SummaryDecayStats {
                before_lines,
                before_chars,
                after_lines,
                after_chars,
            },
        );
    }

    let mut rebuilt = Vec::new();
    rebuilt.extend_from_slice(&lines[..summary_heading_index + 1]);
    rebuilt.push("");
    rebuilt.extend(decayed_text.lines());
    rebuilt.extend_from_slice(&lines[content_end..]);

    (
        rebuilt.join("\n"),
        true,
        SummaryDecayStats {
            before_lines,
            before_chars,
            after_lines,
            after_chars,
        },
    )
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return value.chars().take(max_chars).collect::<String>();
    }
    let truncated = value.chars().take(max_chars - 3).collect::<String>();
    format!("{truncated}...")
}

fn age_days(updated_at: u64) -> u64 {
    unix_timestamp().saturating_sub(updated_at) / 86_400
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{decay_archived_memories, format_memory_decay_report, MemoryDecayOptions};
    use crate::{
        list_archived_memories, memory_archive_path_for_scope, MemoryScope, MemoryScopeSelector,
    };

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-memory-decay-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_archived(
        root: &PathBuf,
        scope: MemoryScope,
        memory_id: &str,
        summary_lines: usize,
    ) -> PathBuf {
        let path = memory_archive_path_for_scope(root, scope, memory_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create archive dir");
        }

        let summary = (0..summary_lines)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let markdown = format!(
            "# hellox memory\n\n- scope: {}\n- memory_id: {}\n- updated_at: 1\n\n## Summary\n\n{}\n\n## Key Points\n\n- ok\n",
            scope.as_str(),
            memory_id,
            summary
        );
        fs::write(&path, markdown).expect("write archived memory");
        path
    }

    #[test]
    fn decay_report_marks_candidates_outside_window_and_truncates_summary() {
        let root = temp_root();
        write_archived(&root, MemoryScope::Session, "session-a-keep", 1);
        write_archived(&root, MemoryScope::Session, "session-z-decay", 40);

        let report = decay_archived_memories(
            &root,
            &MemoryDecayOptions {
                scope: MemoryScopeSelector::Session,
                older_than_days: 0,
                keep_latest: 1,
                max_summary_lines: 2,
                max_summary_chars: 80,
                apply: false,
            },
        )
        .expect("decay report");

        assert_eq!(report.scanned, 2);
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].memory_id, "session-z-decay");
        assert!(format_memory_decay_report(&report)
            .contains("Would decay 1 stale archived memory file(s)"));
    }

    #[test]
    fn decay_apply_preserves_file_mtime() {
        let root = temp_root();
        let keep_path = write_archived(&root, MemoryScope::Session, "session-a-keep", 1);
        let decay_path = write_archived(&root, MemoryScope::Session, "session-z-decay", 40);

        // Ensure ordering is deterministic: keep is newer than candidate.
        let keep_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&keep_path)
            .expect("open keep");
        let decay_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&decay_path)
            .expect("open decay");
        keep_file
            .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(200)))
            .expect("set keep mtime");
        decay_file
            .set_times(fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(100)))
            .expect("set decay mtime");

        let before = fs::metadata(&decay_path)
            .expect("meta")
            .modified()
            .expect("modified");

        let report = decay_archived_memories(
            &root,
            &MemoryDecayOptions {
                scope: MemoryScopeSelector::Session,
                older_than_days: 0,
                keep_latest: 1,
                max_summary_lines: 2,
                max_summary_chars: 80,
                apply: true,
            },
        )
        .expect("apply decay");

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(list_archived_memories(&root).expect("list").len(), 2);
        let after = fs::metadata(&decay_path)
            .expect("meta")
            .modified()
            .expect("modified");
        assert_eq!(
            before.duration_since(UNIX_EPOCH).expect("dur").as_secs(),
            after.duration_since(UNIX_EPOCH).expect("dur").as_secs()
        );

        let markdown = fs::read_to_string(&decay_path).expect("read decayed");
        assert!(markdown.contains("## Summary"));
        assert!(markdown.contains("..."));
    }
}
