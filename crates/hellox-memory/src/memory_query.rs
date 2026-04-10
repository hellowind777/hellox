use anyhow::{anyhow, Result};

use crate::{
    list_archived_memories, list_memories, load_archived_memory, load_memory, MemoryEntry,
    MemoryScope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySearchHit {
    pub memory_id: String,
    pub scope: MemoryScope,
    pub updated_at: u64,
    pub age: String,
    pub score: usize,
    pub location: String,
    pub preview: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemorySection {
    Other,
    Summary,
    KeyPoints,
    PendingWork,
    Risks,
    RecentArtifacts,
}

pub fn search_memories_ranked(
    root: &std::path::Path,
    query: &str,
    limit: usize,
) -> Result<Vec<MemorySearchHit>> {
    search_entries_ranked(
        list_memories(root)?,
        |memory_id| load_memory(root, memory_id),
        query,
        limit,
    )
}

pub fn search_archived_memories_ranked(
    root: &std::path::Path,
    query: &str,
    limit: usize,
) -> Result<Vec<MemorySearchHit>> {
    search_entries_ranked(
        list_archived_memories(root)?,
        |memory_id| load_archived_memory(root, memory_id),
        query,
        limit,
    )
}

fn search_entries_ranked<F>(
    entries: Vec<MemoryEntry>,
    mut load: F,
    query: &str,
    limit: usize,
) -> Result<Vec<MemorySearchHit>>
where
    F: FnMut(&str) -> Result<String>,
{
    let query = query.trim();
    if query.is_empty() {
        return Err(anyhow!("memory search query cannot be empty"));
    }
    if limit == 0 {
        return Err(anyhow!("memory search limit must be at least 1"));
    }

    let query_lower = query.to_ascii_lowercase();
    let tokens = query_lower
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let mut hits = Vec::new();
    for entry in entries {
        let memory = load(&entry.memory_id)?;
        let mut best_hit: Option<MemorySearchHit> = None;
        let mut section = MemorySection::Other;

        for (index, line) in memory.lines().enumerate() {
            if let Some(next_section) = memory_section_for_heading(line.trim()) {
                section = next_section;
                continue;
            }

            let score = score_memory_line(
                &query_lower,
                &tokens,
                line,
                section,
                entry.scope,
                entry.updated_at,
            );
            if score == 0 {
                continue;
            }

            let candidate = MemorySearchHit {
                memory_id: entry.memory_id.clone(),
                scope: entry.scope,
                updated_at: entry.updated_at,
                age: relative_age_text(entry.updated_at),
                score,
                location: format!("line {}", index + 1),
                preview: collapse_preview(line),
                path: entry.path.clone(),
            };

            match &best_hit {
                Some(existing)
                    if existing.score > candidate.score
                        || (existing.score == candidate.score
                            && existing.updated_at >= candidate.updated_at) => {}
                _ => best_hit = Some(candidate),
            }
        }

        if let Some(hit) = best_hit {
            hits.push(hit);
        }
    }

    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    hits.truncate(limit);
    Ok(hits)
}

pub fn format_memory_search_results(query: &str, hits: &[MemorySearchHit]) -> String {
    if hits.is_empty() {
        return format!("No memory hits for `{query}`.");
    }

    let mut lines = Vec::with_capacity(hits.len() + 1);
    lines.push("memory_id\tscope\tage\tscore\tlocation\tpreview\tpath".to_string());
    for hit in hits {
        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            hit.memory_id,
            hit.scope.as_str(),
            hit.age,
            hit.score,
            hit.location,
            hit.preview,
            hit.path
        ));
    }
    lines.join("\n")
}

pub fn relative_age_text(updated_at: u64) -> String {
    relative_age_text_at(updated_at, crate::unix_timestamp())
}

fn relative_age_text_at(updated_at: u64, now: u64) -> String {
    let delta = now.saturating_sub(updated_at);
    match delta {
        0..=59 => "fresh <1m".to_string(),
        60..=3_599 => format!("fresh {}m", delta / 60),
        3_600..=86_399 => format!("active {}h", delta / 3_600),
        86_400..=2_591_999 => format!("warm {}d", delta / 86_400),
        _ => format!("stale {}d", delta / 86_400),
    }
}

fn score_memory_line(
    query_lower: &str,
    tokens: &[String],
    line: &str,
    section: MemorySection,
    scope: MemoryScope,
    updated_at: u64,
) -> usize {
    let line_lower = line.to_ascii_lowercase();
    let exact_matches = line_lower.matches(query_lower).count();
    let token_matches = tokens
        .iter()
        .filter(|token| line_lower.contains(token.as_str()))
        .count();

    if exact_matches == 0 && token_matches == 0 {
        return 0;
    }

    let mut score = (exact_matches * 100) + (token_matches * 20);
    if !tokens.is_empty() && token_matches == tokens.len() {
        score += 30;
    }
    score += section_bonus(section);
    if matches!(scope, MemoryScope::Session) {
        score += 3;
    }
    score + recency_bonus(updated_at)
}

fn memory_section_for_heading(line: &str) -> Option<MemorySection> {
    match line {
        "## Summary" => Some(MemorySection::Summary),
        "## Key Points" => Some(MemorySection::KeyPoints),
        "## Pending Work" => Some(MemorySection::PendingWork),
        "## Risks" => Some(MemorySection::Risks),
        "## Recent Artifacts" => Some(MemorySection::RecentArtifacts),
        _ => None,
    }
}

fn section_bonus(section: MemorySection) -> usize {
    match section {
        MemorySection::Other => 0,
        MemorySection::Summary => 12,
        MemorySection::KeyPoints => 16,
        MemorySection::PendingWork => 18,
        MemorySection::Risks => 10,
        MemorySection::RecentArtifacts => 8,
    }
}

fn recency_bonus(updated_at: u64) -> usize {
    let delta = crate::unix_timestamp().saturating_sub(updated_at);
    match delta {
        0..=3_599 => 20,
        3_600..=86_399 => 12,
        86_400..=604_799 => 6,
        604_800..=2_591_999 => 3,
        _ => 1,
    }
}

fn collapse_preview(line: &str) -> String {
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 120 {
        return collapsed;
    }

    let truncated = collapsed.chars().take(117).collect::<String>();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{format_memory_search_results, relative_age_text_at, search_memories_ranked};

    fn temp_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!("hellox-memory-query-{suffix}"));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn relative_age_text_marks_stale_entries() {
        assert_eq!(relative_age_text_at(100, 120), "fresh <1m");
        assert_eq!(relative_age_text_at(100, 4_000), "active 1h");
        assert_eq!(relative_age_text_at(100, 200_000), "warm 2d");
        assert_eq!(relative_age_text_at(100, 4_000_000), "stale 46d");
    }

    #[test]
    fn ranked_memory_search_prefers_summary_and_formats_scores() {
        let root = temp_root();
        let session_root = root.join("sessions");
        let project_root = root.join("projects");
        fs::create_dir_all(&session_root).expect("create session memory root");
        fs::create_dir_all(&project_root).expect("create project memory root");
        fs::write(
            session_root.join("session-a.md"),
            "# hellox memory\n\n## Summary\n\naccepted architecture is the current plan\n",
        )
        .expect("write session memory");
        fs::write(
            project_root.join("project-b.md"),
            "# hellox memory\n\nmetadata line\nanother architecture note\n",
        )
        .expect("write project memory");

        let hits =
            search_memories_ranked(&root, "accepted architecture", 10).expect("search memories");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].memory_id, "session-a");
        assert_eq!(hits[0].scope.as_str(), "session");
        assert!(hits[0].score > 0);
        assert!(hits[0].age.starts_with("fresh"));
        assert_eq!(hits[1].memory_id, "project-b");
        assert!(hits[0].score > hits[1].score);

        let rendered = format_memory_search_results("accepted architecture", &hits);
        assert!(rendered.contains("score"));
        assert!(rendered.contains("session-a"));
        assert!(rendered.contains("project-b"));
        assert!(rendered.contains("accepted architecture is the current plan"));
    }

    #[test]
    fn ranked_memory_search_prefers_pending_work_section_bonus() {
        let root = temp_root();
        let session_root = root.join("sessions");
        let project_root = root.join("projects");
        fs::create_dir_all(&session_root).expect("create session memory root");
        fs::create_dir_all(&project_root).expect("create project memory root");
        fs::write(
            session_root.join("session-structured.md"),
            "# hellox memory\n\n## Summary\n\nworkflow panel is planned\n\n## Pending Work\n\n- workflow panel remains pending for local UI\n",
        )
        .expect("write session memory");
        fs::write(
            project_root.join("project-plain.md"),
            "# hellox memory\n\nworkflow panel note\n",
        )
        .expect("write project memory");

        let hits = search_memories_ranked(&root, "workflow panel", 10).expect("search memories");
        assert_eq!(hits[0].memory_id, "session-structured");
        assert!(hits[0].preview.contains("remains pending"));
    }
}
