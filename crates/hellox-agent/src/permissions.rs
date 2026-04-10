use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use hellox_config::PermissionMode;
use serde::{Deserialize, Serialize};
use tokio::task;

#[derive(Clone)]
pub struct PermissionPolicy {
    mode: PermissionMode,
    workspace_root: PathBuf,
}

impl PermissionPolicy {
    pub fn new(mode: PermissionMode, workspace_root: PathBuf) -> Self {
        Self {
            mode,
            workspace_root,
        }
    }

    pub fn check_shell_command(&self, command: &str) -> PermissionDecision {
        let normalized = command.to_ascii_lowercase();

        if hard_blocked_command(&normalized) {
            return PermissionDecision::Deny(
                "shell command matches a hard-blocked destructive pattern".to_string(),
            );
        }

        if matches!(self.mode, PermissionMode::BypassPermissions) {
            return PermissionDecision::Allow;
        }

        if is_read_only_or_build_command(&normalized) {
            return PermissionDecision::Allow;
        }

        if looks_risky_command(&normalized) {
            return PermissionDecision::Ask(format!("approve shell command?\n{command}"));
        }

        PermissionDecision::Ask(format!("approve shell command?\n{command}"))
    }

    pub fn check_write_path(&self, path: &Path) -> PermissionDecision {
        if matches!(self.mode, PermissionMode::BypassPermissions) {
            return PermissionDecision::Allow;
        }

        if is_within_workspace(&self.workspace_root, path) {
            return PermissionDecision::Allow;
        }

        PermissionDecision::Ask(format!(
            "approve write outside workspace?\n{}",
            path.display()
        ))
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn mode(&self) -> &PermissionMode {
        &self.mode
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
    }
}

pub enum PermissionDecision {
    Allow,
    Ask(String),
    Deny(String),
}

#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn confirm(&self, prompt: &str) -> Result<bool>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserQuestion {
    pub question: String,
    pub header: Option<String>,
}

#[async_trait]
pub trait QuestionHandler: Send + Sync {
    async fn ask_questions(&self, questions: &[UserQuestion]) -> Result<Vec<String>>;
}

#[derive(Clone, Default)]
pub struct ConsoleApprovalHandler;

#[async_trait]
impl ApprovalHandler for ConsoleApprovalHandler {
    async fn confirm(&self, prompt: &str) -> Result<bool> {
        let prompt = prompt.to_string();
        task::spawn_blocking(move || -> Result<bool> {
            print!("{prompt} [y/N]: ");
            io::stdout()
                .flush()
                .context("failed to flush approval prompt")?;
            let mut line = String::new();
            io::stdin()
                .read_line(&mut line)
                .context("failed to read approval input")?;
            let value = line.trim().to_ascii_lowercase();
            Ok(matches!(value.as_str(), "y" | "yes"))
        })
        .await
        .context("approval prompt task failed")?
    }
}

#[async_trait]
impl QuestionHandler for ConsoleApprovalHandler {
    async fn ask_questions(&self, questions: &[UserQuestion]) -> Result<Vec<String>> {
        let questions = questions.to_vec();
        task::spawn_blocking(move || -> Result<Vec<String>> {
            let mut answers = Vec::with_capacity(questions.len());
            for question in questions {
                if let Some(header) = &question.header {
                    println!("{header}");
                }
                print!("{} ", question.question);
                io::stdout()
                    .flush()
                    .context("failed to flush question prompt")?;
                let mut line = String::new();
                io::stdin()
                    .read_line(&mut line)
                    .context("failed to read question input")?;
                answers.push(line.trim_end().to_string());
            }
            Ok(answers)
        })
        .await
        .context("question prompt task failed")?
    }
}

pub async fn resolve_permission(
    decision: PermissionDecision,
    approver: Option<Arc<dyn ApprovalHandler>>,
) -> Result<Option<String>> {
    match decision {
        PermissionDecision::Allow => Ok(None),
        PermissionDecision::Deny(message) => Ok(Some(message)),
        PermissionDecision::Ask(prompt) => match approver {
            Some(approver) => {
                let approved = approver.confirm(&prompt).await?;
                if approved {
                    Ok(None)
                } else {
                    Ok(Some("operation denied by user approval policy".to_string()))
                }
            }
            None => Ok(Some(
                "operation requires approval but no approval handler is configured".to_string(),
            )),
        },
    }
}

fn hard_blocked_command(command: &str) -> bool {
    [
        "rm -rf /",
        "git push --force main",
        "git reset --hard",
        "drop database",
        "drop table",
        "truncate ",
        "chmod 777",
        "mkfs",
        "dd of=/dev/",
        "flushall",
        "flushdb",
    ]
    .iter()
    .any(|pattern| command.contains(pattern))
}

fn looks_risky_command(command: &str) -> bool {
    [
        "rm -rf",
        "git push",
        "git commit",
        "git tag",
        "del ",
        "remove-item",
        "move-item",
        "rename-item",
        "curl ",
        "invoke-webrequest",
        "gh pr",
        "gh issue",
        "npm publish",
    ]
    .iter()
    .any(|pattern| command.contains(pattern))
}

fn is_read_only_or_build_command(command: &str) -> bool {
    let prefixes = [
        "rg ",
        "rg\n",
        "grep ",
        "find ",
        "ls",
        "dir",
        "pwd",
        "git status",
        "git diff",
        "git log",
        "cat ",
        "head ",
        "tail ",
        "sed -n",
        "get-content",
        "get-childitem",
        "cargo check",
        "cargo test",
        "cargo fmt",
        "cargo clippy",
        "npm test",
        "npm run",
        "bun test",
        "bun run",
        "pytest",
        "python -m pytest",
        "go test",
    ];

    prefixes.iter().any(|prefix| command.starts_with(prefix))
}

fn is_within_workspace(workspace_root: &Path, path: &Path) -> bool {
    let root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let candidate = canonicalize_best_effort(path);
    candidate.starts_with(&root)
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    if path.exists() {
        return path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    }

    match path.parent() {
        Some(parent) => {
            let canonical_parent = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            match path.file_name() {
                Some(name) => canonical_parent.join(name),
                None => canonical_parent,
            }
        }
        None => path.to_path_buf(),
    }
}
