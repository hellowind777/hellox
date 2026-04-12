use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_config::load_or_default;
use hellox_gateway_api::ToolDefinition;
use hellox_tool_runtime::{required_string, LocalTool, LocalToolResult, ToolRegistry};
use serde_json::{json, Value};

/// Minimal context contract shared by MCP-facing local tools.
pub trait McpToolContext: Send + Sync {
    /// Returns the active local config path.
    fn config_path(&self) -> &Path;
}

/// Registers MCP-facing tools into a shared runtime registry.
pub fn register_tools<C>(registry: &mut ToolRegistry<C>)
where
    C: McpToolContext + Send + Sync + 'static,
{
    registry.register(McpTool);
    registry.register(ListMcpResourcesTool);
    registry.register(ReadMcpResourceTool);
    registry.register(ListMcpPromptsTool);
    registry.register(GetMcpPromptTool);
    registry.register(McpAuthTool);
}

pub struct McpTool;

#[async_trait]
impl<C> LocalTool<C> for McpTool
where
    C: McpToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "MCP".to_string(),
            description: Some("Call a configured MCP tool by server name.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string" },
                    "tool_name": { "type": "string" },
                    "input": { "type": "object" }
                },
                "required": ["server_name", "tool_name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let tool_name = required_string(&input, "tool_name")?;
        let arguments = optional_object(&input, "input")?;
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        let server = crate::get_server(&config, server_name)?;
        let auth_backend = crate::auth_backend_for_config_path(context.config_path());
        let result = crate::call_tool(&auth_backend, server_name, server, tool_name, arguments)?;

        Ok(LocalToolResult::text(crate::format_tool_call(
            server_name,
            tool_name,
            &result,
        )))
    }
}

pub struct ListMcpResourcesTool;

#[async_trait]
impl<C> LocalTool<C> for ListMcpResourcesTool
where
    C: McpToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ListMcpResources".to_string(),
            description: Some("List resources exposed by a configured MCP server.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string" }
                },
                "required": ["server_name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        let server = crate::get_server(&config, server_name)?;
        let auth_backend = crate::auth_backend_for_config_path(context.config_path());
        let result = crate::list_resources(&auth_backend, server_name, server)?;

        Ok(LocalToolResult::text(crate::format_resource_list(
            server_name,
            &result,
        )))
    }
}

pub struct ReadMcpResourceTool;

#[async_trait]
impl<C> LocalTool<C> for ReadMcpResourceTool
where
    C: McpToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ReadMcpResource".to_string(),
            description: Some("Read a specific resource from a configured MCP server.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string" },
                    "uri": { "type": "string" }
                },
                "required": ["server_name", "uri"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let uri = required_string(&input, "uri")?;
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        let server = crate::get_server(&config, server_name)?;
        let auth_backend = crate::auth_backend_for_config_path(context.config_path());
        let result = crate::read_resource(&auth_backend, server_name, server, uri)?;

        Ok(LocalToolResult::text(crate::format_resource_read(
            server_name,
            uri,
            &result,
        )))
    }
}

pub struct ListMcpPromptsTool;

#[async_trait]
impl<C> LocalTool<C> for ListMcpPromptsTool
where
    C: McpToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ListMcpPrompts".to_string(),
            description: Some("List prompts exposed by a configured MCP server.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string" }
                },
                "required": ["server_name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        let server = crate::get_server(&config, server_name)?;
        let auth_backend = crate::auth_backend_for_config_path(context.config_path());
        let result = crate::list_prompts(&auth_backend, server_name, server)?;

        Ok(LocalToolResult::text(crate::format_prompt_list(
            server_name,
            &result,
        )))
    }
}

pub struct GetMcpPromptTool;

#[async_trait]
impl<C> LocalTool<C> for GetMcpPromptTool
where
    C: McpToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "GetMcpPrompt".to_string(),
            description: Some(
                "Fetch a prompt definition from a configured MCP server.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string" },
                    "prompt_name": { "type": "string" },
                    "arguments": { "type": "object" }
                },
                "required": ["server_name", "prompt_name"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let prompt_name = required_string(&input, "prompt_name")?;
        let arguments = optional_object(&input, "arguments")?;
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        let server = crate::get_server(&config, server_name)?;
        let auth_backend = crate::auth_backend_for_config_path(context.config_path());
        let result = crate::get_prompt(&auth_backend, server_name, server, prompt_name, arguments)?;

        Ok(LocalToolResult::text(crate::format_prompt_get(
            server_name,
            prompt_name,
            &result,
        )))
    }
}

pub struct McpAuthTool;

#[async_trait]
impl<C> LocalTool<C> for McpAuthTool
where
    C: McpToolContext + Send + Sync,
{
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "McpAuth".to_string(),
            description: Some(
                "Show, set, or clear local bearer-token and OAuth helper state for an MCP server."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string" },
                    "action": {
                        "type": "string",
                        "enum": [
                            "show",
                            "set_token",
                            "clear",
                            "oauth_start",
                            "oauth_exchange",
                            "oauth_refresh",
                            "oauth_clear"
                        ]
                    },
                    "bearer_token": { "type": "string" },
                    "code": { "type": "string" },
                    "code_verifier": { "type": "string" }
                },
                "required": ["server_name", "action"]
            }),
        }
    }

    async fn call(&self, input: Value, context: &C) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let action = required_string(&input, "action")?;
        let config = load_or_default(Some(context.config_path().to_path_buf()))?;
        let server = crate::get_server(&config, server_name)?;
        let auth_backend = crate::auth_backend_for_config_path(context.config_path());

        let text = match action {
            "show" => crate::format_auth_status(&auth_backend, server_name, server)?,
            "set_token" => {
                let bearer_token = required_string(&input, "bearer_token")?;
                crate::set_bearer_token(
                    &auth_backend,
                    server_name,
                    server,
                    bearer_token.to_string(),
                )?;
                format!("Stored MCP bearer token for `{server_name}`.")
            }
            "clear" => {
                if crate::clear_bearer_token(&auth_backend, server_name)? {
                    format!("Cleared MCP bearer token for `{server_name}`.")
                } else {
                    format!("No stored MCP bearer token found for `{server_name}`.")
                }
            }
            "oauth_start" => {
                let request = crate::start_server_oauth_authorization(server_name, server)?;
                format!(
                    "server: {server_name}\nauthorization_url: {}\ncode_verifier: {}\nstate: {}",
                    request.authorization_url, request.code_verifier, request.state
                )
            }
            "oauth_exchange" => {
                let code = required_string(&input, "code")?;
                let code_verifier = required_string(&input, "code_verifier")?;
                let account = crate::exchange_server_oauth_authorization_code(
                    &auth_backend,
                    server_name,
                    server,
                    code,
                    code_verifier,
                )?;
                format!(
                    "Stored MCP OAuth account `{}` for `{server_name}` (provider: {}).",
                    account.account_id, account.provider
                )
            }
            "oauth_refresh" => {
                let account =
                    crate::refresh_server_oauth_access_token(&auth_backend, server_name, server)?;
                format!(
                    "Refreshed MCP OAuth account `{}` for `{server_name}`.",
                    account.account_id
                )
            }
            "oauth_clear" => {
                if crate::clear_server_oauth_account(&auth_backend, server_name, server)? {
                    format!("Cleared linked MCP OAuth account for `{server_name}`.")
                } else {
                    format!("No linked MCP OAuth account found for `{server_name}`.")
                }
            }
            _ => return Err(anyhow!("unsupported McpAuth action `{action}`")),
        };

        Ok(LocalToolResult::text(text))
    }
}

fn optional_object(input: &Value, key: &str) -> Result<Option<Value>> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) if value.is_object() => Ok(Some(value.clone())),
        Some(_) => Err(anyhow!("field `{key}` must be a JSON object when provided")),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::optional_object;

    #[test]
    fn optional_object_accepts_objects_and_null() {
        assert_eq!(
            optional_object(&json!({ "input": { "path": "README.md" } }), "input")
                .expect("parse object"),
            Some(json!({ "path": "README.md" }))
        );
        assert_eq!(
            optional_object(&json!({ "input": null }), "input").expect("parse null"),
            None
        );
    }

    #[test]
    fn optional_object_rejects_scalars() {
        let error = optional_object(&json!({ "input": "README.md" }), "input")
            .expect_err("string must be rejected");
        assert!(error.to_string().contains("JSON object"));
    }
}
