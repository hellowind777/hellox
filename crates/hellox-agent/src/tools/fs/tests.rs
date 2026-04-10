use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hellox_gateway_api::{ContentBlock, ToolResultContent};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::permissions::PermissionPolicy;
use crate::planning::PlanningState;
use crate::tools::{default_tool_registry, ToolExecutionContext};
use hellox_config::PermissionMode;

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new() -> Self {
        let root = env::temp_dir().join(format!("hellox-agent-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp workspace");
        Self { root }
    }

    fn context(&self) -> ToolExecutionContext {
        ToolExecutionContext {
            config_path: self.root.join(".hellox").join("config.toml"),
            planning_state: Arc::new(Mutex::new(PlanningState::default())),
            working_directory: self.root.clone(),
            permission_policy: PermissionPolicy::new(
                PermissionMode::BypassPermissions,
                self.root.clone(),
            ),
            approval_handler: None,
            question_handler: None,
            telemetry_sink: None,
        }
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write fixture");
    }

    fn write_bytes(&self, relative: &str, content: &[u8]) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write fixture");
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn text_result(result: crate::tools::LocalToolResult) -> String {
    match result.content {
        ToolResultContent::Text(text) => text,
        ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        ToolResultContent::Empty => String::new(),
    }
}

async fn execute(name: &str, input: Value, context: &ToolExecutionContext) -> String {
    let registry = default_tool_registry();
    text_result(registry.execute(name, input, context).await)
}

#[tokio::test]
async fn edit_file_replaces_unique_match() {
    let workspace = TestWorkspace::new();
    workspace.write("notes.txt", "hello world");

    let output = execute(
        "Edit",
        json!({
            "file_path": "notes.txt",
            "old_text": "world",
            "new_text": "hellox"
        }),
        &workspace.context(),
    )
    .await;

    assert!(output.contains("\"replacements\""), "{output}");
    assert_eq!(
        fs::read_to_string(workspace.root.join("notes.txt")).expect("read file"),
        "hello hellox"
    );
}

#[tokio::test]
async fn glob_matches_nested_rust_files() {
    let workspace = TestWorkspace::new();
    workspace.write("src/main.rs", "fn main() {}\n");
    workspace.write("src/lib.rs", "pub fn lib() {}\n");
    workspace.write("README.md", "# hello\n");

    let output = execute(
        "Glob",
        json!({ "pattern": "src/**/*.rs" }),
        &workspace.context(),
    )
    .await;
    assert!(output.contains("src/main.rs"), "{output}");
    assert!(output.contains("src/lib.rs"), "{output}");
    assert!(!output.contains("README.md"), "{output}");
}

#[tokio::test]
async fn read_file_returns_notebook_metadata() {
    let workspace = TestWorkspace::new();
    workspace.write(
        "analysis.ipynb",
        r##"{
  "cells": [
    { "cell_type": "markdown", "source": ["# Hello", "\n", "Notebook preview"] },
    { "cell_type": "code", "source": ["print('hi')"] }
  ],
  "metadata": {
    "kernelspec": { "name": "python3" },
    "language_info": { "name": "python" }
  },
  "nbformat": 4,
  "nbformat_minor": 5
}"##,
    );

    let output = execute(
        "Read",
        json!({ "file_path": "analysis.ipynb" }),
        &workspace.context(),
    )
    .await;

    assert!(
        output.contains("type: application/x-ipynb+json"),
        "{output}"
    );
    assert!(
        output.contains("preview: # Hello Notebook preview"),
        "{output}"
    );
}

#[tokio::test]
async fn notebook_edit_appends_new_markdown_cell() {
    let workspace = TestWorkspace::new();
    workspace.write(
        "analysis.ipynb",
        r##"{
  "cells": [
    {
      "cell_type": "code",
      "metadata": {},
      "execution_count": null,
      "outputs": [],
      "source": ["print('hi')"]
    }
  ],
  "metadata": {},
  "nbformat": 4,
  "nbformat_minor": 5
}"##,
    );

    let output = execute(
        "NotebookEdit",
        json!({
            "notebook_path": "analysis.ipynb",
            "edit_mode": "append",
            "cell_type": "markdown",
            "new_source": "## Notes\nMore detail"
        }),
        &workspace.context(),
    )
    .await;
    assert!(output.contains("\"cell_type\": \"markdown\""), "{output}");

    let notebook: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(workspace.root.join("analysis.ipynb")).expect("read notebook"),
    )
    .expect("parse notebook");
    assert_eq!(notebook["cells"].as_array().expect("cells").len(), 2);
}

#[tokio::test]
async fn read_file_returns_pdf_and_image_metadata() {
    let workspace = TestWorkspace::new();
    workspace.write_bytes(
        "spec.pdf",
        b"%PDF-1.4\n1 0 obj << /Type /Page >>\n2 0 obj << /Type /Pages >>\n3 0 obj << /Type /Page >>\n",
    );
    workspace.write_bytes(
        "pixel.png",
        &[
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89,
        ],
    );

    let pdf = execute(
        "Read",
        json!({ "file_path": "spec.pdf" }),
        &workspace.context(),
    )
    .await;
    let image = execute(
        "Read",
        json!({ "file_path": "pixel.png" }),
        &workspace.context(),
    )
    .await;

    assert!(pdf.contains("type: application/pdf"), "{pdf}");
    assert!(pdf.contains("pages: 2"), "{pdf}");
    assert!(image.contains("type: image/png"), "{image}");
    assert!(image.contains("dimensions: 1x1"), "{image}");
}
