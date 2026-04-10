use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{cluster_memories, format_memory_cluster_report, MemoryClusterOptions};

fn temp_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-memory-cluster-{suffix}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}

#[test]
fn clusters_group_similar_entries_by_token_overlap() {
    let root = temp_root();
    let session_root = root.join("sessions");
    fs::create_dir_all(&session_root).expect("create session root");

    fs::write(
        session_root.join("session-a.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel pending work remains\n\n## Pending Work\n\n- workflow panel needs wiring\n",
    )
    .expect("write session-a");
    fs::write(
        session_root.join("session-b.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel still missing\n\n## Pending Work\n\n- need to build workflow panel\n",
    )
    .expect("write session-b");
    fs::write(
        session_root.join("session-c.md"),
        "# hellox memory\n\n## Summary\n\nmcp oauth flow stabilized\n\n## Key Points\n\n- oauth pkce works\n",
    )
    .expect("write session-c");

    let report = cluster_memories(
        &root,
        &MemoryClusterOptions {
            archived: false,
            limit: 50,
            min_jaccard: 0.15,
            max_tokens: 48,
            semantic: false,
        },
    )
    .expect("cluster");

    assert_eq!(report.entries, 3);
    assert!(report.clusters.len() <= 3);

    let rendered = format_memory_cluster_report(&report);
    assert!(rendered.contains("cluster_id"));
    assert!(rendered.contains("session-a"));
    assert!(rendered.contains("session-b"));
    assert!(rendered.contains("session-c"));
}

#[test]
fn semantic_clusters_group_similar_entries_by_tfidf_cosine() {
    let root = temp_root();
    let session_root = root.join("sessions");
    fs::create_dir_all(&session_root).expect("create session root");

    fs::write(
        session_root.join("session-a.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel pending work remains\n\n## Pending Work\n\n- workflow panel needs wiring\n",
    )
    .expect("write session-a");
    fs::write(
        session_root.join("session-b.md"),
        "# hellox memory\n\n## Summary\n\nworkflow panel still missing\n\n## Pending Work\n\n- need to build workflow panel\n",
    )
    .expect("write session-b");
    fs::write(
        session_root.join("session-c.md"),
        "# hellox memory\n\n## Summary\n\nmcp oauth flow stabilized\n\n## Key Points\n\n- oauth pkce works\n",
    )
    .expect("write session-c");

    let report = cluster_memories(
        &root,
        &MemoryClusterOptions {
            archived: false,
            limit: 50,
            min_jaccard: 0.18,
            max_tokens: 48,
            semantic: true,
        },
    )
    .expect("cluster");

    assert_eq!(report.entries, 3);

    let rendered = format_memory_cluster_report(&report);
    assert!(rendered.contains("mode: tfidf_cosine"));
    assert!(rendered.contains("session-a"));
    assert!(rendered.contains("session-b"));
    assert!(rendered.contains("session-c"));
}
