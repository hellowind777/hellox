use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{default_worktree_name, enter_worktree, exit_worktree, ExitWorktreeAction};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("hellox-worktree-{suffix}"));
    std::fs::create_dir_all(&root).expect("create temp dir");
    root
}

fn git(directory: &PathBuf, args: &[&str]) {
    let output = std::process::Command::new("git")
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
    std::fs::write(root.join("README.md"), "hello\n").expect("write readme");
    git(root, &["add", "README.md"]);
    git(root, &["commit", "-m", "init"]);
}

#[test]
fn creates_and_removes_managed_worktree() {
    let root = temp_dir();
    init_repo(&root);

    let created = enter_worktree(&root, "review", None, false).expect("create worktree");
    assert!(created.path.exists(), "{}", created.path.display());

    let removed = exit_worktree(&root, Some("review"), None, ExitWorktreeAction::Remove)
        .expect("remove worktree");
    assert_eq!(removed.name, "review");
    assert!(!created.path.exists(), "{}", created.path.display());
}

#[test]
fn default_name_is_stable_and_sanitized() {
    let generated = default_worktree_name(Some("Review Agent"));
    assert!(generated.starts_with("review-agent-"), "{generated}");
}
