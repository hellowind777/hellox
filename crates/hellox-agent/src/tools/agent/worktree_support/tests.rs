use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::permissions::PermissionPolicy;
use crate::planning::PlanningState;
use crate::tools::ToolExecutionContext;
use hellox_config::PermissionMode;

use super::resolve_child_working_directory;

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("hellox-agent-worktree-{suffix}"));
    fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn git(directory: &PathBuf, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo(root: &PathBuf) {
    git(root, &["init"]);
    git(root, &["config", "user.email", "local@example.com"]);
    git(root, &["config", "user.name", "Local"]);
    fs::write(root.join("README.md"), "hello\n").expect("write readme");
    git(root, &["add", "README.md"]);
    git(root, &["commit", "-m", "init"]);
}

fn context(root: PathBuf) -> ToolExecutionContext {
    ToolExecutionContext {
        config_path: root.join(".hellox").join("config.toml"),
        planning_state: Arc::new(Mutex::new(PlanningState::default())),
        working_directory: root.clone(),
        permission_policy: PermissionPolicy::new(PermissionMode::BypassPermissions, root),
        approval_handler: None,
        question_handler: None,
        telemetry_sink: None,
    }
}

#[test]
fn resolve_child_working_directory_creates_managed_worktree() {
    let root = temp_dir();
    init_repo(&root);

    let resolved = resolve_child_working_directory(
        &context(root.clone()),
        None,
        Some("worktree"),
        Some("review"),
        None,
        false,
        Some("reviewer"),
    )
    .expect("resolve worktree");

    assert!(
        resolved.ends_with(".hellox/worktrees/review"),
        "{}",
        resolved.display()
    );
    assert!(resolved.exists(), "{}", resolved.display());
}
