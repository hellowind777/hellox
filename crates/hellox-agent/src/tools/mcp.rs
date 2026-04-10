use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hellox_config::load_or_default;
use serde_json::{json, Value};

use super::{required_string, LocalTool, LocalToolResult, ToolExecutionContext, ToolRegistry};

pub(super) fn register_tools(registry: &mut ToolRegistry) {
    registry.register(McpTool);
    registry.register(ListMcpResourcesTool);
    registry.register(ReadMcpResourceTool);
    registry.register(ListMcpPromptsTool);
    registry.register(GetMcpPromptTool);
    registry.register(McpAuthTool);
}

struct McpTool;

#[async_trait]
impl LocalTool for McpTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
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

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let tool_name = required_string(&input, "tool_name")?;
        let arguments = optional_object(&input, "input")?;
        let config = load_or_default(Some(context.config_path.clone()))?;
        let server = hellox_tools_mcp::get_server(&config, server_name)?;
        let auth_backend = hellox_tools_mcp::auth_backend_for_config_path(&context.config_path);
        let result =
            hellox_tools_mcp::call_tool(&auth_backend, server_name, server, tool_name, arguments)?;

        Ok(LocalToolResult::text(hellox_tools_mcp::format_tool_call(
            server_name,
            tool_name,
            &result,
        )))
    }
}

struct ListMcpResourcesTool;

#[async_trait]
impl LocalTool for ListMcpResourcesTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
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

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let config = load_or_default(Some(context.config_path.clone()))?;
        let server = hellox_tools_mcp::get_server(&config, server_name)?;
        let auth_backend = hellox_tools_mcp::auth_backend_for_config_path(&context.config_path);
        let result = hellox_tools_mcp::list_resources(&auth_backend, server_name, server)?;

        Ok(LocalToolResult::text(
            hellox_tools_mcp::format_resource_list(server_name, &result),
        ))
    }
}

struct ReadMcpResourceTool;

#[async_trait]
impl LocalTool for ReadMcpResourceTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
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

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let uri = required_string(&input, "uri")?;
        let config = load_or_default(Some(context.config_path.clone()))?;
        let server = hellox_tools_mcp::get_server(&config, server_name)?;
        let auth_backend = hellox_tools_mcp::auth_backend_for_config_path(&context.config_path);
        let result = hellox_tools_mcp::read_resource(&auth_backend, server_name, server, uri)?;

        Ok(LocalToolResult::text(
            hellox_tools_mcp::format_resource_read(server_name, uri, &result),
        ))
    }
}

struct ListMcpPromptsTool;

#[async_trait]
impl LocalTool for ListMcpPromptsTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
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

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let config = load_or_default(Some(context.config_path.clone()))?;
        let server = hellox_tools_mcp::get_server(&config, server_name)?;
        let auth_backend = hellox_tools_mcp::auth_backend_for_config_path(&context.config_path);
        let result = hellox_tools_mcp::list_prompts(&auth_backend, server_name, server)?;

        Ok(LocalToolResult::text(hellox_tools_mcp::format_prompt_list(
            server_name,
            &result,
        )))
    }
}

struct GetMcpPromptTool;

#[async_trait]
impl LocalTool for GetMcpPromptTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
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

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let prompt_name = required_string(&input, "prompt_name")?;
        let arguments = optional_object(&input, "arguments")?;
        let config = load_or_default(Some(context.config_path.clone()))?;
        let server = hellox_tools_mcp::get_server(&config, server_name)?;
        let auth_backend = hellox_tools_mcp::auth_backend_for_config_path(&context.config_path);
        let result = hellox_tools_mcp::get_prompt(
            &auth_backend,
            server_name,
            server,
            prompt_name,
            arguments,
        )?;

        Ok(LocalToolResult::text(hellox_tools_mcp::format_prompt_get(
            server_name,
            prompt_name,
            &result,
        )))
    }
}

struct McpAuthTool;

#[async_trait]
impl LocalTool for McpAuthTool {
    fn definition(&self) -> hellox_gateway_api::ToolDefinition {
        hellox_gateway_api::ToolDefinition {
            name: "McpAuth".to_string(),
            description: Some(
                "Show, set, or clear local bearer-token helper state for an MCP server."
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

    async fn call(&self, input: Value, context: &ToolExecutionContext) -> Result<LocalToolResult> {
        let server_name = required_string(&input, "server_name")?;
        let action = required_string(&input, "action")?;
        let config = load_or_default(Some(context.config_path.clone()))?;
        let server = hellox_tools_mcp::get_server(&config, server_name)?;
        let auth_backend = hellox_tools_mcp::auth_backend_for_config_path(&context.config_path);

        let text = match action {
            "show" => hellox_tools_mcp::format_auth_status(&auth_backend, server_name, server)?,
            "set_token" => {
                let bearer_token = required_string(&input, "bearer_token")?;
                hellox_tools_mcp::set_bearer_token(
                    &auth_backend,
                    server_name,
                    server,
                    bearer_token.to_string(),
                )?;
                format!("Stored MCP bearer token for `{server_name}`.")
            }
            "clear" => {
                if hellox_tools_mcp::clear_bearer_token(&auth_backend, server_name)? {
                    format!("Cleared MCP bearer token for `{server_name}`.")
                } else {
                    format!("No stored MCP bearer token found for `{server_name}`.")
                }
            }
            "oauth_start" => {
                let request =
                    hellox_tools_mcp::start_server_oauth_authorization(server_name, server)?;
                format!(
                    "server: {server_name}\nauthorization_url: {}\ncode_verifier: {}\nstate: {}",
                    request.authorization_url, request.code_verifier, request.state
                )
            }
            "oauth_exchange" => {
                let code = required_string(&input, "code")?;
                let code_verifier = required_string(&input, "code_verifier")?;
                let account = hellox_tools_mcp::exchange_server_oauth_authorization_code(
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
                let account = hellox_tools_mcp::refresh_server_oauth_access_token(
                    &auth_backend,
                    server_name,
                    server,
                )?;
                format!(
                    "Refreshed MCP OAuth account `{}` for `{server_name}`.",
                    account.account_id
                )
            }
            "oauth_clear" => {
                if hellox_tools_mcp::clear_server_oauth_account(&auth_backend, server_name, server)?
                {
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
