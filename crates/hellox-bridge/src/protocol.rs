use std::io::{BufRead, Write};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{inspect_bridge_status, list_bridge_sessions, load_bridge_session, BridgeRuntimePaths};

#[derive(Debug, Deserialize)]
struct BridgeRequest {
    #[serde(default)]
    id: Option<String>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn run_stdio_bridge<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    paths: &BridgeRuntimePaths,
) -> Result<()> {
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let outcome = match serde_json::from_str::<BridgeRequest>(&line) {
            Ok(request) => handle_request(paths, request),
            Err(error) => RequestOutcome::error(None, format!("invalid bridge request: {error}")),
        };

        let raw = serde_json::to_string(&outcome.response)?;
        writer.write_all(raw.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        if outcome.shutdown {
            break;
        }
    }

    Ok(())
}

struct RequestOutcome {
    response: BridgeResponse,
    shutdown: bool,
}

impl RequestOutcome {
    fn success(id: Option<String>, result: Value, shutdown: bool) -> Self {
        Self {
            response: BridgeResponse {
                id,
                ok: true,
                result: Some(result),
                error: None,
            },
            shutdown,
        }
    }

    fn error(id: Option<String>, error: String) -> Self {
        Self {
            response: BridgeResponse {
                id,
                ok: false,
                result: None,
                error: Some(error),
            },
            shutdown: false,
        }
    }
}

fn handle_request(paths: &BridgeRuntimePaths, request: BridgeRequest) -> RequestOutcome {
    let id = request.id.clone();
    let result = match request.method.as_str() {
        "status" => inspect_bridge_status(paths).map(|status| (json!(status), false)),
        "sessions/list" => list_bridge_sessions(paths).map(|sessions| (json!(sessions), false)),
        "sessions/get" => required_string_param(&request.params, "session_id")
            .and_then(|session_id| load_bridge_session(paths, &session_id))
            .map(|detail| (json!(detail), false)),
        "shutdown" => Ok((json!({ "message": "bridge shutting down" }), true)),
        _ => Err(anyhow!("unsupported bridge method `{}`", request.method)),
    };

    match result {
        Ok((payload, shutdown)) => RequestOutcome::success(id, payload, shutdown),
        Err(error) => RequestOutcome::error(id, error.to_string()),
    }
}

fn required_string_param(params: &Value, key: &str) -> Result<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("bridge request is missing `{key}`"))
}
