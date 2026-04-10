use std::collections::BTreeMap;
use std::net::TcpStream;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::{HeaderName, HeaderValue};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};

use super::{
    build_notification, build_request, initialize_params, process_incoming_message,
    TransportSession, MCP_PROTOCOL_VERSION,
};

pub(super) struct WsSession {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    negotiated_protocol_version: Option<String>,
    next_id: u64,
}

impl WsSession {
    pub(super) fn connect(
        server_name: &str,
        endpoint: &str,
        headers: BTreeMap<String, String>,
    ) -> Result<Self> {
        let mut request = endpoint.into_client_request().with_context(|| {
            format!("Invalid WebSocket endpoint `{endpoint}` for MCP server `{server_name}`.")
        })?;
        for (key, value) in headers {
            request.headers_mut().insert(
                parse_header_name(&key)?,
                HeaderValue::from_str(&value)
                    .with_context(|| format!("Invalid WebSocket header value for `{key}`."))?,
            );
        }

        let (socket, _) = connect(request)
            .with_context(|| format!("Failed to connect to MCP WebSocket `{endpoint}`."))?;
        Ok(Self {
            socket,
            negotiated_protocol_version: None,
            next_id: 1,
        })
    }

    fn send_message(&mut self, message: &Value) -> Result<()> {
        let payload = serde_json::to_string(message)?;
        self.socket
            .send(Message::Text(payload.into()))
            .context("Failed to send MCP WebSocket message.")
    }

    fn read_response(&mut self, request_id: u64) -> Result<Value> {
        loop {
            let message = self
                .socket
                .read()
                .context("Failed to read MCP WebSocket message.")?;
            match message {
                Message::Text(payload) => {
                    if let Some(result) =
                        process_incoming_message(&payload, request_id, |message| {
                            self.send_message(&message)
                        })?
                    {
                        return Ok(result);
                    }
                }
                Message::Binary(payload) => {
                    let payload = String::from_utf8(payload.to_vec())
                        .context("MCP WebSocket binary frame was not valid UTF-8.")?;
                    if let Some(result) =
                        process_incoming_message(&payload, request_id, |message| {
                            self.send_message(&message)
                        })?
                    {
                        return Ok(result);
                    }
                }
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .context("Failed to answer MCP WebSocket ping.")?;
                }
                Message::Pong(_) => {}
                Message::Close(frame) => {
                    let reason = frame
                        .map(|frame| format!(": {}", frame.reason))
                        .unwrap_or_default();
                    return Err(anyhow!(
                        "MCP WebSocket closed before a matching response arrived{reason}"
                    ));
                }
                Message::Frame(_) => {}
            }
        }
    }
}

impl TransportSession for WsSession {
    fn request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_message(&build_request(id, method, params))?;
        self.read_response(id)
    }

    fn initialize(&mut self) -> Result<()> {
        let response = self.request("initialize", Some(initialize_params()))?;
        self.negotiated_protocol_version = response
            .get("protocolVersion")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| Some(MCP_PROTOCOL_VERSION.to_string()));
        self.send_message(&build_notification("notifications/initialized", None))
    }

    fn terminate(&mut self) -> Result<()> {
        let _ = self.socket.close(None);
        Ok(())
    }
}

fn parse_header_name(value: &str) -> Result<HeaderName> {
    HeaderName::from_bytes(value.as_bytes())
        .with_context(|| format!("Invalid WebSocket header name `{value}` in MCP config."))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::net::TcpListener;
    use std::thread;

    use hellox_auth::LocalAuthStoreBackend;
    use hellox_config::{McpScope, McpServerConfig, McpTransportConfig};
    use serde_json::{json, Value};
    use tungstenite::{accept, Message};

    use crate::{get_prompt, list_prompts, list_tools};

    use super::MCP_PROTOCOL_VERSION;

    #[test]
    fn websocket_runtime_lists_tools() {
        let (url, handle) = spawn_server(|socket| {
            let initialize = read_json(socket);
            assert_eq!(initialize["method"], "initialize");
            socket
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "id": initialize["id"],
                        "result": {
                            "protocolVersion": MCP_PROTOCOL_VERSION,
                            "capabilities": {}
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .expect("send initialize response");

            let initialized = read_json(socket);
            assert_eq!(initialized["method"], "notifications/initialized");

            let request = read_json(socket);
            assert_eq!(request["method"], "tools/list");
            socket
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": {
                            "tools": [
                                { "name": "read_file", "description": "Read files" }
                            ]
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .expect("send tools response");
        });

        let backend = LocalAuthStoreBackend::default();
        let server = ws_server(url);
        let result = list_tools(&backend, "filesystem", &server).expect("list tools");
        assert_eq!(result["tools"][0]["name"], "read_file");
        handle.join().expect("join ws server");
    }

    #[test]
    fn websocket_runtime_lists_and_gets_prompts() {
        let (list_url, list_handle) = spawn_server(|socket| {
            let initialize = read_json(socket);
            socket
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "id": initialize["id"],
                        "result": {
                            "protocolVersion": MCP_PROTOCOL_VERSION,
                            "capabilities": {}
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .expect("send initialize response");
            let _initialized = read_json(socket);

            let request = read_json(socket);
            assert_eq!(request["method"], "prompts/list");
            socket
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": {
                            "prompts": [
                                {
                                    "name": "reviewer",
                                    "description": "Review a patch"
                                }
                            ]
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .expect("send prompts response");
        });

        let backend = LocalAuthStoreBackend::default();
        let list_server = ws_server(list_url);
        let listed = list_prompts(&backend, "docs", &list_server).expect("list prompts");
        assert_eq!(listed["prompts"][0]["name"], "reviewer");
        list_handle.join().expect("join prompts list server");

        let (get_url, get_handle) = spawn_server(|socket| {
            let initialize = read_json(socket);
            socket
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "id": initialize["id"],
                        "result": {
                            "protocolVersion": MCP_PROTOCOL_VERSION,
                            "capabilities": {}
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .expect("send initialize response");
            let _initialized = read_json(socket);

            let request = read_json(socket);
            assert_eq!(request["method"], "prompts/get");
            assert_eq!(request["params"]["name"], "reviewer");
            assert_eq!(request["params"]["arguments"]["audience"], "dev");
            socket
                .send(Message::Text(
                    json!({
                        "jsonrpc": "2.0",
                        "id": request["id"],
                        "result": {
                            "description": "Review a patch",
                            "messages": [
                                {
                                    "role": "user",
                                    "content": "Review this diff"
                                }
                            ]
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .expect("send prompt response");
        });

        let get_server = ws_server(get_url);
        let prompt = get_prompt(
            &backend,
            "docs",
            &get_server,
            "reviewer",
            Some(json!({ "audience": "dev" })),
        )
        .expect("get prompt");
        assert_eq!(prompt["messages"][0]["role"], "user");
        get_handle.join().expect("join prompts get server");
    }

    fn ws_server(url: String) -> McpServerConfig {
        McpServerConfig {
            enabled: true,
            description: None,
            scope: McpScope::User,
            oauth: None,
            transport: McpTransportConfig::Ws {
                url,
                headers: BTreeMap::new(),
            },
        }
    }

    fn spawn_server(
        handler: impl FnOnce(&mut tungstenite::WebSocket<std::net::TcpStream>) + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ws server");
        let address = listener.local_addr().expect("ws server addr");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept ws client");
            let mut socket = accept(stream).expect("accept websocket");
            handler(&mut socket);
            let _ = socket.close(None);
        });
        (format!("ws://{address}"), handle)
    }

    fn read_json(socket: &mut tungstenite::WebSocket<std::net::TcpStream>) -> Value {
        match socket.read().expect("read ws message") {
            Message::Text(payload) => serde_json::from_str(&payload).expect("parse json message"),
            Message::Binary(payload) => serde_json::from_slice(&payload).expect("parse json bytes"),
            other => panic!("unexpected websocket message: {other:?}"),
        }
    }
}
