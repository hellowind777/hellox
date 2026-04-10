use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::StatusCode;
use serde_json::Value;

use super::{
    build_notification, build_request, initialize_params, process_incoming_message,
    TransportSession, MCP_PROTOCOL_VERSION,
};

pub(super) struct HttpSession {
    client: Client,
    endpoint: String,
    headers: BTreeMap<String, String>,
    session_id: Option<String>,
    negotiated_protocol_version: Option<String>,
    next_id: u64,
    pending_response: Option<Response>,
}

impl HttpSession {
    pub(super) fn connect(
        server_name: &str,
        endpoint: &str,
        headers: BTreeMap<String, String>,
    ) -> Result<Self> {
        Ok(Self {
            client: Client::builder().build().with_context(|| {
                format!("Failed to create HTTP client for MCP server `{server_name}`.")
            })?,
            endpoint: endpoint.to_string(),
            headers,
            session_id: None,
            negotiated_protocol_version: None,
            next_id: 1,
            pending_response: None,
        })
    }

    fn send_message(
        &mut self,
        message: &Value,
        expects_response: bool,
        initialize: bool,
    ) -> Result<()> {
        if expects_response && self.pending_response.is_some() {
            return Err(anyhow!("MCP HTTP client already has a pending response."));
        }

        let mut request = self
            .client
            .post(&self.endpoint)
            .header(ACCEPT, "application/json, text/event-stream")
            .header(CONTENT_TYPE, "application/json");
        for (key, value) in &self.headers {
            request = request.header(parse_header_name(key)?, HeaderValue::from_str(value)?);
        }
        if !initialize {
            request = request.header("MCP-Protocol-Version", self.protocol_version());
            if let Some(session_id) = &self.session_id {
                request = request.header("MCP-Session-Id", session_id);
            }
        }

        let response = request
            .body(serde_json::to_vec(message)?)
            .send()
            .with_context(|| format!("Failed to reach MCP endpoint `{}`.", self.endpoint))?;

        if let Some(session_id) = session_header(response.headers()) {
            self.session_id = Some(session_id);
        }

        if expects_response {
            ensure_request_status(&response, initialize)?;
            self.pending_response = Some(response);
            return Ok(());
        }

        ensure_auxiliary_status(&response, initialize)
    }

    fn read_response(&mut self, request_id: u64) -> Result<Value> {
        let response = self
            .pending_response
            .take()
            .ok_or_else(|| anyhow!("MCP HTTP client is missing a pending response."))?;

        if is_sse_response(response.headers()) {
            return self.read_sse_response(response, request_id);
        }

        let payload: Value = response
            .json()
            .context("Failed to parse MCP HTTP response body.")?;
        let raw = serde_json::to_string(&payload)?;
        process_incoming_message(&raw, request_id, |message| self.send_auxiliary(&message))
            .and_then(|result| {
                result
                    .ok_or_else(|| anyhow!("MCP HTTP response did not contain a matching result."))
            })
    }

    fn read_sse_response(&mut self, response: Response, request_id: u64) -> Result<Value> {
        let mut reader = BufReader::new(response);
        let mut data_lines = Vec::new();

        loop {
            let mut line = String::new();
            let read = reader
                .read_line(&mut line)
                .context("Failed to read MCP event stream.")?;
            if read == 0 {
                return finalize_event(data_lines, request_id, |message| {
                    self.send_auxiliary(&message)
                })
                .and_then(|result| {
                    result.ok_or_else(|| {
                        anyhow!("MCP event stream ended before a matching response arrived.")
                    })
                });
            }

            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if let Some(result) =
                    finalize_event(std::mem::take(&mut data_lines), request_id, |message| {
                        self.send_auxiliary(&message)
                    })?
                {
                    return Ok(result);
                }
                continue;
            }

            if line.starts_with(':') {
                continue;
            }
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.trim_start().to_string());
            }
        }
    }

    fn send_auxiliary(&mut self, message: &Value) -> Result<()> {
        self.send_message(message, false, false)
    }

    fn protocol_version(&self) -> String {
        self.negotiated_protocol_version
            .clone()
            .unwrap_or_else(|| MCP_PROTOCOL_VERSION.to_string())
    }
}

impl TransportSession for HttpSession {
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_message(
            &build_request(id, method, params),
            true,
            method == "initialize",
        )?;
        self.read_response(id)
    }

    fn initialize(&mut self) -> Result<()> {
        let response = self.request("initialize", Some(initialize_params()))?;
        self.negotiated_protocol_version = response
            .get("protocolVersion")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| Some(MCP_PROTOCOL_VERSION.to_string()));
        self.send_message(
            &build_notification("notifications/initialized", None),
            false,
            false,
        )
    }

    fn terminate(&mut self) -> Result<()> {
        if let Some(session_id) = &self.session_id {
            let mut request = self.client.delete(&self.endpoint);
            for (key, value) in &self.headers {
                request = request.header(parse_header_name(key)?, HeaderValue::from_str(value)?);
            }
            let _ = request
                .header("MCP-Protocol-Version", self.protocol_version())
                .header("MCP-Session-Id", session_id)
                .send();
        }
        Ok(())
    }
}

fn ensure_request_status(response: &Response, initialize: bool) -> Result<()> {
    if response.status().is_success() {
        return Ok(());
    }

    if initialize
        && (response.status() == StatusCode::METHOD_NOT_ALLOWED
            || response.status() == StatusCode::NOT_FOUND)
    {
        return Err(anyhow!(
            "Configured `sse` endpoint did not accept streamable HTTP POST. Legacy HTTP+SSE fallback is not implemented yet."
        ));
    }

    Err(anyhow!(
        "MCP HTTP request failed with status {}.",
        response.status()
    ))
}

fn ensure_auxiliary_status(response: &Response, initialize: bool) -> Result<()> {
    if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
        return Ok(());
    }
    ensure_request_status(response, initialize)
}

fn session_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get("MCP-Session-Id")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

fn is_sse_response(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        .unwrap_or(false)
}

fn finalize_event(
    data_lines: Vec<String>,
    request_id: u64,
    mut send_auxiliary: impl FnMut(Value) -> Result<()>,
) -> Result<Option<Value>> {
    if data_lines.is_empty() {
        return Ok(None);
    }

    let payload = data_lines.join("\n");
    process_incoming_message(&payload, request_id, |message| send_auxiliary(message))
}

fn parse_header_name(value: &str) -> Result<HeaderName> {
    HeaderName::from_bytes(value.as_bytes())
        .with_context(|| format!("Invalid HTTP header name `{value}` in MCP config."))
}
