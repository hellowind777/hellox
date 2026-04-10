use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use super::{build_error_response, JSONRPC_VERSION};

pub(crate) fn form_capability() -> Value {
    json!({
        "form": {}
    })
}

pub(crate) fn handle_server_method(
    method: &str,
    id: Option<Value>,
    params: Option<&Value>,
) -> Result<Option<Value>> {
    match method {
        "elicitation/create" => {
            let Some(id) = id else {
                return Ok(None);
            };
            Ok(Some(handle_elicitation_create(id, params)?))
        }
        "notifications/elicitation/complete" => Ok(None),
        _ => Ok(id.map(|id| {
            build_error_response(
                id,
                -32601,
                &format!("MCP client method `{method}` is not implemented by hellox."),
            )
        })),
    }
}

fn handle_elicitation_create(id: Value, params: Option<&Value>) -> Result<Value> {
    let mode = params
        .and_then(|value| value.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("form");
    if mode != "form" && mode != "url" {
        return Ok(build_error_response(
            id,
            -32602,
            &format!("Unsupported MCP elicitation mode `{mode}`."),
        ));
    }

    Ok(json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": {
            "action": "decline"
        }
    }))
}

pub(crate) fn url_elicitation_error_text(error: &Value) -> Result<Option<String>> {
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if code != -32042 {
        return Ok(None);
    }

    let Some(data) = error.get("data") else {
        return Ok(None);
    };
    let Some(elicitations) = data.get("elicitations").and_then(Value::as_array) else {
        return Ok(None);
    };
    if elicitations.is_empty() {
        return Err(anyhow!(
            "MCP URL elicitation error payload did not contain any URLs."
        ));
    }

    let urls = elicitations
        .iter()
        .filter_map(|item| item.get("url").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(anyhow!(
            "MCP URL elicitation error payload did not contain any URL strings."
        ));
    }

    Ok(Some(format!(
        "MCP server requested URL-based follow-up before continuing: {}",
        urls.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn declines_form_elicitation_requests() {
        let response = handle_server_method(
            "elicitation/create",
            Some(json!(7)),
            Some(&json!({
                "mode": "form",
                "title": "Need confirmation"
            })),
        )
        .expect("handle elicitation")
        .expect("response");

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 7);
        assert_eq!(response["result"]["action"], "decline");
    }

    #[test]
    fn ignores_completion_notifications() {
        let response = handle_server_method(
            "notifications/elicitation/complete",
            None,
            Some(&json!({ "elicitationId": "el-1" })),
        )
        .expect("handle completion");
        assert!(response.is_none());
    }

    #[test]
    fn renders_url_elicitation_errors() {
        let text = url_elicitation_error_text(&json!({
            "code": -32042,
            "message": "authorization required",
            "data": {
                "elicitations": [
                    { "url": "https://example.test/authorize" }
                ]
            }
        }))
        .expect("render text")
        .expect("formatted text");
        assert!(text.contains("https://example.test/authorize"));
    }
}
