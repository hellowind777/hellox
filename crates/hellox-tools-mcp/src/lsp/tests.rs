use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde_json::{json, Value};

use crate::lsp::client::LspClient;
use crate::lsp::config::ResolvedLspServer;
use crate::lsp::{execute_operation_with_client, LspInput};
use hellox_config::LspServerConfig;

struct MockLspClient {
    requests: VecDeque<(&'static str, Value)>,
    notifications: Vec<(&'static str, Value)>,
}

impl MockLspClient {
    fn new(requests: Vec<(&'static str, Value)>) -> Self {
        Self {
            requests: requests.into(),
            notifications: Vec::new(),
        }
    }
}

impl LspClient for MockLspClient {
    fn initialize(&mut self, _root: &Path) -> Result<()> {
        Ok(())
    }

    fn did_open(&mut self, file_path: &Path, language_id: &str, _text: &str) -> Result<()> {
        let uri = Url::from_file_path(file_path)
            .map_err(|_| anyhow!("bad test file path `{}`", file_path.display()))?;
        self.notifications.push((
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri.as_str(),
                    "languageId": language_id
                }
            }),
        ));
        Ok(())
    }

    fn request(&mut self, method: &str, _params: Value) -> Result<Value> {
        let Some((expected, result)) = self.requests.pop_front() else {
            return Err(anyhow!("unexpected LSP request `{method}`"));
        };
        if expected != method {
            return Err(anyhow!("expected LSP request `{expected}`, got `{method}`"));
        }
        Ok(result)
    }
}

fn resolved_server() -> ResolvedLspServer {
    let root = std::env::temp_dir().join("hellox-lsp-test-root");
    let file_path = root.join("src").join("main.rs");
    ResolvedLspServer {
        name: "rust-analyzer".to_string(),
        config: LspServerConfig {
            enabled: true,
            description: None,
            command: "rust-analyzer".to_string(),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            language_id: Some("rust".to_string()),
            file_extensions: vec!["rs".to_string()],
            root_markers: vec![".git".to_string()],
        },
        workspace_root: root,
        file_path,
        language_id: "rust".to_string(),
    }
}

#[test]
fn go_to_definition_formats_locations() {
    let resolved = resolved_server();
    let mut client = MockLspClient::new(vec![(
        "textDocument/definition",
        json!([{
            "uri": "file:///repo/src/lib.rs",
            "range": { "start": { "line": 9, "character": 4 } }
        }]),
    )]);

    let output = execute_operation_with_client(
        &mut client,
        &resolved,
        &LspInput {
            operation: "goToDefinition".to_string(),
            file_path: "src/main.rs".to_string(),
            line: 4,
            character: 3,
        },
        "fn main() {}\n",
    )
    .expect("execute definition");

    assert!(output.text.contains("file:///repo/src/lib.rs:10:5"));
    assert_eq!(output.result_count, 1);
    assert_eq!(client.notifications.len(), 1);
}

#[test]
fn incoming_calls_uses_prepare_call_hierarchy_first() {
    let resolved = resolved_server();
    let mut client = MockLspClient::new(vec![
        (
            "textDocument/prepareCallHierarchy",
            json!([{
                "name": "main",
                "uri": "file:///repo/src/main.rs",
                "range": { "start": { "line": 0, "character": 0 } }
            }]),
        ),
        (
            "callHierarchy/incomingCalls",
            json!([{
                "from": {
                    "name": "bootstrap",
                    "uri": "file:///repo/src/bootstrap.rs"
                }
            }]),
        ),
    ]);

    let output = execute_operation_with_client(
        &mut client,
        &resolved,
        &LspInput {
            operation: "incomingCalls".to_string(),
            file_path: "src/main.rs".to_string(),
            line: 1,
            character: 1,
        },
        "fn main() {}\n",
    )
    .expect("execute incoming calls");

    assert!(output.text.contains("bootstrap"));
    assert_eq!(output.result_count, 1);
}
