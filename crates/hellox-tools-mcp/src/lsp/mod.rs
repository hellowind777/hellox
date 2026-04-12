mod client;
mod config;
mod format;

#[cfg(test)]
mod tests;

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hellox_config::load_or_default;
use hellox_gateway_api::ToolDefinition;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult};
use reqwest::Url;
use serde_json::{json, Value};

use crate::lsp::client::{LspClient, ProcessLspClient};
use crate::lsp::config::{resolve_server, ResolvedLspServer};
use crate::lsp::format::{format_operation, FormattedLspResult};
use crate::McpToolContext;

const MAX_LSP_FILE_SIZE_BYTES: u64 = 10_000_000;

pub struct LspTool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LspInput {
    pub(crate) operation: String,
    pub(crate) file_path: String,
    pub(crate) line: usize,
    pub(crate) character: usize,
}

impl LspInput {
    fn parse(input: &Value) -> Result<Self> {
        let operation = required_string(input, "operation")?.to_string();
        validate_operation(&operation)?;
        Ok(Self {
            operation,
            file_path: required_string(input, "file_path")?.to_string(),
            line: input
                .get("line")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow!("missing required integer field `line`"))?
                as usize,
            character: input
                .get("character")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow!("missing required integer field `character`"))?
                as usize,
        })
    }
}

#[async_trait]
impl<C> LocalTool<C> for LspTool
where
    C: McpToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "LSP".to_string(),
            description: Some(
                "Query a configured local LSP server for definitions, references, hover, symbols, or call hierarchy.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": [
                            "goToDefinition",
                            "findReferences",
                            "hover",
                            "documentSymbol",
                            "workspaceSymbol",
                            "goToImplementation",
                            "prepareCallHierarchy",
                            "incomingCalls",
                            "outgoingCalls"
                        ]
                    },
                    "file_path": { "type": "string" },
                    "line": { "type": "integer", "minimum": 1 },
                    "character": { "type": "integer", "minimum": 1 }
                },
                "required": ["operation", "file_path", "line", "character"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        let parsed = LspInput::parse(&input)?;
        let resolved = resolve_server(&config, Path::new("."), &parsed.file_path)?;
        let metadata = fs::metadata(&resolved.file_path).with_context(|| {
            format!(
                "failed to inspect LSP file metadata `{}`",
                resolved.file_path.display()
            )
        })?;
        if metadata.len() > MAX_LSP_FILE_SIZE_BYTES {
            return Err(anyhow!(
                "LSP file `{}` exceeds {} bytes",
                resolved.file_path.display(),
                MAX_LSP_FILE_SIZE_BYTES
            ));
        }
        let text = fs::read_to_string(&resolved.file_path).with_context(|| {
            format!("failed to read LSP file `{}`", resolved.file_path.display())
        })?;

        let mut client = ProcessLspClient::spawn(&resolved)?;
        let output = execute_operation_with_client(&mut client, &resolved, &parsed, &text)?;

        Ok(LocalToolResult::text(serde_json::to_string_pretty(
            &json!({
                "operation": parsed.operation,
                "server": resolved.name,
                "workspace_root": normalize_path(&resolved.workspace_root),
                "file_path": normalize_path(&resolved.file_path),
                "result_count": output.result_count,
                "file_count": output.file_count,
                "result": output.text,
            }),
        )?))
    }
}

pub(crate) fn execute_operation_with_client(
    client: &mut dyn LspClient,
    resolved: &ResolvedLspServer,
    input: &LspInput,
    text: &str,
) -> Result<FormattedLspResult> {
    client.initialize(&resolved.workspace_root)?;
    client.did_open(&resolved.file_path, &resolved.language_id, text)?;

    let uri = Url::from_file_path(&resolved.file_path).map_err(|_| {
        anyhow!(
            "failed to build file URL for `{}`",
            resolved.file_path.display()
        )
    })?;
    let position = json!({
        "line": input.line.saturating_sub(1),
        "character": input.character.saturating_sub(1),
    });

    let result = match input.operation.as_str() {
        "goToDefinition" => client.request(
            "textDocument/definition",
            json!({ "textDocument": { "uri": uri.as_str() }, "position": position }),
        )?,
        "findReferences" => client.request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri.as_str() },
                "position": position,
                "context": { "includeDeclaration": true }
            }),
        )?,
        "hover" => client.request(
            "textDocument/hover",
            json!({ "textDocument": { "uri": uri.as_str() }, "position": position }),
        )?,
        "documentSymbol" => client.request(
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": uri.as_str() } }),
        )?,
        "workspaceSymbol" => client.request("workspace/symbol", json!({ "query": "" }))?,
        "goToImplementation" => client.request(
            "textDocument/implementation",
            json!({ "textDocument": { "uri": uri.as_str() }, "position": position }),
        )?,
        "prepareCallHierarchy" => client.request(
            "textDocument/prepareCallHierarchy",
            json!({ "textDocument": { "uri": uri.as_str() }, "position": position }),
        )?,
        "incomingCalls" | "outgoingCalls" => {
            let prepared = client.request(
                "textDocument/prepareCallHierarchy",
                json!({ "textDocument": { "uri": uri.as_str() }, "position": position }),
            )?;
            let first = prepared
                .as_array()
                .and_then(|items| items.first())
                .cloned()
                .unwrap_or(Value::Null);
            if first.is_null() {
                Value::Array(Vec::new())
            } else {
                let method = if input.operation == "incomingCalls" {
                    "callHierarchy/incomingCalls"
                } else {
                    "callHierarchy/outgoingCalls"
                };
                client.request(method, json!({ "item": first }))?
            }
        }
        _ => unreachable!(),
    };

    Ok(format_operation(&input.operation, &result))
}

fn validate_operation(operation: &str) -> Result<()> {
    if matches!(
        operation,
        "goToDefinition"
            | "findReferences"
            | "hover"
            | "documentSymbol"
            | "workspaceSymbol"
            | "goToImplementation"
            | "prepareCallHierarchy"
            | "incomingCalls"
            | "outgoingCalls"
    ) {
        Ok(())
    } else {
        Err(anyhow!("unsupported LSP operation `{operation}`"))
    }
}

fn normalize_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
