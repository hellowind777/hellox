use std::path::{Path, PathBuf};

use super::{
    render_workflow_run_inspect_panel, render_workflow_run_inspect_panel_with_step,
    render_workflow_run_list,
};
use crate::workflow_runs::{WorkflowRunRecord, WorkflowRunStepRecord, WorkflowRunSummary};
use crate::workflows::WorkflowRunTarget;

fn sample_record() -> WorkflowRunRecord {
    WorkflowRunRecord {
        run_id: "run-123".to_string(),
        status: "completed".to_string(),
        workflow_name: Some("release-review".to_string()),
        workflow_source: Some(".hellox/workflows/release-review.json".to_string()),
        requested_script_path: None,
        started_at: 10,
        finished_at: 20,
        shared_context: Some("ship carefully".to_string()),
        continue_on_error: Some(false),
        summary: WorkflowRunSummary {
            total_steps: 2,
            completed_steps: 1,
            failed_steps: 1,
            running_steps: 0,
            skipped_steps: 0,
        },
        steps: vec![
            WorkflowRunStepRecord {
                name: "review".to_string(),
                status: "completed".to_string(),
                result_text: Some("ok".to_string()),
                error: None,
                reason: None,
            },
            WorkflowRunStepRecord {
                name: "summarize".to_string(),
                status: "failed".to_string(),
                result_text: None,
                error: Some("boom".to_string()),
                reason: Some("previous stage failed".to_string()),
            },
        ],
        error: Some("workflow failed".to_string()),
        result_text: "{\"status\":\"failed\"}".to_string(),
    }
}

#[test]
fn run_list_renders_history_panel_cards() {
    let text = render_workflow_run_list(
        Path::new("D:/repo"),
        &[sample_record()],
        Some(&WorkflowRunTarget::Named("release-review".to_string())),
    );

    assert!(text.contains("Workflow run history panel"));
    assert!(text.contains("== Recorded runs =="));
    assert!(text.contains("run_id"));
    assert!(text.contains("run-123"));
    assert!(text.contains("COMPLETED"));
    assert!(text.contains("== Recent run selector =="));
    assert!(text.contains("[1] run-123 — COMPLETED"));
    assert!(text.contains("primary_step: [2] summarize — FAILED"));
    assert!(text.contains("focus: `hellox workflow show-run run-123 2`"));
    assert!(text.contains("next: `hellox workflow last-run release-review`"));
    assert!(text.contains("hellox workflow last-run release-review"));
}

#[test]
fn run_list_renders_script_path_action_palette_for_custom_runs() {
    let mut record = sample_record();
    record.workflow_name = None;
    record.workflow_source = Some("scripts/custom-release.json".to_string());
    record.requested_script_path = Some("scripts/custom-release.json".to_string());

    let text = render_workflow_run_list(
        Path::new("D:/repo"),
        &[record],
        Some(&WorkflowRunTarget::Path(PathBuf::from(
            "scripts/custom-release.json",
        ))),
    );

    assert!(text.contains("script_path"));
    assert!(text.contains("hellox workflow last-run --script-path scripts/custom-release.json"));
    assert!(text.contains("hellox workflow panel --script-path scripts/custom-release.json"));
    assert!(text.contains("/workflow runs --script-path scripts/custom-release.json"));
}

#[test]
fn inspect_panel_renders_visual_map_and_palettes() {
    let text = render_workflow_run_inspect_panel(Path::new("D:/repo"), &sample_record());

    assert!(text.contains("Workflow run inspect panel: run-123"));
    assert!(text.contains("c1/f1/r0/s0"));
    assert!(text.contains("== Visual execution map =="));
    assert!(text.contains("status"));
    assert!(text.contains("review"));
    assert!(text.contains("summarize"));
    assert!(text.contains("yes"));
    assert!(text.contains("== Step selector =="));
    assert!(text.contains("== Primary step lens =="));
    assert!(text.contains("> [1] summarize — FAILED (step 2)"));
    assert!(text.contains("== Execution details =="));
    assert!(text.contains("reason: previous stage failed"));
    assert!(text.contains("error: boom"));
    assert!(text.contains("== CLI palette =="));
    assert!(text.contains("hellox workflow run release-review --shared-context \"ship carefully\""));
    assert!(text.contains("== REPL palette =="));
    assert!(text.contains("/workflow panel release-review"));
}

#[test]
fn inspect_panel_uses_script_path_palette_for_custom_runs() {
    let mut record = sample_record();
    record.workflow_name = None;
    record.workflow_source = Some("scripts/custom-release.json".to_string());
    record.requested_script_path = Some("scripts/custom-release.json".to_string());

    let text = render_workflow_run_inspect_panel(Path::new("D:/repo"), &record);

    assert!(text.contains("(custom path)"));
    assert!(text.contains(
        "hellox workflow run --script-path scripts/custom-release.json --shared-context \"ship carefully\""
    ));
    assert!(text.contains("hellox workflow runs --script-path scripts/custom-release.json"));
    assert!(text.contains("hellox workflow last-run --script-path scripts/custom-release.json"));
    assert!(text.contains("hellox workflow panel --script-path scripts/custom-release.json"));
    assert!(text.contains("== REPL palette =="));
    assert!(text.contains("/workflow run --script-path scripts/custom-release.json ship carefully"));
    assert!(text.contains("/workflow runs --script-path scripts/custom-release.json"));
    assert!(text.contains("/workflow last-run --script-path scripts/custom-release.json"));
    assert!(text.contains("/workflow panel --script-path scripts/custom-release.json"));
    assert!(text.contains("/workflow validate --script-path scripts/custom-release.json"));
}

#[test]
fn inspect_panel_can_focus_explicit_step() {
    let text = render_workflow_run_inspect_panel_with_step(
        Path::new("D:/repo"),
        &sample_record(),
        Some(1),
    );

    assert!(text.contains("== Focused step lens =="));
    assert!(text.contains("== Step selector =="));
    assert!(text.contains("> [1] review — COMPLETED (step 1)"));
    assert!(!text.contains("> [1] summarize — FAILED (step 2)"));
}
