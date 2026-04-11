use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::render_workflow_panel;

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-workflow-panel-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn write_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(".hellox").join("workflows").join(relative);
    fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
    fs::write(path, raw).expect("write workflow");
}

fn write_run(root: &Path, run_id: &str, value: serde_json::Value) {
    let path = root
        .join(".hellox")
        .join("workflow-runs")
        .join(format!("{run_id}.json"));
    fs::create_dir_all(path.parent().expect("runs dir")).expect("create run dir");
    fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("serialize run"),
    )
    .expect("write run");
}

#[test]
fn focused_panel_renders_action_palette_and_step_status() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "shared_context": "ship carefully",
  "steps": [
    { "name": "review", "prompt": "review release notes" },
    { "name": "summarize", "prompt": "summarize findings", "backend": "detached_process", "run_in_background": true }
  ]
}"#,
    );
    write_run(
        &root,
        "run-100",
        json!({
            "run_id": "run-100",
            "status": "completed",
            "workflow_name": "release-review",
            "workflow_source": ".hellox/workflows/release-review.json",
            "requested_script_path": null,
            "started_at": 1,
            "finished_at": 2,
            "shared_context": "ship carefully",
            "continue_on_error": false,
            "summary": {
                "total_steps": 2,
                "completed_steps": 2,
                "failed_steps": 0,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [
                { "name": "review", "status": "completed", "result_text": "ok", "error": null, "reason": null },
                { "name": "summarize", "status": "completed", "result_text": "ok", "error": null, "reason": null }
            ],
            "error": null,
            "result_text": "ok"
        }),
    );

    let text = render_workflow_panel(&root, Some("release-review"), Some(2))
        .expect("render workflow panel");

    assert!(text.contains("Workflow authoring panel: release-review"));
    assert!(text.contains("== Visual step map =="));
    assert!(text.contains("> | 2 | summarize"));
    assert!(text.contains("== Step selector =="));
    assert!(text.contains("focus: `/workflow panel release-review 2`"));
    assert!(text.contains("== Recent runs =="));
    assert!(text.contains("[3] run-100"));
    assert!(text.contains("primary_step: [1] review — COMPLETED"));
    assert!(text.contains("focus: `hellox workflow show-run run-100 1`"));
    assert!(text.contains("next: `hellox workflow last-run release-review`"));
    assert!(text.contains("== Focused step lens =="));
    assert!(text.contains("> [1] summarize"));
    assert!(text.contains("latest_status: completed"));
    assert!(text.contains("== Action palette =="));
    assert!(text.contains("hellox workflow update-step --workflow release-review 2"));
    assert!(text.contains("== REPL palette =="));
    assert!(text.contains("name <text>"));
    assert!(text.contains("background` / `foreground"));
    assert!(text.contains("`first` / `prev` / `next` / `last`"));
    assert!(text.contains("/workflow panel release-review 2"));
}

#[test]
fn selector_panel_lists_open_commands() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "prompt": "review release" }] }"#,
    );

    let text = render_workflow_panel(&root, None, None).expect("render selector panel");

    assert!(text.contains("Workflow authoring panel selector"));
    assert!(text.contains("== Workflows =="));
    assert!(text.contains("[1] release-review"));
    assert!(text.contains("release-review — VALID"));
    assert!(text.contains("open: `hellox workflow panel release-review`"));
    assert!(text.contains("== Action palette =="));
}
