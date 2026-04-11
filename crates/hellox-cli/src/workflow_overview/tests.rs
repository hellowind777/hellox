use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::{
    list_workflow_focus_selection_items, list_workflow_overview_selection_items,
    render_workflow_overview, WorkflowOverviewFocusSelectionItem, WorkflowOverviewSelectionItem,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-workflow-overview-{suffix}"));
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
fn selector_includes_latest_run_and_custom_run_sections() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "shared_context": "ship carefully",
  "steps": [
    { "name": "review", "prompt": "review release notes" }
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
                "total_steps": 1,
                "completed_steps": 1,
                "failed_steps": 0,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [],
            "error": null,
            "result_text": "ok"
        }),
    );
    write_run(
        &root,
        "run-200",
        json!({
            "run_id": "run-200",
            "status": "failed",
            "workflow_name": null,
            "workflow_source": "scripts/custom-release.json",
            "requested_script_path": "scripts/custom-release.json",
            "started_at": 3,
            "finished_at": 4,
            "shared_context": null,
            "continue_on_error": null,
            "summary": {
                "total_steps": 0,
                "completed_steps": 0,
                "failed_steps": 1,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [],
            "error": "boom",
            "result_text": "boom"
        }),
    );

    let text = render_workflow_overview(&root, None).expect("render selector");
    assert!(text.contains("Workflow overview selector"));
    assert!(text.contains("== Workflows =="));
    assert!(text.contains("[1] release-review"));
    assert!(text.contains("release-review — VALID"));
    assert!(text.contains("dynamic_command: /release-review [shared_context]"));
    assert!(text.contains("latest_run: COMPLETED (`run-100`"));
    assert!(text.contains("== Custom-path runs =="));
    assert!(text.contains("[2] run-200 — FAILED"));
    assert!(text.contains("hellox workflow run --script-path scripts/custom-release.json"));
}

#[test]
fn selection_items_follow_global_selector_order() {
    let root = temp_dir();
    write_workflow(
        &root,
        "alpha.json",
        r#"{ "steps": [{ "prompt": "alpha" }] }"#,
    );
    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "prompt": "release" }] }"#,
    );
    write_run(
        &root,
        "run-200",
        json!({
            "run_id": "run-200",
            "status": "failed",
            "workflow_name": null,
            "workflow_source": "scripts/custom-release.json",
            "requested_script_path": "scripts/custom-release.json",
            "started_at": 3,
            "finished_at": 4,
            "shared_context": null,
            "continue_on_error": null,
            "summary": {
                "total_steps": 0,
                "completed_steps": 0,
                "failed_steps": 1,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [],
            "error": "boom",
            "result_text": "boom"
        }),
    );

    let items = list_workflow_overview_selection_items(&root).expect("list overview items");
    assert_eq!(
        items,
        vec![
            WorkflowOverviewSelectionItem::Workflow(String::from("alpha")),
            WorkflowOverviewSelectionItem::Workflow(String::from("release-review")),
            WorkflowOverviewSelectionItem::Run(String::from("run-200")),
        ]
    );
}

#[test]
fn named_overview_renders_visual_map_and_latest_run_snapshot() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "continue_on_error": true,
  "steps": [
    { "name": "review", "prompt": "review release notes", "backend": "detached_process" }
  ]
}"#,
    );
    write_run(
        &root,
        "run-300",
        json!({
            "run_id": "run-300",
            "status": "completed",
            "workflow_name": "release-review",
            "workflow_source": ".hellox/workflows/release-review.json",
            "requested_script_path": null,
            "started_at": 10,
            "finished_at": 20,
            "shared_context": null,
            "continue_on_error": true,
            "summary": {
                "total_steps": 1,
                "completed_steps": 1,
                "failed_steps": 0,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [
                {
                    "name": "review",
                    "status": "completed",
                    "result_text": "done",
                    "error": null,
                    "reason": null
                }
            ],
            "error": null,
            "result_text": "ok"
        }),
    );

    let text =
        render_workflow_overview(&root, Some("release-review")).expect("render named overview");
    assert!(text.contains("Workflow overview: release-review"));
    assert!(text.contains("== Visual script map =="));
    assert!(text.contains("backend=detached_process"));
    assert!(text.contains("latest_status=COMPLETED"));
    assert!(text.contains("== Step selector =="));
    assert!(text.contains("== Recent runs =="));
    assert!(text.contains("[2] run-300"));
    assert!(text.contains("primary_step: [1] review — COMPLETED"));
    assert!(text.contains("focus: `hellox workflow show-run run-300 1`"));
    assert!(text.contains("next: `hellox workflow last-run release-review`"));
    assert!(text.contains("== Latest run snapshot =="));
    assert!(text.contains("run_id: run-300"));
    assert!(text.contains("== CLI palette =="));
    assert!(text.contains("hellox workflow run release-review --shared-context \"<text>\""));
}

#[test]
fn focused_selection_items_append_recent_runs_after_steps() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{ "steps": [{ "name": "review", "prompt": "review release" }] }"#,
    );
    write_run(
        &root,
        "run-300",
        json!({
            "run_id": "run-300",
            "status": "completed",
            "workflow_name": "release-review",
            "workflow_source": ".hellox/workflows/release-review.json",
            "requested_script_path": null,
            "started_at": 10,
            "finished_at": 20,
            "shared_context": null,
            "continue_on_error": false,
            "summary": {
                "total_steps": 1,
                "completed_steps": 1,
                "failed_steps": 0,
                "running_steps": 0,
                "skipped_steps": 0
            },
            "steps": [],
            "error": null,
            "result_text": "ok"
        }),
    );

    let items =
        list_workflow_focus_selection_items(&root, "release-review").expect("list focus items");
    assert_eq!(
        items,
        vec![
            WorkflowOverviewFocusSelectionItem::Step(1),
            WorkflowOverviewFocusSelectionItem::Run(String::from("run-300")),
        ]
    );
}
