use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hellox_gateway_api::{ContentBlock, DocumentSource, ImageSource, ToolResultContent};
use hellox_tool_runtime::{LocalTool, ToolRegistry};
use serde_json::json;
use uuid::Uuid;

use crate::files::{EditFileTool, ReadFileTool, WriteFileTool};
use crate::notebook::NotebookEditTool;
use crate::search::{GlobTool, GrepTool};
use crate::support::{compile_glob_pattern, matches_glob};
use crate::{register_tools, FsToolContext};

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new() -> Self {
        let root = env::temp_dir().join(format!("hellox-tools-fs-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp workspace");
        Self { root }
    }

    fn context(&self) -> TestContext {
        TestContext {
            root: self.root.clone(),
            denied_paths: Arc::new(Mutex::new(Vec::new())),
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

#[derive(Clone)]
struct TestContext {
    root: PathBuf,
    denied_paths: Arc<Mutex<Vec<PathBuf>>>,
}

#[async_trait]
impl FsToolContext for TestContext {
    fn resolve_path(&self, raw: &str) -> PathBuf {
        let path = PathBuf::from(raw);
        if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        }
    }

    fn working_directory(&self) -> &Path {
        &self.root
    }

    async fn ensure_write_allowed(&self, path: &Path) -> anyhow::Result<()> {
        let denied = self.denied_paths.lock().expect("lock");
        if denied.iter().any(|item| item == path) {
            anyhow::bail!("blocked path: {}", path.display());
        }
        Ok(())
    }
}

fn text_result(result: hellox_tool_runtime::LocalToolResult) -> String {
    match result.content {
        ToolResultContent::Text(text) => text,
        other => panic!("expected text result, got {other:?}"),
    }
}

fn blocks_result(result: hellox_tool_runtime::LocalToolResult) -> Vec<ContentBlock> {
    match result.content {
        ToolResultContent::Blocks(blocks) => blocks,
        ToolResultContent::Text(text) => vec![ContentBlock::Text { text }],
        ToolResultContent::Empty => Vec::new(),
    }
}

fn flatten_text_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn list_files_reports_nested_entries() {
    let workspace = TestWorkspace::new();
    workspace.write("src/main.rs", "fn main() {}\n");
    workspace.write("README.md", "# hello\n");
    let mut registry = ToolRegistry::<TestContext>::default();
    register_tools(&mut registry);

    let output = text_result(
        registry
            .execute(
                "ListFiles",
                json!({ "path": ".", "recursive": true }),
                &workspace.context(),
            )
            .await,
    );

    assert!(output.contains("dir  src"), "{output}");
    assert!(output.contains("file src/main.rs"), "{output}");
    assert!(output.contains("file README.md"), "{output}");
}

#[tokio::test]
async fn write_file_creates_parent_directories() {
    let workspace = TestWorkspace::new();
    let context = workspace.context();

    let result = WriteFileTool
        .call(
            json!({
                "path": "notes/daily/todo.txt",
                "content": "ship tools split"
            }),
            &context,
        )
        .await
        .expect("write file");

    assert!(!result.is_error);
    assert_eq!(
        fs::read_to_string(workspace.root.join("notes/daily/todo.txt")).expect("read file"),
        "ship tools split"
    );
}

#[tokio::test]
async fn edit_file_replaces_unique_match() {
    let workspace = TestWorkspace::new();
    workspace.write("notes.txt", "hello world");

    let result = EditFileTool
        .call(
            json!({
                "path": "notes.txt",
                "old_text": "world",
                "new_text": "hellox"
            }),
            &workspace.context(),
        )
        .await
        .expect("edit file");

    assert!(!result.is_error);
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

    let result = GlobTool
        .call(json!({ "pattern": "src/**/*.rs" }), &workspace.context())
        .await
        .expect("glob");

    let output = text_result(result);
    assert!(output.contains("src/main.rs"), "{output}");
    assert!(output.contains("src/lib.rs"), "{output}");
    assert!(!output.contains("README.md"), "{output}");
}

#[tokio::test]
async fn grep_returns_matching_lines_with_context() {
    let workspace = TestWorkspace::new();
    workspace.write(
        "src/main.rs",
        "use anyhow::Result;\nfn main() {\n    println!(\"hello\");\n}\n",
    );
    workspace.write("README.md", "Result is documented here\n");

    let result = GrepTool
        .call(
            json!({
                "pattern": "Result",
                "include": "**/*.rs",
                "context": 1
            }),
            &workspace.context(),
        )
        .await
        .expect("grep");

    let output = text_result(result);
    assert!(
        output.contains("src/main.rs:1:use anyhow::Result;"),
        "{output}"
    );
    assert!(output.contains("  2:fn main() {"), "{output}");
    assert!(!output.contains("README.md"), "{output}");
}

#[tokio::test]
async fn read_file_returns_notebook_metadata() {
    let workspace = TestWorkspace::new();
    workspace.write(
        "analysis.ipynb",
        r##"{
  "cells": [
    {
      "cell_type": "markdown",
      "source": ["# Hello", "\n", "Notebook preview"]
    },
    {
      "cell_type": "code",
      "source": ["print('hi')"]
    }
  ],
  "metadata": {
    "kernelspec": { "name": "python3" },
    "language_info": { "name": "python" }
  },
  "nbformat": 4,
  "nbformat_minor": 5
}"##,
    );

    let result = ReadFileTool
        .call(json!({ "path": "analysis.ipynb" }), &workspace.context())
        .await
        .expect("read notebook");

    let blocks = blocks_result(result);
    let output = flatten_text_blocks(&blocks);
    assert!(
        output.contains("type: application/x-ipynb+json"),
        "{output}"
    );
    assert!(output.contains("cells: 2"), "{output}");
    assert!(output.contains("code_cells: 1"), "{output}");
    assert!(output.contains("markdown_cells: 1"), "{output}");
    assert!(output.contains("language: python"), "{output}");
    assert!(
        output.contains("preview: # Hello Notebook preview"),
        "{output}"
    );
    assert!(
        blocks
            .iter()
            .any(|block| matches!(block, ContentBlock::Text { text } if text.contains("[cell 1]"))),
        "{output}"
    );
}

#[tokio::test]
async fn notebook_edit_replaces_existing_cell() {
    let workspace = TestWorkspace::new();
    workspace.write(
        "analysis.ipynb",
        r##"{
  "cells": [
    {
      "cell_type": "markdown",
      "metadata": {},
      "source": ["# Title", "\n", "Old text"]
    },
    {
      "cell_type": "code",
      "metadata": {},
      "execution_count": 3,
      "outputs": [{"output_type": "stream"}],
      "source": ["print('old')"]
    }
  ],
  "metadata": {
    "kernelspec": { "name": "python3" },
    "language_info": { "name": "python" }
  },
  "nbformat": 4,
  "nbformat_minor": 5
}"##,
    );

    let result = NotebookEditTool
        .call(
            json!({
                "notebook_path": "analysis.ipynb",
                "cell_number": 2,
                "new_source": "print('new')"
            }),
            &workspace.context(),
        )
        .await
        .expect("edit notebook");

    let output = text_result(result);
    assert!(output.contains("\"cell_number\": 2"), "{output}");
    assert!(output.contains("\"cell_type\": \"code\""), "{output}");

    let notebook: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(workspace.root.join("analysis.ipynb")).expect("read notebook"),
    )
    .expect("parse notebook");
    assert_eq!(notebook["cells"][1]["source"], json!(["print('new')"]));
    assert_eq!(
        notebook["cells"][1]["execution_count"],
        serde_json::Value::Null
    );
    assert_eq!(notebook["cells"][1]["outputs"], json!([]));
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

    NotebookEditTool
        .call(
            json!({
                "notebook_path": "analysis.ipynb",
                "edit_mode": "append",
                "cell_type": "markdown",
                "new_source": "## Notes\nMore detail"
            }),
            &workspace.context(),
        )
        .await
        .expect("append notebook cell");

    let notebook: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(workspace.root.join("analysis.ipynb")).expect("read notebook"),
    )
    .expect("parse notebook");
    assert_eq!(notebook["cells"].as_array().expect("cells").len(), 2);
    assert_eq!(notebook["cells"][1]["cell_type"], json!("markdown"));
    assert_eq!(
        notebook["cells"][1]["source"],
        json!(["## Notes\n", "More detail"])
    );
}

#[tokio::test]
async fn read_file_returns_pdf_metadata() {
    let workspace = TestWorkspace::new();
    workspace.write_bytes(
        "spec.pdf",
        b"%PDF-1.4\n1 0 obj << /Type /Page >>\n2 0 obj << /Type /Pages >>\n3 0 obj << /Type /Page >>\n",
    );

    let result = ReadFileTool
        .call(json!({ "path": "spec.pdf" }), &workspace.context())
        .await
        .expect("read pdf");

    let blocks = blocks_result(result);
    let output = flatten_text_blocks(&blocks);
    assert!(output.contains("type: application/pdf"), "{output}");
    assert!(output.contains("pages: 2"), "{output}");
    assert!(
        blocks.iter().any(|block| matches!(block, ContentBlock::Document { source: DocumentSource::Base64 { media_type, .. }, .. } if media_type == "application/pdf")),
        "{output}"
    );
}

#[tokio::test]
async fn read_file_returns_image_metadata() {
    let workspace = TestWorkspace::new();
    workspace.write_bytes(
        "pixel.png",
        &[
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89,
        ],
    );

    let result = ReadFileTool
        .call(json!({ "path": "pixel.png" }), &workspace.context())
        .await
        .expect("read image");

    let blocks = blocks_result(result);
    let output = flatten_text_blocks(&blocks);
    assert!(output.contains("type: image/png"), "{output}");
    assert!(output.contains("dimensions: 1x1"), "{output}");
    assert!(
        blocks.iter().any(|block| matches!(block, ContentBlock::Image { source: ImageSource::Base64 { media_type, .. } } if media_type == "image/png")),
        "{output}"
    );
}

#[test]
fn normalize_paths_uses_workspace_relative_matches() {
    let root = Path::new("D:/repo");
    let path = Path::new("D:/repo/src/main.rs");
    let pattern = compile_glob_pattern("src/**/*.rs").expect("pattern");
    assert!(matches_glob(&pattern, root, path));
}
