use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::workflow_authoring::{
    add_workflow_step, duplicate_workflow_step, move_workflow_step, remove_workflow_step,
    resolve_existing_workflow_path, set_workflow_continue_on_error, set_workflow_shared_context,
    update_workflow_step, WorkflowStepDraft, WorkflowStepPatch,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-workflow-authoring-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn write_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(".hellox").join("workflows").join(relative);
    fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
    fs::write(path, raw).expect("write workflow");
}

#[test]
fn authoring_roundtrip_edits_workflow_script() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" }
  ]
}"#,
    );

    let path = resolve_existing_workflow_path(&root, "release-review").expect("resolve workflow");
    let added = add_workflow_step(
        &root,
        &path,
        WorkflowStepDraft {
            name: Some("summarize".to_string()),
            prompt: "summarize findings".to_string(),
            when: Some(r#"{"previous_status":"completed"}"#.to_string()),
            model: Some("mock-model".to_string()),
            backend: None,
            step_cwd: Some("docs".to_string()),
            run_in_background: true,
        },
        Some(2),
    )
    .expect("add step");
    assert_eq!(added.step_number, 2);
    assert_eq!(added.detail.summary.step_count, 2);

    let updated = update_workflow_step(
        &root,
        &path,
        2,
        WorkflowStepPatch {
            name: Some(None),
            prompt: Some("ship release".to_string()),
            when: Some(None),
            model: Some(None),
            backend: Some(Some("detached_process".to_string())),
            step_cwd: Some(None),
            run_in_background: Some(false),
        },
    )
    .expect("update step");
    assert_eq!(updated.steps[1].name, None);
    assert_eq!(
        updated.steps[1].backend.as_deref(),
        Some("detached_process")
    );
    assert!(!updated.steps[1].run_in_background);

    let duplicated =
        duplicate_workflow_step(&root, &path, 1, Some(2), None).expect("duplicate workflow step");
    assert_eq!(duplicated.step_number, 2);
    assert_eq!(
        duplicated.duplicated_step_name.as_deref(),
        Some("review copy")
    );

    let moved = move_workflow_step(&root, &path, 2, 1).expect("move workflow step");
    assert_eq!(moved.step_number, 1);
    assert_eq!(moved.moved_step_name.as_deref(), Some("review copy"));

    let with_context =
        set_workflow_shared_context(&root, &path, Some("ship carefully".to_string()))
            .expect("set shared context");
    assert_eq!(
        with_context.summary.shared_context.as_deref(),
        Some("ship carefully")
    );

    let enabled =
        set_workflow_continue_on_error(&root, &path, true).expect("enable continue_on_error");
    assert!(enabled.summary.continue_on_error);

    let removed = remove_workflow_step(&root, &path, 2).expect("remove step");
    assert_eq!(removed.detail.summary.step_count, 2);
}
