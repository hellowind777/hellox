use std::collections::BTreeMap;
use std::fmt;
use std::io::{BufRead, BufReader};

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{StatusCode, Url};
use serde_json::Value;

use super::{
    build_notification, build_request, initialize_params, process_incoming_message,
    TransportSession, MCP_PROTOCOL_VERSION,
};

pub(super) struct HttpSession {
    server_name: String,
    endpoint: String,
    headers: BTreeMap<String, String>,
    transport: HttpTransportMode,
}

enum HttpTransportMode {
    Streamable(StreamableHttpSession),
    Legacy(LegacySseSession),
}

struct StreamableHttpSession {
    client: Client,
    endpoint: String,
    headers: BTreeMap<String, String>,
    session_id: Option<String>,
    negotiated_protocol_version: Option<String>,
    next_id: u64,
    pending_response: Option<Response>,
}

struct LegacySseSession {
    client: Client,
    message_endpoint: String,
    headers: BTreeMap<String, String>,
    session_id: Option<String>,
    negotiated_protocol_version: Option<String>,
    next_id: u64,
    event_stream: BufReader<Response>,
}

#[derive(Debug, Clone, Copy)]
struct LegacySseFallbackRequired(StatusCode);

impl fmt::Display for LegacySseFallbackRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Configured `sse` endpoint did not accept streamable HTTP POST (status {}). Falling back to legacy HTTP+SSE.",
            self.0
        )
    }
}

impl std::error::Error for LegacySseFallbackRequired {}

#[derive(Debug, Default)]
struct SseEvent {
    event: Option<String>,
    data_lines: Vec<String>,
}

impl SseEvent {
    fn kind(&self) -> &str {
        self.event.as_deref().unwrap_or("message")
    }

    fn payload(&self) -> Option<String> {
        (!self.data_lines.is_empty()).then(|| self.data_lines.join("\n"))
    }
}

impl HttpSession {
    pub(super) fn connect(
        server_name: &str,
        endpoint: &str,
        headers: BTreeMap<String, String>,
    ) -> Result<Self> {
        Ok(Self {
            server_name: server_name.to_string(),
            endpoint: endpoint.to_string(),
            transport: HttpTransportMode::Streamable(StreamableHttpSession::connect(
                server_name,
                endpoint,
                headers.clone(),
            )?),
            headers,
        })
    }
}

impl TransportSession for HttpSession {
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        match &mut self.transport {
            HttpTransportMode::Streamable(session) => session.request(method, params),
            HttpTransportMode::Legacy(session) => session.request(method, params),
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let should_fallback = match &mut self.transport {
            HttpTransportMode::Streamable(session) => match session.initialize() {
                Ok(()) => return Ok(()),
                Err(error) if error.downcast_ref::<LegacySseFallbackRequired>().is_some() => true,
                Err(error) => return Err(error),
            },
            HttpTransportMode::Legacy(session) => return session.initialize(),
        };

        if should_fallback {
            let mut session =
                LegacySseSession::connect(&self.server_name, &self.endpoint, self.headers.clone())?;
            session.initialize()?;
            self.transport = HttpTransportMode::Legacy(session);
        }

        Ok(())
    }

    fn terminate(&mut self) -> Result<()> {
        match &mut self.transport {
            HttpTransportMode::Streamable(session) => session.terminate(),
            HttpTransportMode::Legacy(session) => session.terminate(),
        }
    }
}

impl StreamableHttpSession {
    fn connect(
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

        let request = apply_common_request_headers(
            self.client
                .post(&self.endpoint)
                .header(ACCEPT, "application/json, text/event-stream")
                .header(CONTENT_TYPE, "application/json"),
            &self.headers,
            (!initialize).then(|| self.protocol_version()),
            (!initialize)
                .then_some(self.session_id.as_deref())
                .flatten(),
        )?;

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
        read_event_stream_response(&mut reader, request_id, |message| {
            self.send_auxiliary(&message)
        })
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

impl TransportSession for StreamableHttpSession {
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
            let request = apply_common_request_headers(
                self.client.delete(&self.endpoint),
                &self.headers,
                Some(self.protocol_version()),
                Some(session_id.as_str()),
            )?;
            let _ = request.send();
        }
        Ok(())
    }
}

impl LegacySseSession {
    fn connect(
        server_name: &str,
        endpoint: &str,
        headers: BTreeMap<String, String>,
    ) -> Result<Self> {
        let client = Client::builder().build().with_context(|| {
            format!("Failed to create HTTP client for MCP server `{server_name}`.")
        })?;

        let response = apply_common_request_headers(
            client.get(endpoint).header(ACCEPT, "text/event-stream"),
            &headers,
            None,
            None,
        )?
        .send()
        .with_context(|| format!("Failed to connect to MCP SSE endpoint `{endpoint}`."))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "MCP SSE handshake failed with status {}.",
                response.status()
            ));
        }
        if !is_sse_response(response.headers()) {
            return Err(anyhow!(
                "MCP SSE endpoint `{endpoint}` did not return `text/event-stream`."
            ));
        }

        let session_id = session_header(response.headers());
        let mut event_stream = BufReader::new(response);
        let message_endpoint = read_legacy_message_endpoint(&mut event_stream, endpoint)?;

        Ok(Self {
            client,
            message_endpoint,
            headers,
            session_id,
            negotiated_protocol_version: None,
            next_id: 1,
            event_stream,
        })
    }

    fn send_message(
        &mut self,
        message: &Value,
        initialize: bool,
        session_id: Option<&str>,
    ) -> Result<()> {
        let response = apply_common_request_headers(
            self.client
                .post(&self.message_endpoint)
                .header(ACCEPT, "application/json, text/event-stream")
                .header(CONTENT_TYPE, "application/json"),
            &self.headers,
            (!initialize).then(|| self.protocol_version()),
            session_id.or(self.session_id.as_deref()),
        )?
        .body(serde_json::to_vec(message)?)
        .send()
        .with_context(|| {
            format!(
                "Failed to reach MCP legacy message endpoint `{}`.",
                self.message_endpoint
            )
        })?;

        if let Some(session_id) = session_header(response.headers()) {
            self.session_id = Some(session_id);
        }

        ensure_auxiliary_status(&response, false)
    }

    fn read_response(&mut self, request_id: u64) -> Result<Value> {
        let client = self.client.clone();
        let headers = self.headers.clone();
        let message_endpoint = self.message_endpoint.clone();
        let protocol_version = self.protocol_version();
        let session_id = self.session_id.clone();
        read_event_stream_response(&mut self.event_stream, request_id, move |message| {
            send_legacy_auxiliary(
                &client,
                &message_endpoint,
                &headers,
                session_id.as_deref(),
                &protocol_version,
                &message,
            )
        })
    }

    fn protocol_version(&self) -> String {
        self.negotiated_protocol_version
            .clone()
            .unwrap_or_else(|| MCP_PROTOCOL_VERSION.to_string())
    }
}

impl TransportSession for LegacySseSession {
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_message(
            &build_request(id, method, params),
            method == "initialize",
            None,
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
            None,
        )
    }

    fn terminate(&mut self) -> Result<()> {
        Ok(())
    }
}

fn apply_common_request_headers(
    mut request: RequestBuilder,
    headers: &BTreeMap<String, String>,
    protocol_version: Option<String>,
    session_id: Option<&str>,
) -> Result<RequestBuilder> {
    for (key, value) in headers {
        request = request.header(parse_header_name(key)?, HeaderValue::from_str(value)?);
    }
    if let Some(protocol_version) = protocol_version {
        request = request.header("MCP-Protocol-Version", protocol_version);
    }
    if let Some(session_id) = session_id {
        request = request.header("MCP-Session-Id", session_id);
    }
    Ok(request)
}

fn send_legacy_auxiliary(
    client: &Client,
    message_endpoint: &str,
    headers: &BTreeMap<String, String>,
    session_id: Option<&str>,
    protocol_version: &str,
    message: &Value,
) -> Result<()> {
    let response = apply_common_request_headers(
        client
            .post(message_endpoint)
            .header(ACCEPT, "application/json, text/event-stream")
            .header(CONTENT_TYPE, "application/json"),
        headers,
        Some(protocol_version.to_string()),
        session_id,
    )?
    .body(serde_json::to_vec(message)?)
    .send()
    .with_context(|| {
        format!("Failed to reach MCP legacy message endpoint `{message_endpoint}`.")
    })?;
    ensure_auxiliary_status(&response, false)
}

fn ensure_request_status(response: &Response, initialize: bool) -> Result<()> {
    if response.status().is_success() {
        return Ok(());
    }

    if initialize && response.status().is_client_error() {
        return Err(anyhow!(LegacySseFallbackRequired(response.status())));
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

fn read_legacy_message_endpoint(reader: &mut impl BufRead, endpoint: &str) -> Result<String> {
    loop {
        let Some(event) = read_next_sse_event(reader)? else {
            return Err(anyhow!(
                "MCP legacy SSE stream ended before advertising a message endpoint."
            ));
        };
        if event.kind() != "endpoint" {
            continue;
        }
        let payload = event
            .payload()
            .ok_or_else(|| anyhow!("MCP legacy SSE endpoint event was missing a URL payload."))?;
        return resolve_message_endpoint(endpoint, &payload);
    }
}

fn resolve_message_endpoint(endpoint: &str, raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!(
            "MCP legacy SSE endpoint event did not contain a message URL."
        ));
    }

    if let Ok(url) = Url::parse(trimmed) {
        return Ok(url.to_string());
    }

    let base = Url::parse(endpoint)
        .with_context(|| format!("Invalid MCP SSE endpoint URL `{endpoint}`."))?;
    base.join(trimmed)
        .map(|url| url.to_string())
        .with_context(|| {
            format!(
                "Failed to resolve legacy MCP message endpoint `{trimmed}` against `{endpoint}`."
            )
        })
}

fn read_event_stream_response(
    reader: &mut impl BufRead,
    request_id: u64,
    mut send_auxiliary: impl FnMut(Value) -> Result<()>,
) -> Result<Value> {
    loop {
        let Some(event) = read_next_sse_event(reader)? else {
            return Err(anyhow!(
                "MCP event stream ended before a matching response arrived."
            ));
        };
        let Some(payload) = event.payload() else {
            continue;
        };
        if event.kind() == "endpoint" {
            continue;
        }
        if let Some(result) =
            process_incoming_message(&payload, request_id, |message| send_auxiliary(message))?
        {
            return Ok(result);
        }
    }
}

fn read_next_sse_event(reader: &mut impl BufRead) -> Result<Option<SseEvent>> {
    let mut event = SseEvent::default();
    let mut saw_content = false;

    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .context("Failed to read MCP event stream.")?;

        if read == 0 {
            if !saw_content && event.data_lines.is_empty() && event.event.is_none() {
                return Ok(None);
            }
            return Ok(Some(event));
        }

        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if !saw_content && event.data_lines.is_empty() && event.event.is_none() {
                continue;
            }
            return Ok(Some(event));
        }

        if line.starts_with(':') {
            continue;
        }

        saw_content = true;

        if let Some(value) = line.strip_prefix("event:") {
            event.event = Some(value.trim_start().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            event.data_lines.push(value.trim_start().to_string());
        }
    }
}

fn parse_header_name(value: &str) -> Result<HeaderName> {
    HeaderName::from_bytes(value.as_bytes())
        .with_context(|| format!("Invalid HTTP header name `{value}` in MCP config."))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::thread;

    use hellox_auth::LocalAuthStoreBackend;
    use hellox_config::{McpScope, McpServerConfig, McpTransportConfig};
    use serde_json::{json, Value};

    use crate::list_tools;

    use super::super::MCP_PROTOCOL_VERSION;

    #[test]
    fn sse_transport_falls_back_to_legacy_http_sse() {
        let (url, handle) = spawn_legacy_sse_server();
        let backend = LocalAuthStoreBackend::default();
        let server = sse_server(url);

        let result = list_tools(&backend, "legacy", &server).expect("list tools via legacy sse");
        assert_eq!(result["tools"][0]["name"], "read_file");
        handle.join().expect("join legacy sse server");
    }

    fn sse_server(url: String) -> McpServerConfig {
        McpServerConfig {
            enabled: true,
            description: Some("legacy sse test".to_string()),
            scope: McpScope::User,
            oauth: None,
            transport: McpTransportConfig::Sse {
                url,
                headers: BTreeMap::from([("X-Test-Token".to_string(), "secret".to_string())]),
            },
        }
    }

    fn spawn_legacy_sse_server() -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener addr");
        let handle = thread::spawn(move || {
            let mut sse_stream: Option<TcpStream> = None;

            for _ in 0..5 {
                let (mut stream, _) = listener.accept().expect("accept request");
                let request = read_http_request(&stream);
                assert_eq!(
                    request.headers.get("x-test-token"),
                    Some(&"secret".to_string())
                );

                match (request.method.as_str(), request.path.as_str()) {
                    ("POST", "/mcp") => {
                        let payload: Value =
                            serde_json::from_str(&request.body).expect("parse fallback request");
                        assert_eq!(payload["method"], "initialize");
                        write_http_response(
                            &mut stream,
                            "405 Method Not Allowed",
                            &[("Connection", "close")],
                            "",
                        );
                    }
                    ("GET", "/mcp") => {
                        write_http_stream_headers(
                            &mut stream,
                            "200 OK",
                            &[("Content-Type", "text/event-stream")],
                        );
                        write_sse_event(
                            &mut stream,
                            Some("endpoint"),
                            "/messages?sessionId=legacy-session",
                        );
                        sse_stream = Some(stream);
                    }
                    ("POST", path) if path.starts_with("/messages?sessionId=legacy-session") => {
                        let payload: Value =
                            serde_json::from_str(&request.body).expect("parse legacy post body");
                        match payload["method"].as_str() {
                            Some("initialize") => {
                                write_http_response(
                                    &mut stream,
                                    "202 Accepted",
                                    &[("Connection", "close")],
                                    "",
                                );
                                write_sse_event(
                                    sse_stream.as_mut().expect("legacy sse stream available"),
                                    Some("message"),
                                    &json!({
                                        "jsonrpc": "2.0",
                                        "id": payload["id"],
                                        "result": {
                                            "protocolVersion": MCP_PROTOCOL_VERSION,
                                            "capabilities": {}
                                        }
                                    })
                                    .to_string(),
                                );
                            }
                            Some("notifications/initialized") => {
                                write_http_response(
                                    &mut stream,
                                    "202 Accepted",
                                    &[("Connection", "close")],
                                    "",
                                );
                            }
                            Some("tools/list") => {
                                write_http_response(
                                    &mut stream,
                                    "202 Accepted",
                                    &[("Connection", "close")],
                                    "",
                                );
                                write_sse_event(
                                    sse_stream.as_mut().expect("legacy sse stream available"),
                                    Some("message"),
                                    &json!({
                                        "jsonrpc": "2.0",
                                        "id": payload["id"],
                                        "result": {
                                            "tools": [
                                                {
                                                    "name": "read_file",
                                                    "description": "Read files"
                                                }
                                            ]
                                        }
                                    })
                                    .to_string(),
                                );
                            }
                            other => panic!("unexpected legacy SSE method: {other:?}"),
                        }
                    }
                    other => panic!("unexpected request: {other:?}"),
                }
            }

            if let Some(stream) = sse_stream.as_mut() {
                let _ = stream.shutdown(Shutdown::Both);
            }
        });

        (format!("http://{address}/mcp"), handle)
    }

    struct SimpleHttpRequest {
        method: String,
        path: String,
        headers: BTreeMap<String, String>,
        body: String,
    }

    fn read_http_request(stream: &TcpStream) -> SimpleHttpRequest {
        let mut reader = BufReader::new(stream.try_clone().expect("clone request stream"));
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .expect("read request line");
        let request_line = request_line.trim_end_matches(['\r', '\n']);
        let mut parts = request_line.split_whitespace();
        let method = parts.next().expect("request method").to_string();
        let path = parts.next().expect("request path").to_string();

        let mut headers = BTreeMap::new();
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("read header line");
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                if key == "content-length" {
                    content_length = value.parse::<usize>().expect("parse content-length");
                }
                headers.insert(key, value);
            }
        }

        let mut body = vec![0u8; content_length];
        if content_length > 0 {
            reader.read_exact(&mut body).expect("read request body");
        }

        SimpleHttpRequest {
            method,
            path,
            headers,
            body: String::from_utf8(body).expect("utf8 body"),
        }
    }

    fn write_http_stream_headers(stream: &mut TcpStream, status: &str, headers: &[(&str, &str)]) {
        write!(stream, "HTTP/1.1 {status}\r\n").expect("write status line");
        for (key, value) in headers {
            write!(stream, "{key}: {value}\r\n").expect("write stream header");
        }
        write!(stream, "Connection: keep-alive\r\n\r\n").expect("finish stream headers");
        stream.flush().expect("flush stream headers");
    }

    fn write_http_response(
        stream: &mut TcpStream,
        status: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) {
        write!(stream, "HTTP/1.1 {status}\r\n").expect("write status line");
        for (key, value) in headers {
            write!(stream, "{key}: {value}\r\n").expect("write header");
        }
        write!(stream, "Content-Length: {}\r\n\r\n", body.len()).expect("write content-length");
        write!(stream, "{body}").expect("write body");
        stream.flush().expect("flush response");
    }

    fn write_sse_event(stream: &mut TcpStream, event: Option<&str>, payload: &str) {
        if let Some(event) = event {
            write!(stream, "event: {event}\r\n").expect("write event kind");
        }
        for line in payload.lines() {
            write!(stream, "data: {line}\r\n").expect("write event payload");
        }
        write!(stream, "\r\n").expect("finish sse event");
        stream.flush().expect("flush sse event");
    }
}
