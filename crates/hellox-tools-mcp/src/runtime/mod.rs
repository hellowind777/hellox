mod elicitation;
mod http;
mod stdio;
mod ws;

use anyhow::{anyhow, Context, Result};
use hellox_auth::LocalAuthStoreBackend;
use hellox_config::{McpServerConfig, McpTransportConfig};
use serde_json::{json, Map, Value};

use crate::auth::transport_headers_with_auth;

const JSONRPC_VERSION: &str = "2.0";
pub(crate) const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

pub fn list_tools(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<Value> {
    with_session(backend, server_name, server, |session| {
        collect_paginated(session, "tools/list", "tools")
    })
}

pub fn call_tool(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
    tool_name: &str,
    arguments: Option<Value>,
) -> Result<Value> {
    let params = match arguments {
        Some(arguments) => json!({
            "name": tool_name,
            "arguments": arguments,
        }),
        None => json!({ "name": tool_name }),
    };

    with_session(backend, server_name, server, |session| {
        session.request("tools/call", Some(params.clone()))
    })
}

pub fn list_resources(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<Value> {
    with_session(backend, server_name, server, |session| {
        collect_paginated(session, "resources/list", "resources")
    })
}

pub fn list_prompts(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
) -> Result<Value> {
    with_session(backend, server_name, server, |session| {
        collect_paginated(session, "prompts/list", "prompts")
    })
}

pub fn read_resource(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
    uri: &str,
) -> Result<Value> {
    let params = json!({ "uri": uri });
    with_session(backend, server_name, server, |session| {
        session.request("resources/read", Some(params.clone()))
    })
}

pub fn get_prompt(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
    prompt_name: &str,
    arguments: Option<Value>,
) -> Result<Value> {
    let params = match arguments {
        Some(arguments) => json!({
            "name": prompt_name,
            "arguments": arguments,
        }),
        None => json!({ "name": prompt_name }),
    };

    with_session(backend, server_name, server, |session| {
        session.request("prompts/get", Some(params.clone()))
    })
}

pub fn parse_tool_call_arguments(input: Option<&str>) -> Result<Option<Value>> {
    parse_object_arguments("MCP tool input", input)
}

pub fn parse_prompt_arguments(input: Option<&str>) -> Result<Option<Value>> {
    parse_object_arguments("MCP prompt arguments", input)
}

fn parse_object_arguments(label: &str, input: Option<&str>) -> Result<Option<Value>> {
    match input.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some(raw) => {
            let value: Value = serde_json::from_str(raw)
                .with_context(|| format!("{label} must be valid JSON."))?;
            if !value.is_object() {
                return Err(anyhow!("{label} must be a JSON object."));
            }
            Ok(Some(value))
        }
    }
}

pub fn format_tool_list(server_name: &str, result: &Value) -> String {
    format_list_result(server_name, "tools", "name", result)
}

pub fn format_resource_list(server_name: &str, result: &Value) -> String {
    format_list_result(server_name, "resources", "uri", result)
}

pub fn format_prompt_list(server_name: &str, result: &Value) -> String {
    format_list_result(server_name, "prompts", "name", result)
}

pub fn format_tool_call(server_name: &str, tool_name: &str, result: &Value) -> String {
    format!(
        "server: {server_name}\ntool: {tool_name}\nresult:\n{}",
        render_json(result)
    )
}

pub fn format_resource_read(server_name: &str, uri: &str, result: &Value) -> String {
    let contents = result
        .get("contents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if contents.is_empty() {
        return format!(
            "server: {server_name}\nuri: {uri}\nresult:\n{}",
            render_json(result)
        );
    }

    let sections = contents
        .iter()
        .map(format_content_item)
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("server: {server_name}\nuri: {uri}\n\n{sections}")
}

pub fn format_prompt_get(server_name: &str, prompt_name: &str, result: &Value) -> String {
    format!(
        "server: {server_name}\nprompt: {prompt_name}\nresult:\n{}",
        render_json(result)
    )
}

pub(crate) trait TransportSession {
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value>;
    fn initialize(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;
}

pub(crate) fn build_request(id: u64, method: &str, params: Option<Value>) -> Value {
    let mut object = Map::from_iter([
        (
            "jsonrpc".to_string(),
            Value::String(JSONRPC_VERSION.to_string()),
        ),
        ("id".to_string(), Value::Number(id.into())),
        ("method".to_string(), Value::String(method.to_string())),
    ]);
    if let Some(params) = params {
        object.insert("params".to_string(), params);
    }
    Value::Object(object)
}

pub(crate) fn build_notification(method: &str, params: Option<Value>) -> Value {
    let mut object = Map::from_iter([
        (
            "jsonrpc".to_string(),
            Value::String(JSONRPC_VERSION.to_string()),
        ),
        ("method".to_string(), Value::String(method.to_string())),
    ]);
    if let Some(params) = params {
        object.insert("params".to_string(), params);
    }
    Value::Object(object)
}

pub(crate) fn build_error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

pub(crate) fn initialize_params() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "elicitation": elicitation::form_capability()
        },
        "clientInfo": {
            "name": "hellox",
            "version": env!("CARGO_PKG_VERSION"),
        }
    })
}

pub(crate) fn process_incoming_message(
    payload: &str,
    expected_id: u64,
    mut send_auxiliary: impl FnMut(Value) -> Result<()>,
) -> Result<Option<Value>> {
    let message: Value = serde_json::from_str(payload)
        .with_context(|| format!("Invalid MCP JSON-RPC payload: {payload}"))?;
    let Some(object) = message.as_object() else {
        return Err(anyhow!("Invalid MCP JSON-RPC payload: expected an object."));
    };

    if let Some(method) = object.get("method").and_then(Value::as_str) {
        if let Some(response) = elicitation::handle_server_method(
            method,
            object.get("id").cloned(),
            object.get("params"),
        )? {
            send_auxiliary(response)?;
        }
        return Ok(None);
    }

    let response_id = object
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("Invalid MCP response id in payload: {payload}"))?;
    if response_id != expected_id {
        return Ok(None);
    }

    if let Some(result) = object.get("result") {
        return Ok(Some(result.clone()));
    }

    let error = object
        .get("error")
        .ok_or_else(|| anyhow!("Invalid MCP response payload: missing `result` or `error`."))?;
    Err(anyhow!(jsonrpc_error_text(error)))
}

fn with_session<T>(
    backend: &LocalAuthStoreBackend,
    server_name: &str,
    server: &McpServerConfig,
    execute: impl FnOnce(&mut dyn TransportSession) -> Result<T>,
) -> Result<T> {
    if !server.enabled {
        return Err(anyhow!("MCP server `{server_name}` is disabled."));
    }

    let headers = transport_headers_with_auth(backend, server_name, server)?;
    let mut session = build_session(server_name, server, headers)?;
    session.initialize()?;
    let result = execute(session.as_mut());
    let _ = session.terminate();
    result
}

fn build_session(
    server_name: &str,
    server: &McpServerConfig,
    headers: std::collections::BTreeMap<String, String>,
) -> Result<Box<dyn TransportSession>> {
    match &server.transport {
        McpTransportConfig::Stdio {
            command,
            args,
            env,
            cwd,
        } => Ok(Box::new(stdio::StdioSession::spawn(
            server_name,
            command,
            args,
            env,
            cwd.as_deref(),
        )?)),
        McpTransportConfig::Sse { url, .. } => Ok(Box::new(http::HttpSession::connect(
            server_name,
            url,
            headers,
        )?)),
        McpTransportConfig::Ws { url, .. } => {
            Ok(Box::new(ws::WsSession::connect(server_name, url, headers)?))
        }
    }
}

fn collect_paginated(
    session: &mut dyn TransportSession,
    method: &str,
    list_key: &str,
) -> Result<Value> {
    let mut combined = Vec::new();
    let mut cursor: Option<String> = None;
    let mut last_page = loop {
        let params = cursor
            .as_ref()
            .map(|value| json!({ "cursor": value }))
            .or(None);
        let result = session.request(method, params)?;
        let page = result
            .as_object()
            .cloned()
            .ok_or_else(|| anyhow!("Invalid MCP `{method}` result: expected an object."))?;

        let items = page
            .get(list_key)
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| anyhow!("Invalid MCP `{method}` result: missing `{list_key}` array."))?;
        combined.extend(items);
        cursor = page
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        if cursor.is_none() {
            break page;
        }
    };
    last_page.insert(list_key.to_string(), Value::Array(combined));
    last_page.remove("nextCursor");
    Ok(Value::Object(last_page))
}

fn format_list_result(
    server_name: &str,
    list_key: &str,
    primary_key: &str,
    result: &Value,
) -> String {
    let items = result
        .get(list_key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return format!("server: {server_name}\n{list_key}: (none)");
    }

    let mut lines = vec![format!("server: {server_name}"), format!("{list_key}:")];
    for item in items {
        let primary = item
            .get(primary_key)
            .and_then(Value::as_str)
            .unwrap_or("(unknown)");
        let title = item.get("title").and_then(Value::as_str);
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .or_else(|| item.get("name").and_then(Value::as_str));
        lines.push(format!(
            "- {}{}{}",
            primary,
            title
                .map(|value| format!(" | title: {value}"))
                .unwrap_or_default(),
            description
                .filter(|value| *value != primary)
                .map(|value| format!(" | description: {value}"))
                .unwrap_or_default()
        ));
    }
    lines.join("\n")
}

fn format_content_item(item: &Value) -> String {
    let uri = item
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or("(unknown-uri)");
    let mime_type = item
        .get("mimeType")
        .and_then(Value::as_str)
        .unwrap_or("(unknown)");

    if let Some(text) = item.get("text").and_then(Value::as_str) {
        return format!("content_uri: {uri}\nmime_type: {mime_type}\ntext:\n{text}");
    }

    if let Some(blob) = item.get("blob").and_then(Value::as_str) {
        return format!(
            "content_uri: {uri}\nmime_type: {mime_type}\nblob_base64_length: {}",
            blob.len()
        );
    }

    render_json(item)
}

fn jsonrpc_error_text(error: &Value) -> String {
    let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32000);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Unknown MCP error");
    if let Ok(Some(elicitation_text)) = elicitation::url_elicitation_error_text(error) {
        return elicitation_text;
    }
    match error.get("data") {
        Some(data) => format!("MCP error {code}: {message} ({})", render_json(data)),
        None => format!("MCP error {code}: {message}"),
    }
}

fn render_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_object_tool_arguments() {
        let error = parse_tool_call_arguments(Some("[1,2,3]")).expect_err("must reject array");
        assert!(error.to_string().contains("JSON object"));
    }

    #[test]
    fn parses_object_tool_arguments() {
        let arguments = parse_tool_call_arguments(Some("{\"path\":\"README.md\"}"))
            .expect("parse json")
            .expect("object args");
        assert_eq!(arguments["path"], Value::String(String::from("README.md")));
    }

    #[test]
    fn rejects_non_object_prompt_arguments() {
        let error = parse_prompt_arguments(Some("\"reviewer\"")).expect_err("must reject string");
        assert!(error.to_string().contains("JSON object"));
    }

    #[test]
    fn initialize_declares_form_elicitation_capability() {
        assert_eq!(
            initialize_params()["capabilities"]["elicitation"]["form"],
            json!({})
        );
    }

    #[test]
    fn process_incoming_message_declines_elicitation_requests() {
        let mut sent = Vec::new();
        let outcome = process_incoming_message(
            &json!({
                "jsonrpc": "2.0",
                "id": 9,
                "method": "elicitation/create",
                "params": {
                    "mode": "form",
                    "title": "Need approval"
                }
            })
            .to_string(),
            1,
            |message| {
                sent.push(message);
                Ok(())
            },
        )
        .expect("process message");

        assert!(outcome.is_none());
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["result"]["action"], "decline");
    }
}
