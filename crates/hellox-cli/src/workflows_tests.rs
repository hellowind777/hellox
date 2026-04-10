use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::workflows::{
    initialize_workflow, list_invocable_workflows, list_workflows, load_named_workflow_detail,
    render_workflow_list, render_workflow_validation, validate_named_workflow, validate_workflows,
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = env::temp_dir().join(format!("hellox-cli-workflows-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn write_workflow(root: &Path, relative: &str, raw: &str) {
    let path = root.join(".hellox").join("workflows").join(relative);
    fs::create_dir_all(path.parent().expect("workflow dir")).expect("create workflow dir");
    fs::write(path, raw).expect("write workflow");
}

#[test]
fn discovers_and_renders_workflows() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "shared_context": "ship carefully",
  "steps": [
    { "name": "review", "prompt": "review release notes" },
    { "name": "summarize", "prompt": "summarize findings", "backend": "detached_process" }
  ]
}"#,
    );
    write_workflow(
        &root,
        "nested/deploy.json",
        r#"{
  "continue_on_error": true,
  "steps": [
    { "prompt": "deploy staging", "run_in_background": true }
  ]
}"#,
    );
    write_workflow(&root, "broken.json", "{ not-json");

    let workflows = list_workflows(&root).expect("discover workflows");
    assert_eq!(workflows.len(), 3);
    assert_eq!(workflows[0].name, "broken");
    assert!(workflows[0].validation_error.is_some());
    assert_eq!(workflows[1].name, "nested/deploy");
    assert_eq!(workflows[2].name, "release-review");

    let invocable = list_invocable_workflows(&root).expect("discover invocable workflows");
    assert_eq!(invocable.len(), 1);
    assert_eq!(invocable[0].name, "release-review");

    let rendered = render_workflow_list(&root, &workflows);
    assert!(rendered.contains("release-review"));
    assert!(rendered.contains("nested/deploy"));
    assert!(rendered.contains("broken"));
    assert!(rendered.contains("invalid"));
    assert!(rendered.contains("ship carefully"));
}

#[test]
fn loads_named_workflow_detail() {
    let root = temp_dir();
    write_workflow(
        &root,
        "release-review.json",
        r#"{
  "steps": [
    {
      "name": "review",
      "prompt": "review release notes",
      "when": { "previous_status": "completed" },
      "model": "mock-model",
      "cwd": "docs",
      "run_in_background": true
    }
  ]
}"#,
    );

    let detail =
        load_named_workflow_detail(&root, "release-review").expect("load release-review detail");
    assert_eq!(detail.summary.name, "release-review");
    assert_eq!(detail.summary.step_count, 1);
    assert!(detail.summary.dynamic_command);
    assert_eq!(detail.steps[0].name.as_deref(), Some("review"));
    assert!(detail.steps[0].when);
    assert_eq!(detail.steps[0].model.as_deref(), Some("mock-model"));
    assert_eq!(detail.steps[0].cwd.as_deref(), Some("docs"));
    assert!(detail.steps[0].run_in_background);
}

#[test]
fn validates_workflows_and_renders_diagnostics() {
    let root = temp_dir();
    write_workflow(
        &root,
        "ops/release.json",
        r#"{
  "steps": [
    { "name": "review", "prompt": "review release notes" },
    { "name": "review", "prompt": "duplicate review name" },
    { "prompt": "unnamed step" }
  ]
}"#,
    );
    write_workflow(&root, "broken.json", "{ not-json");

    let all = validate_workflows(&root).expect("validate workflows");
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|item| !item.valid));
    assert!(all.iter().any(|item| item
        .messages
        .iter()
        .any(|message| message.contains("duplicate step names"))));

    let nested = validate_named_workflow(&root, "ops/release").expect("validate named workflow");
    assert!(nested.valid);
    assert!(!nested.dynamic_command);
    assert!(nested
        .messages
        .iter()
        .any(|message| message.contains("dynamic `/name` invocation is unavailable")));

    let rendered = render_workflow_validation(&all, &root);
    assert!(rendered.contains("Workflow validation"));
    assert!(rendered.contains("broken"));
    assert!(rendered.contains("duplicate step names"));
}

#[test]
fn initializes_workflow_template() {
    let root = temp_dir();
    let path = initialize_workflow(
        &root,
        "release-review",
        Some("ship carefully".to_string()),
        true,
        false,
    )
    .expect("initialize workflow");

    assert!(path.ends_with("release-review.json"));
    let raw = fs::read_to_string(&path).expect("read workflow template");
    assert!(raw.contains("\"shared_context\": \"ship carefully\""));
    assert!(raw.contains("\"continue_on_error\": true"));
    assert!(raw.contains("\"name\": \"review\""));
    assert!(raw.contains("{{workflow.previous_result}}"));
}
